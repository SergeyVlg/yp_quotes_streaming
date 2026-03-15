use clap::Parser;
use shared::{StockQuote, StreamCommand};
use std::io::{BufRead, BufReader, Error, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::thread;

const CLIENT_ADDRESS: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);

#[derive(Parser, Debug)]
struct Arguments {
    #[arg(long = "server-addr", value_parser = parse_socket_addr)]
    server_addr: SocketAddrV4,

    #[arg(long = "udp-port")]
    udp_port: u16,

    #[arg(long = "tickers-file", value_parser = parse_existing_file)]
    tickers_file: PathBuf,
}

fn parse_socket_addr(raw_addr: &str) -> Result<SocketAddrV4, String> {
    let addr: SocketAddrV4 = raw_addr.parse()
        .map_err(|_| "Ожидается адрес в формате `127.0.0.1:5678`".to_string())?;

    Ok(addr)
}

fn parse_existing_file(raw_path: &str) -> Result<PathBuf, String> {
    let path_buf = PathBuf::from(raw_path);
    let path: &Path = &path_buf;

    match Path::new(raw_path).try_exists() {
        Ok(true) => {},
        Ok(false) => return Err("Путь к файлу не существует".to_string()),
        Err(e) => return Err(format!("Ошибка в пути файла: {}", e)),
    }

    if !path.is_file() {
        return Err(format!("Это не файл: {}", path.display()));
    }

    Ok(path_buf)
}

fn main() -> std::io::Result<()> {
    let args = Arguments::parse();

    let tickets_file = std::fs::File::open(&args.tickers_file)?;
    let reader = BufReader::new(tickets_file);
    let tickers: Vec<String> = reader.lines()
        .filter_map(|ticket| ticket.ok())
        .collect();

    if tickers.is_empty() {
        return Err(Error::new(std::io::ErrorKind::InvalidData, format!("Файл с тикерами пустой: {}", args.tickers_file.display())));
    }

    let mut stream = TcpStream::connect(args.server_addr)?;
    let mut reader = BufReader::new(stream.try_clone()?);

    // Читаем приветствие
    let mut server_response = String::new();
    reader.read_line(&mut server_response)?;
    print!("{}", server_response);

    let command = StreamCommand::new(CLIENT_ADDRESS, args.udp_port, tickers);
    stream.write_all(&command.to_bytes())?;
    stream.flush()?;

    server_response.clear();
    reader.read_line(&mut server_response)?;

    if server_response.trim() != "Ack" {
        return Err(Error::new(std::io::ErrorKind::InvalidData, format!("Ожидалось подтверждение от сервера, получено: {}", server_response.trim())));
    }

    let join_handle = thread::spawn(move || {
        let address = SocketAddrV4::new(CLIENT_ADDRESS, args.udp_port);
        let Ok(socket) = UdpSocket::bind(address) else {
            eprintln!("Failed to bind UDP socket");
            return ();
        };

        println!("UDP socket opened");
        let mut buf = [0u8; 4096];

        loop {
            match socket.recv_from(&mut buf) {
                Ok((size, src)) => {
                    StockQuote::try_deserialize(&buf[..size])
                        .map(|quotes| {
                            println!("Received quotes data:");
                            quotes.iter().for_each(|quote| println!("{:?}", quote));
                        })
                        .unwrap_or_else(|e| {
                            eprintln!("Failed to deserialize stock quotes: {}", e);
                        });
                }
                Err(e) => {
                    eprintln!("Failed to receive UDP packet: {}", e);
                    break;
                }
            }
        }
    });

    let _ = join_handle.join();

    Ok(())
}
