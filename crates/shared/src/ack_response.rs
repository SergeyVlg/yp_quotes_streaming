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

#[cfg(test)]
mod tests {
    use super::AckResponse;
    use std::io::{Cursor, ErrorKind};
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn to_bytes_serializes_ack_with_newline() {
        let response = AckResponse {
            source_address: SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080),
        };

        assert_eq!(response.to_bytes(), b"ACK|127.0.0.1:8080\n".to_vec());
    }

    #[test]
    fn try_read_from_reader_parses_valid_ack() {
        let mut cursor = Cursor::new(b"ACK|127.0.0.1:7878\n".to_vec());

        let parsed = AckResponse::try_read_from_reader(&mut cursor).unwrap();

        assert_eq!(
            parsed.source_address,
            SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 7878)
        );
    }

    #[test]
    fn try_read_from_reader_returns_unexpected_eof_for_empty_input() {
        let mut cursor = Cursor::new(Vec::<u8>::new());

        let err = match AckResponse::try_read_from_reader(&mut cursor) {
            Ok(_) => panic!("expected error for empty input"),
            Err(e) => e,
        };

        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }

    #[test]
    fn try_read_from_reader_returns_error_when_response_too_long() {
        let mut cursor = Cursor::new(vec![b'a'; 257]);

        let err = match AckResponse::try_read_from_reader(&mut cursor) {
            Ok(_) => panic!("expected error for oversized ack response"),
            Err(e) => e,
        };

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("too long"));
    }

    #[test]
    fn try_read_from_reader_returns_error_when_newline_is_missing() {
        let mut cursor = Cursor::new(b"ACK|127.0.0.1:7878".to_vec());

        let err = match AckResponse::try_read_from_reader(&mut cursor) {
            Ok(_) => panic!("expected error when newline is missing"),
            Err(e) => e,
        };

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("newline not found"));
    }

    #[test]
    fn try_read_from_reader_returns_error_for_invalid_utf8() {
        let mut cursor = Cursor::new(vec![b'A', b'C', b'K', b'|', 0xFF, b'\n']);

        let err = match AckResponse::try_read_from_reader(&mut cursor) {
            Ok(_) => panic!("expected UTF-8 parsing error"),
            Err(e) => e,
        };

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("Invalid UTF-8"));
    }

    #[test]
    fn try_read_from_reader_returns_error_for_wrong_fields_count() {
        let mut cursor = Cursor::new(b"ACK\n".to_vec());

        let err = match AckResponse::try_read_from_reader(&mut cursor) {
            Ok(_) => panic!("expected format error for wrong fields count"),
            Err(e) => e,
        };

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("Invalid data format"));
    }

    #[test]
    fn try_read_from_reader_returns_error_for_wrong_prefix() {
        let mut cursor = Cursor::new(b"NACK|127.0.0.1:7878\n".to_vec());

        let err = match AckResponse::try_read_from_reader(&mut cursor) {
            Ok(_) => panic!("expected error for invalid ack prefix"),
            Err(e) => e,
        };

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("Invalid ack response"));
    }

    #[test]
    fn try_read_from_reader_returns_error_for_invalid_socket_address() {
        let mut cursor = Cursor::new(b"ACK|not-an-address\n".to_vec());

        let err = match AckResponse::try_read_from_reader(&mut cursor) {
            Ok(_) => panic!("expected error for invalid socket address"),
            Err(e) => e,
        };

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("Invalid socket address format"));
    }
}

