mod quote_generator;
mod stock_exchange;
mod stock_quote;

use crate::stock_exchange::StockExchange;
use crate::stock_quote::StockQuote;
use crossbeam_channel::{bounded, Sender, TrySendError};
use parking_lot::RwLock;
use shared::StreamCommand;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use std::{io, thread};

type SenderInfo = Sender<Arc<HashMap<String, StockQuote>>>;
type Subscribers = Arc<RwLock<Vec<SenderInfo>>>;

fn main() -> Result<(), io::Error> {
    //Поток обновления котировок - только он может записывать данные в биржу, а клиенты только читают
    let mut stock_exchange = StockExchange::new("AAPL,MSFT,TSLA".to_string()); //read file with quotes
    let generator = quote_generator::QuoteGenerator;
    let subscribers: Subscribers = Arc::new(RwLock::new(Vec::new()));
    let subscribers_to_write = Arc::clone(&subscribers);

    thread::spawn(move || {
        loop {
            stock_exchange.update_quotes(&generator);
            let snapshot = Arc::new(stock_exchange.quotes.clone());

            { // Блок для записи в subscribers, чтобы не держать блокировку на весь цикл
                let mut subs = subscribers_to_write.write();

                subs.retain(|sender| {
                    //возможно, стоит вынести отправку через каналы в отдельный поток

                    // Фильтруем котировки по запрошенным тикерам
                    //let payload: Vec<StockQuote> = requested_quotes.iter().map(|q| snapshot.get(q).cloned()).flatten().collect();
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
    });

    const ADDRESS: &str = "127.0.0.1:7878";

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

// Логика обработки клиента
fn handle_client(stream: TcpStream, subscribers: Subscribers) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));

    let mut writer = stream.try_clone().expect("failed to clone stream"); //убрать expect, завершать функцию с логом ошибки через логгер
    let mut reader = BufReader::new(stream);

    let _ = writer.write_all(b"Start streaming quotes!\n");
    let _ = writer.flush();

    match read_command_bounded(&mut reader, StreamCommand::MAX_COMMAND_SIZE) {
        Ok(bytes) => {
            match StreamCommand::try_from_bytes(&bytes) {
                Ok(command) => {
                    println!("Parsed command: {:?}", command);

                    thread::spawn(move || {
                        let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else {
                            eprintln!("Failed to bind UDP socket");
                            return;
                        };

                        let (sender, receiver) = bounded::<Arc<HashMap<String, StockQuote>>>(20);
                        {
                            let mut subscribers_lock = subscribers.write();
                            subscribers_lock.push(sender);
                        }

                        loop {
                            //todo необходимо удалять отключившихся клиентов из subscribers
                            match receiver.recv() {
                                Ok(all_quotes) => {
                                    // Фильтруем котировки по запрошенным тикерам
                                    let payload: Vec<StockQuote> = command.quotes.iter().filter_map(|required_quote| all_quotes.get(required_quote).cloned()).collect();

                                    //здесь нужно сериализовывать котировки в поток байт с разделителем между котировками, типа |
                                    let payload_bytes = payload.iter().flat_map(|quote| quote.to_bytes()).collect::<Vec<u8>>();

                                    if let Err(e) = socket.send_to(&payload_bytes, command.get_socket_address()) {
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
                    });
                }
                Err(e) => {
                    let _ = writer.write_all(format!("Error parsing command: {}\n", e).as_bytes());
                    let _ = writer.flush();

                    return; // Закрываем соединение после отправки ошибки
                }
            }
        }
        Err(_) => {}
    }
}

fn read_command_bounded(reader: &mut BufReader<TcpStream>, max_command_size: u64) -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(256);

    let n = reader
        .take(max_command_size + 1)
        .read_until(b'\n', &mut buf)?;

    if n == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed"));
    }

    if buf.len() > max_command_size as usize {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "command too long"));
    }

    if !buf.ends_with(b"\n") {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "newline not found"));
    }

    if matches!(buf.last(), Some(b'\n')) {
        buf.pop();
    }
    if matches!(buf.last(), Some(b'\r')) {
        buf.pop();
    }

    Ok(buf)
}
