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
        let mut buf = Vec::with_capacity((Self::MAX_COMMAND_SIZE + 1) as usize);

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

#[cfg(test)]
mod tests {
    use super::StreamCommand;
    use std::io::{Cursor, ErrorKind};
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn new_builds_command_with_socket_address() {
        let command = StreamCommand::new(
            Ipv4Addr::new(127, 0, 0, 1),
            7878,
            vec!["AAPL".to_string(), "TSLA".to_string()],
        );

        assert_eq!(
            command.address,
            SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 7878)
        );
        assert_eq!(command.quotes, vec!["AAPL", "TSLA"]);
    }

    #[test]
    fn to_bytes_serializes_command_with_newline() {
        let command = StreamCommand::new(
            Ipv4Addr::new(127, 0, 0, 1),
            7878,
            vec!["AAPL".to_string(), "TSLA".to_string()],
        );

        assert_eq!(
            command.to_bytes(),
            b"STREAM|127.0.0.1:7878|AAPL,TSLA\n".to_vec()
        );
    }

    #[test]
    fn try_read_from_reader_parses_valid_command() {
        let mut cursor = Cursor::new(b"STREAM|127.0.0.1:9000|AAPL,TSLA\n".to_vec());

        let parsed = StreamCommand::try_read_from_reader(&mut cursor).unwrap();

        assert_eq!(
            parsed.address,
            SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 9000)
        );
        assert_eq!(parsed.quotes, vec!["AAPL", "TSLA"]);
    }

    #[test]
    fn try_read_from_reader_parses_valid_command_with_crlf() {
        let mut cursor = Cursor::new(b"STREAM|127.0.0.1:9000|AAPL,TSLA\r\n".to_vec());

        let parsed = StreamCommand::try_read_from_reader(&mut cursor).unwrap();

        assert_eq!(
            parsed.address,
            SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 9000)
        );
        assert_eq!(parsed.quotes, vec!["AAPL", "TSLA"]);
    }

    #[test]
    fn try_read_from_reader_trims_quotes() {
        let mut cursor = Cursor::new(b"STREAM|127.0.0.1:9000| AAPL , TSLA \n".to_vec());

        let parsed = StreamCommand::try_read_from_reader(&mut cursor).unwrap();

        assert_eq!(parsed.quotes, vec!["AAPL", "TSLA"]);
    }

    #[test]
    fn try_read_from_reader_returns_unexpected_eof_for_empty_input() {
        let mut cursor = Cursor::new(Vec::<u8>::new());

        let err = StreamCommand::try_read_from_reader(&mut cursor).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }

    #[test]
    fn try_read_from_reader_returns_error_for_too_long_command() {
        let mut cursor = Cursor::new(vec![b'a'; 4097]);

        let err = StreamCommand::try_read_from_reader(&mut cursor).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("too long"));
    }

    #[test]
    fn try_read_from_reader_returns_error_when_newline_missing() {
        let mut cursor = Cursor::new(b"STREAM|127.0.0.1:9000|AAPL,TSLA".to_vec());

        let err = StreamCommand::try_read_from_reader(&mut cursor).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("newline not found"));
    }

    #[test]
    fn try_read_from_reader_returns_error_for_invalid_utf8() {
        let mut cursor = Cursor::new(vec![
            b'S', b'T', b'R', b'E', b'A', b'M', b'|', b'1', b'2', b'7', b'.', b'0', b'.', b'0', b'.', b'1',
            b':', b'9', b'0', b'0', b'0', b'|', 0xFF, b'\n',
        ]);

        let err = StreamCommand::try_read_from_reader(&mut cursor).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("Invalid UTF-8 data"));
    }

    #[test]
    fn try_read_from_reader_returns_error_for_invalid_fields_count() {
        let mut cursor = Cursor::new(b"STREAM|127.0.0.1:9000\n".to_vec());

        let err = StreamCommand::try_read_from_reader(&mut cursor).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("Invalid data format"));
    }

    #[test]
    fn try_read_from_reader_returns_error_for_invalid_command_name() {
        let mut cursor = Cursor::new(b"PING|127.0.0.1:9000|AAPL\n".to_vec());

        let err = StreamCommand::try_read_from_reader(&mut cursor).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("Invalid command"));
    }

    #[test]
    fn try_read_from_reader_returns_error_for_invalid_socket_address() {
        let mut cursor = Cursor::new(b"STREAM|invalid-address|AAPL\n".to_vec());

        let err = StreamCommand::try_read_from_reader(&mut cursor).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("Invalid socket address format"));
    }

    #[test]
    fn try_read_from_reader_returns_error_for_empty_quotes_list() {
        let mut cursor = Cursor::new(b"STREAM|127.0.0.1:9000|   \n".to_vec());

        let err = StreamCommand::try_read_from_reader(&mut cursor).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("Empty quotes list"));
    }

    #[test]
    fn try_read_from_reader_returns_error_when_quotes_have_empty_item() {
        let mut cursor = Cursor::new(b"STREAM|127.0.0.1:9000|AAPL,,TSLA\n".to_vec());

        let err = StreamCommand::try_read_from_reader(&mut cursor).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains("empty quote"));
    }
}
