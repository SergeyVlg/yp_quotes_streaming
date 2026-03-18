mod quote_generator;
mod stock_exchange;

use crate::quote_generator::QuoteGenerator;
use crate::stock_exchange::StockExchange;
use crossbeam_channel::{bounded, Sender, TrySendError};
use parking_lot::RwLock;
use shared::{AckResponse, StockQuote, StreamCommand, PING_COMMAND};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool};
use std::sync::Arc;
use std::time::Duration;
use std::{io, thread};
use std::io::ErrorKind::InvalidData;

type SenderInfo = Sender<Arc<HashMap<String, StockQuote>>>;
type Subscribers = Arc<RwLock<Vec<SenderInfo>>>;

const TICKERS_FILE: &str = "tickers.txt";
const QUOTES_UPDATE_DURATION: Duration = Duration::from_secs(5);
const ADDRESS: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 7878);
const CHANNEL_CAPACITY: usize = 20;

const PING_COMMAND_SIZE: usize = 16;
const TCP_READ_TIMEOUT: Duration = Duration::from_secs(5);
const UDP_TIMEOUT: Duration = Duration::from_secs(10);

fn main() -> Result<(), io::Error> {
    let tickers = read_tickers(TICKERS_FILE)?;
    let stock_exchange = StockExchange::new(tickers);

    let generator = QuoteGenerator;
    let subscribers: Subscribers = Arc::new(RwLock::new(Vec::new()));
    let subscribers_to_write = Arc::clone(&subscribers);

    //Поток обновления котировок - только он может записывать данные в биржу, а клиенты только читают
    thread::spawn(move || {
        update_quotes(stock_exchange, &generator, subscribers_to_write);
    });

    let listener = TcpListener::bind(ADDRESS)?;
    println!("Server listening {}", ADDRESS);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let subscribers = Arc::clone(&subscribers);
                let mut cloned_stream = stream.try_clone()?;

                thread::spawn(move || {
                    let _ = handle_client(stream, subscribers)
                        .map_err(move |e| {
                            let _ = cloned_stream.write_all(format!("{}", e).as_bytes());
                            let _ = cloned_stream.flush();
                            eprintln!("Error handling client: {}", e);
                        });
                });
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

fn read_tickers(path: &str) -> io::Result<Vec<String>> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    let mut tickers = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            tickers.push(trimmed.to_string());
        }
    }

    if tickers.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Файл с тикерами пустой: {}", path)));
    }

    Ok(tickers)
}

fn update_quotes(mut stock_exchange: StockExchange, generator: &QuoteGenerator, subscribers: Subscribers) {
    loop {
        stock_exchange.update_quotes(&generator);
        let snapshot = Arc::new(stock_exchange.quotes.clone());

        // Блок для записи в subscribers, чтобы не держать блокировку на весь цикл
        {
            let mut subs = subscribers.write();

            subs.retain(|sender| {
                let snapshot = Arc::clone(&snapshot);

                match sender.try_send(snapshot) {
                    Ok(_) => true, // Клиент получил обновление, оставляем его в списке
                    Err(TrySendError::Full(_)) => true, // Канал полон, пропускаем такт
                    Err(_) => false, // Клиент отключился, удаляем его из списка
                }
            });
        }

        thread::sleep(QUOTES_UPDATE_DURATION);
    }
}

// Логика обработки подключившегося по TCP клиента
fn handle_client(stream: TcpStream, subscribers: Subscribers) -> io::Result<()> {
    println!("Connection from {}", stream.peer_addr()?);

    let _ = stream.set_read_timeout(Some(TCP_READ_TIMEOUT));

    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);

    let _ = writer.write_all(b"Welcome to streaming quotes server! Awaiting command...\n");
    let _ = writer.flush();

    let command = StreamCommand::try_read_from_reader(&mut reader)?;
    if command.quotes.is_empty() {
        return Err(io::Error::new(InvalidData, "Quotes list in command is empty"));
    }

    println!("Parsed command: {:?}", command);

    let socket = UdpSocket::bind("127.0.0.1:0")?; //Здесь не очень хорошо, что на клиент будут отправляться внутренние ошибки сервера, но пока ради упрощения решил оставить так
    let source_address = match socket.local_addr()? {
        SocketAddr::V4(addr) => addr,
        SocketAddr::V6(_) => return Err(io::Error::new(InvalidData,"UDP socket is not IPv4")),
    };

    let ack_response = AckResponse { source_address };

    let _ = writer.write_all(&ack_response.to_bytes());
    let _ = writer.flush();

    thread::spawn(move || {
        let _ = process_udp_streaming(subscribers, command, socket);
    });

    Ok(())
}

fn process_udp_streaming(subscribers: Subscribers, command: StreamCommand, socket: UdpSocket) -> io::Result<()> {
    let socket_clone = socket.try_clone()?;

    let (sender, receiver) = bounded::<Arc<HashMap<String, StockQuote>>>(CHANNEL_CAPACITY);
    {
        let mut subscribers_lock = subscribers.write();
        subscribers_lock.push(sender);
    }

    let is_client_detached = Arc::new(AtomicBool::new(false));
    let is_detached_clone = Arc::clone(&is_client_detached);

    thread::spawn(move || {
        listen_client_ping(socket_clone, is_detached_clone);
    });

    println!("Start processing UDP streaming...");

    loop {
        match receiver.recv() {
            Ok(_) if is_client_detached.load(Relaxed) => break,
            Ok(all_quotes) => {

                let filtered_quotes: Vec<StockQuote> = command.quotes.iter()
                    .filter_map(|required_quote| all_quotes.get(required_quote).cloned())
                    .collect();

                let payload_bytes: Vec<u8> = StockQuote::serialize(&filtered_quotes);

                println!("Send quotes to {}: {:?}", command.address, filtered_quotes);

                if let Err(e) = socket.send_to(&payload_bytes, command.address) {
                    eprintln!("Failed to send UDP packet: {}", e);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Failed to receive quotes from channel: {}", e);
                break;
            }
        }
    }

    Ok(())
}

fn listen_client_ping(socket: UdpSocket, is_client_detached: Arc<AtomicBool>) {
    let mut buf = [0u8; PING_COMMAND_SIZE];
    let _ = socket.set_read_timeout(Some(UDP_TIMEOUT));

    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, _)) => {
                let msg = String::from_utf8_lossy(&buf[..size]);

                if msg.trim() != PING_COMMAND {
                    eprintln!("Received invalid PING message: {}", msg);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Failed to read from socket: {}", e);
                break;
            }
        }
    }

    println!("Client is detached, stopping UDP streaming...");
    is_client_detached.store(true, Relaxed)
}