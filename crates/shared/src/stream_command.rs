use std::io;
use std::io::{BufRead, Read};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::str::FromStr;

#[derive(Debug)]
pub struct StreamCommand {
    pub address: SocketAddrV4,
    pub quotes: Vec<String>
}

impl StreamCommand {
    const MAX_COMMAND_SIZE: u64 = 4096;
    const FIELDS_COUNT: usize = 3; //STREAM|Address:Port|Quote1,Quote2,...

    pub fn new(address: Ipv4Addr, port: u16, quotes: Vec<String>) -> Self {
        let address = SocketAddrV4::new(address, port);
        StreamCommand { address, quotes }
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

    pub fn try_read_from_reader<R: BufRead>(reader: &mut R) -> io::Result<Self> {
        let mut buf = Vec::with_capacity(256);

        let bytes_read = reader
            .take(Self::MAX_COMMAND_SIZE + 1)
            .read_until(b'\n', &mut buf)?;

        if bytes_read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed"));
        }

        if buf.len() > Self::MAX_COMMAND_SIZE as usize {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "command too long"));
        }

        if !buf.ends_with(b"\n") {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "newline not found"));
        }

        buf.pop(); //удаляем \n

        if matches!(buf.last(), Some(b'\r')) {
            buf.pop();
        }

        let s = std::str::from_utf8(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid UTF-8 data: {e}")))?;
        
        let parts: Vec<&str> = s.split('|').collect();
        if parts.len() != Self::FIELDS_COUNT {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Invalid data format: {s}")));
        }

        if parts[0] != "STREAM" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Invalid command: {}", parts[0])));
        }

        let address = SocketAddrV4::from_str(parts[1])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid socket address format: {e}")))?;

        if parts[2].trim().is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Empty quotes list"));
        }

        let quotes: Vec<String> = parts[2]
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        if quotes.iter().any(|s| s.is_empty()) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Quotes list contains empty quote"));
        }

        Ok(StreamCommand {
            address,
            quotes
        })
    }
}