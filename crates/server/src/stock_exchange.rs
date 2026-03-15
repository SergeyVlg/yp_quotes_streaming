use std::collections::HashMap;
use shared::StockQuote;
use crate::quote_generator::QuoteGenerator;

pub(super) struct StockExchange {
    pub(super) quotes: HashMap<String, StockQuote>,
}

impl StockExchange {
    pub(super) fn new(tickers: Vec<String>) -> Self {
        let mut stock_quotes = HashMap::new();

        for ticker in tickers {
            stock_quotes.insert(ticker.to_string(), StockQuote {
                ticker: ticker.to_string(),
                price: 0.0,
                volume: 0,
                timestamp: 0,
            });
        }

        Self { quotes: stock_quotes }
    }

    pub(super) fn update_quotes(&mut self, generator: &QuoteGenerator) {
        for (ticker, quote) in self.quotes.iter_mut() {
            *quote = generator.generate_quote(ticker);
        }
    }
}