mod quote_generator;
mod stock_exchange;

use crate::stock_exchange::StockExchange;
use crossbeam_channel::{bounded, Sender, TrySendError};
use parking_lot::RwLock;
use shared::{StockQuote, StreamCommand};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use std::{io, thread};
use crate::quote_generator::QuoteGenerator;

type SenderInfo = Sender<Arc<HashMap<String, StockQuote>>>;
type Subscribers = Arc<RwLock<Vec<SenderInfo>>>;

const TICKERS_FILE: &str = "tickers.txt";
const ADDRESS: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 7878);
const CHANNEL_CAPACITY: usize = 20;

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

                thread::spawn(move || {
                    let _ = handle_client(stream, subscribers);
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

fn update_quotes(mut stock_exchange: StockExchange, generator: &QuoteGenerator, subscribers: Arc<RwLock<Vec<SenderInfo>>>) {
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

        thread::sleep(Duration::from_secs(5));
    }
}

// Логика обработки подключившегося по TCP клиента
fn handle_client(stream: TcpStream, subscribers: Subscribers) {
    println!("Connection from {}", stream.peer_addr().unwrap());

    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));

    let mut writer = stream.try_clone().expect("failed to clone stream"); //убрать expect, завершать функцию с логом ошибки через логгер
    let mut reader = BufReader::new(stream);

    let _ = writer.write_all(b"Welcome to streaming quotes server! Awaiting command...\n");
    let _ = writer.flush();

    match StreamCommand::try_read_from_reader(&mut reader) {
        Ok(command) => {
            println!("Parsed command: {:?}", command);

            let _ = writer.write_all(b"Ack\n");
            let _ = writer.flush();

            thread::spawn(move || {
                process_udp_streaming(subscribers, command);
            });
        }
        Err(e) => {
            eprintln!("Error parsing command: {}\n", e);

            let _ = writer.write_all(format!("Error parsing command: {}\n", e).as_bytes());
            let _ = writer.flush();
        }
    }
}

fn process_udp_streaming(subscribers: Subscribers, command: StreamCommand) {
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else {
        eprintln!("Failed to bind UDP socket");
        return ();
    };

    let (sender, receiver) = bounded::<Arc<HashMap<String, StockQuote>>>(CHANNEL_CAPACITY);
    {
        let mut subscribers_lock = subscribers.write();
        subscribers_lock.push(sender);
    }

    println!("Start processing UDP streaming...");

    loop {
        match receiver.recv() {
            Ok(all_quotes) => {
                //todo Для отслеживания, активен ли еще клиент, необходима отдельная структура данных, которая будет хранить активных клиентов
                //как только механизм ping/pong сообщает о неактивности клиента, надо удалить его из этой структуры
                //здесь же перед отправкой данных надо проверять, активен ли еще клиент, и если нет, то просто дропнуть receiver.
                //это автоматически удалит клиента из subscribers при следующем такте обновления котировок, в методе вектора retain

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
}