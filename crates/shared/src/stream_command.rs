use clap::Parser;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::str::FromStr;

#[derive(Parser, Debug)]
pub struct StreamRawCommand {
    #[arg(value_name = "\"STREAM\"")]
    command: String,

    #[arg(value_name = "udp://ADDRESS:PORT")]
    address: String,

    #[arg(value_name = "QUOTE1,QUOTE2,...")]
    quotes: String
}

#[derive(Debug)]
pub struct StreamCommand {
    address: SocketAddrV4,
    pub quotes: Vec<String>
}

impl TryFrom<StreamRawCommand> for StreamCommand {
    type Error = String;

    fn try_from(value: StreamRawCommand) -> Result<Self, Self::Error> {
        if value.command != "STREAM" {
            return Err(format!("Invalid command: {}", value.command));
        }

        let parts: Vec<&str> = value.address.split("://").collect();
        if parts.len() != 2 || parts[0] != "udp" {
            return Err(format!("Invalid address format: {}", value.address));
        }

        let addr_parts: Vec<&str> = parts[1].split(':').collect();
        if addr_parts.len() != 2 {
            return Err(format!("Only single colon support: {}", value.address));
        }

        let ip_address = Ipv4Addr::from_str(addr_parts[0]).map_err(|e| format!("Invalid IPv4 address format: {}", e))?;
        let port = addr_parts[1].parse::<u16>().map_err(|e| format!("Invalid port number: {}", e))?;
        let address = SocketAddrV4::new(ip_address, port);

        Ok(StreamCommand {
            address,
            quotes: value.quotes.split(',').map(|s| s.trim().to_string()).collect()
        })
    }
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