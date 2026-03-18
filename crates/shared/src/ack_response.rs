use std::io;
use std::io::{BufRead, Read};
use std::net::SocketAddrV4;
use std::str::FromStr;

pub struct AckResponse {
    pub source_address: SocketAddrV4,
}

impl AckResponse {
    const MAX_SIZE: u64 = 256;
    const FIELDS_COUNT: usize = 2; //ACK|<адрес:порт>

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"ACK");
        bytes.push(b'|');
        bytes.extend_from_slice(self.source_address.to_string().as_bytes());
        bytes.push(b'\n');
        bytes
    }

    pub fn try_read_from_reader<R: BufRead>(reader: &mut R) -> io::Result<Self> {
        let mut buf = Vec::with_capacity((Self::MAX_SIZE + 1) as usize);

        let bytes_read = reader
            .take(Self::MAX_SIZE + 1)
            .read_until(b'\n', &mut buf)?;

        if bytes_read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed"));
        }

        if buf.len() > Self::MAX_SIZE as usize {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "ack response too long"));
        }

        if !buf.ends_with(b"\n") {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "newline not found"));
        }

        buf.pop(); //удаляем \n

        let s = std::str::from_utf8(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid UTF-8 data at ack response: {e}")))?;

        let parts: Vec<&str> = s.split('|').collect();
        if parts.len() != Self::FIELDS_COUNT {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Invalid data format: {s}")));
        }

        if parts[0] != "ACK" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Invalid ack response: {}", parts[0])));
        }

        let address = SocketAddrV4::from_str(parts[1])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid socket address format: {e}")))?;

        Ok(Self { source_address: address })
    }
}