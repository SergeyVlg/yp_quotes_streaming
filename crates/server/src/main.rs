use crate::stock_quote::StockQuote;
use crossbeam_channel::{bounded, Sender, TrySendError};
use parking_lot::RwLock;
use shared::StreamCommand;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use std::{io, thread};
use std::collections::HashMap;
use stock_exchange::StockExchange;

mod quote_generator;
mod stock_exchange;
mod stock_quote;

type SenderInfo = (Sender<Vec<StockQuote>>, Vec<String>);
type Subscribers = Arc<RwLock<Vec<SenderInfo>>>;

fn main() -> Result<(), io::Error> {
    //Поток обновления котировок - только он может записывать данные в биржу, а клиенты только читают
    let mut stock_exchange = StockExchange::new("AAPL,MSFT,TSLA".to_string()); //read file with quotes
    let generator = quote_generator::QuoteGenerator;
    let subscribers: Subscribers = Arc::new(RwLock::new(Vec::new()));

    thread::spawn(move || {
        loop {
            stock_exchange.update_quotes(&generator);
            let snapshot: HashMap<String, StockQuote> = stock_exchange.quotes.clone();

            let mut subs = subscribers.write();
            subs.retain(|sender_info| {
                let (sender, requested_quotes) = sender_info; //возможно, стоит вынести отправку через каналы в отдельный поток

                // Фильтруем котировки по запрошенным тикерам
                let payload: Vec<StockQuote> = requested_quotes.iter().map(|q| snapshot.get(q).cloned()).flatten().collect();

                // Отправляем обновления клиенту, если он все еще подключен
                match sender.try_send(payload) {
                    Ok(_) => true, // Клиент получил обновление, оставляем его в списке
                    Err(TrySendError::Full(_)) => true, // Канал полон, пропускаем такт
                    Err(_) => false, // Клиент отключился, удаляем его из списка
                }
            });

            thread::sleep(Duration::from_secs(5));
        }
    });

    const ADDRESS: &str = "127.0.0.1:7878";

    let listener = TcpListener::bind(ADDRESS)?;
    println!("Server listening {}", ADDRESS);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    let _ = handle_client(stream);
                });
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

// Логика обработки клиента
fn handle_client(stream: TcpStream) {
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
                    // Здесь нужно запустить поток для отправки данных по UDP на указанный адрес и порт
                    // А также передать канал для получения котировок из биржи и фильтрации по запрошенным тикерам
                    // Канал надо создавать в main и передавать в handle_client, а не создавать новый для каждого клиента
                    // Каждому клиенту нужны свои акции, поэтому надо регистрировать клиента в subscribers и отправлять ему обновления через каналы, а не напрямую через UDP

                    let socket = UdpSocket::bind("127.0.0.1")?;




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
