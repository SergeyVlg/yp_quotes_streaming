use std::net::{Ipv4Addr, SocketAddrV4};
use std::str::FromStr;

#[derive(Debug)]
pub struct StreamCommand {
    address: SocketAddrV4,
    pub quotes: Vec<String>
}

impl StreamCommand {
    pub const MAX_COMMAND_SIZE: u64 = 4096;
    const FIELDS_COUNT: usize = 3; //STREAM|Address:Port|Quote1,Quote2,...

    pub fn new(address: Ipv4Addr, port: u16, quotes: Vec<String>) -> Self {
        let address = SocketAddrV4::new(address, port);
        StreamCommand { address, quotes }
    }

    pub fn get_socket_address(&self) -> SocketAddrV4 {
        self.address
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"STREAM");
        bytes.push(b'|');
        bytes.extend_from_slice(self.address.to_string().as_bytes());
        bytes.push(b'|');
        bytes.extend_from_slice(self.quotes.join(",").as_bytes());
        bytes.push(b'\n');
        bytes
    }
    
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let s = std::str::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8 data: {}", e))?;
        
        let parts: Vec<&str> = s.split('|').collect();
        if parts.len() != Self::FIELDS_COUNT {
            return Err(format!("Invalid data format: {}", s));
        }

        if parts[0] != "STREAM" {
            return Err(format!("Invalid command: {}", parts[0]));
        }

        let address = SocketAddrV4::from_str(parts[1]).map_err(|e| format!("Invalid socket address format: {}", e))?;

        Ok(StreamCommand {
            address,
            quotes: parts[2].split(',').map(|s| s.trim().to_string()).collect()
        })
    }
}