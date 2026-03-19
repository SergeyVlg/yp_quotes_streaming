use clap::Parser;
use log::{debug, info, trace};
use shared::{AckResponse, StockQuote, StreamCommand, PING_COMMAND};
use std::io::{BufRead, BufReader, Error, Write};
use std::io::ErrorKind::ConnectionAborted;
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

const CLIENT_ADDRESS: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
const QUOTES_BUFFER_SIZE: usize = 4096;
const PING_TIMEOUT: Duration = Duration::from_secs(2);

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
    env_logger::init();

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

    let ack = AckResponse::try_read_from_reader(&mut reader)?;
    let ping_address = ack.source_address;

    let address = SocketAddrV4::new(CLIENT_ADDRESS, args.udp_port);
    let Ok(socket) = UdpSocket::bind(address) else {
        return Err(Error::new(std::io::ErrorKind::ConnectionRefused, format!("Failed to bind UDP socket to {}", address)));
    };

    info!("UDP socket opened");

    let cloned_socket = socket.try_clone()?;
    let join_handle = thread::spawn(move || receive_quotes(socket));
    let ping_handle = thread::spawn(move || ping_server(cloned_socket, ping_address));

    match join_handle.join() {
        Ok(Err(e)) => return Err(Error::new(std::io::ErrorKind::ConnectionRefused, format!("Получение котировок завершилось с ошибкой: {}", e))),
        Err(_) =>  return Err(Error::new(std::io::ErrorKind::Other,"Поток получения котировок запаниковал")),
        _ => {},
    };

    match ping_handle.join() {
        Ok(Err(e)) => return Err(Error::new(std::io::ErrorKind::ConnectionRefused, format!("Пинг сервера завершился с ошибкой: {}", e))),
        Err(_) =>  return Err(Error::new(std::io::ErrorKind::Other,"Поток пинга сервера запаниковал")),
        _ => {},
    }

    Ok(())
}

fn receive_quotes(socket: UdpSocket) -> std::io::Result<()> {
    let mut buf = [0u8; QUOTES_BUFFER_SIZE];

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, _)) => {
                StockQuote::try_deserialize(&buf[..size])
                    .map(|quotes| {
                        debug!("Received quotes data");
                        quotes.iter().for_each(|quote| debug!("{:?}", quote));
                    })?
            }
            Err(e) => {
                return Err(Error::new(ConnectionAborted, format!("Failed to receive UDP packet: {}", e)));
            }
        }
    }
}

fn ping_server(socket: UdpSocket, server_addr: SocketAddrV4) -> std::io::Result<()> {
    loop {
        trace!("Send ping to server at address {} ...", server_addr);
        socket.send_to(PING_COMMAND.as_bytes(), server_addr)?;

        thread::sleep(PING_TIMEOUT);
    }
}
