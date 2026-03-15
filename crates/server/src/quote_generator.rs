use std::time::{SystemTime, UNIX_EPOCH};
use rand::RngExt;
use shared::StockQuote;

pub struct QuoteGenerator;

impl QuoteGenerator {

    pub fn generate_quote(&self, ticker: &str) -> StockQuote {
        let mut rng = rand::rng();

        let (base, spread) = match ticker {
            "AAPL" => (150.0,10.0),
            "MSFT" => (300.0,30.0),
            "TSLA" => (700.0,50.0),
            _ => (0.0,2.0),
        };

        let last_price: f64 = base + rng.random_range(0.0..spread);

        let (base, spread) = match ticker {
            "AAPL" | "MSFT" | "TSLA" => (1000, 50),

            _ => (100, 10),
        };

        let volume: u32 = base + rng.random_range(0..spread);

        StockQuote {
            ticker: ticker.to_string(),
            price: last_price,
            volume,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64, //TODO убрать unwrap
        }
    }
}