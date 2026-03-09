use std::collections::HashMap;
use crate::quote_generator::QuoteGenerator;
use crate::stock_quote::StockQuote;

pub(super) struct StockExchange {
    pub(super) quotes: HashMap<String, StockQuote>,
}

impl StockExchange {
    pub(super) fn new(quotes: String) -> Self {
        let mut stock_quotes = HashMap::new();

        for quote_str in quotes.split(',') {
            stock_quotes.insert(quote_str.to_string(), StockQuote {
                ticker: quote_str.to_string(),
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