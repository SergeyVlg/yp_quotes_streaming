use std::time::{SystemTime, UNIX_EPOCH};
use crate::stock_quote::StockQuote;

pub struct QuoteGenerator;

impl QuoteGenerator {

    pub fn generate_quote(&self, ticker: &str) -> StockQuote {
        let last_price: f64 = match ticker {
            "AAPL" => 150.0 + rand::random::<f64>() * 10.0,
            "MSFT" => 300.0 + rand::random::<f64>() * 30.0,
            "TSLA" => 700.0 + rand::random::<f64>() * 50.0,

            _ => rand::random::<f64>() * 2.0,
        };

        let volume = match ticker {
            "AAPL" | "MSFT" | "TSLA" => 1000 + (rand::random::<u32>() * 50),

            _ => 100 + (rand::random::<u32>() * 10),
        };

        StockQuote {
            ticker: ticker.to_string(),
            price: last_price,
            volume,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64, //убрать unwrap
        }
    }
}