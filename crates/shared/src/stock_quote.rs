#[derive(Debug, Clone)]
pub struct StockQuote {
    pub ticker: String,
    pub price: f64,
    pub volume: u32,
    pub timestamp: u64,
}

impl StockQuote {
    pub fn to_string(&self) -> String {
        format!("{}|{}|{}|{}", self.ticker, self.price, self.volume, self.timestamp)
    }

    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('|').collect();
        if parts.len() == 4 {
            Some(StockQuote {
                ticker: parts[0].to_string(),
                price: parts[1].parse().ok()?,
                volume: parts[2].parse().ok()?,
                timestamp: parts[3].parse().ok()?,
            })
        } else {
            None
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.ticker.as_bytes());
        bytes.push(b'|');
        bytes.extend_from_slice(self.price.to_string().as_bytes());
        bytes.push(b'|');
        bytes.extend_from_slice(self.volume.to_string().as_bytes());
        bytes.push(b'|');
        bytes.extend_from_slice(self.timestamp.to_string().as_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let s = std::str::from_utf8(bytes).ok()?;
        Self::from_string(s)
    }

    pub fn serialize(quotes: &[StockQuote]) -> Vec<u8> {
        let mut bytes = Vec::new();

        quotes.iter().for_each(|quote| {
            bytes.extend_from_slice(&quote.to_bytes());
            bytes.push(b',');
        });

        bytes.pop(); // Удаляем последнюю запятую
        bytes
    }

    pub fn try_deserialize(bytes: &[u8]) -> std::io::Result<Vec<StockQuote>> {
        let Ok(s) = std::str::from_utf8(bytes) else {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8 data"));
        };

        let quotes = s.split(',')
            .filter_map(|quote_str| StockQuote::from_string(quote_str))
            .collect();

        Ok(quotes)
    }
}