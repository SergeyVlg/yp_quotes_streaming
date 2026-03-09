use std::net::Ipv4Addr;
use std::str::FromStr;
use clap::Parser;

#[derive(Parser, Debug)]
struct StreamRawCommand {
    #[arg(value_name = "\"STREAM\"")]
    command: String,

    #[arg(value_name = "udp://ADDRESS:PORT")]
    address: String,

    #[arg(value_name = "QUOTE1,QUOTE2,...")]
    quotes: String
}

#[derive(Debug)]
pub struct StreamCommand {
    address: Ipv4Addr,
    port: u16,
    quotes: Vec<String>
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

        let address = Ipv4Addr::from_str(addr_parts[0]).map_err(|e| format!("Invalid IPv4 address format: {}", e))?;
        let port = addr_parts[1].parse::<u16>().map_err(|e| format!("Invalid port number: {}", e))?;

        Ok(StreamCommand {
            address,
            port,
            quotes: value.quotes.split(',').map(|s| s.trim().to_string()).collect()
        })
    }
}

impl StreamCommand {
    pub const MAX_COMMAND_SIZE: u64 = 4096;
    const FIELDS_COUNT: usize = 4; //STREAM|Address|Port|Quote1,Quote2,...

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"STREAM");
        bytes.push(b'|');
        bytes.extend_from_slice(self.address.to_string().as_bytes());
        bytes.push(b'|');
        bytes.extend_from_slice(self.port.to_string().as_bytes());
        bytes.push(b'|');
        bytes.extend_from_slice(self.quotes.join(",").as_bytes());
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

        let address = Ipv4Addr::from_str(parts[1]).map_err(|e| format!("Invalid address format: {}", e))?;
        let port = parts[2].parse::<u16>().map_err(|e| format!("Invalid port number: {}", e))?;

        Ok(StreamCommand {
            address,
            port,
            quotes: parts[3].split(',').map(|s| s.trim().to_string()).collect()
        })
    }
}