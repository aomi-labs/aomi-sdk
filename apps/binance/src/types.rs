use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BinanceSymbolPrice {
    pub(crate) symbol: String,
    pub(crate) price: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum BinancePriceResponse {
    Single(BinanceSymbolPrice),
    Many(Vec<BinanceSymbolPrice>),
}

impl BinancePriceResponse {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn first(&self) -> Option<&BinanceSymbolPrice> {
        match self {
            Self::Single(item) => Some(item),
            Self::Many(items) => items.first(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BinanceDepthResponse {
    #[serde(rename = "lastUpdateId")]
    pub(crate) last_update_id: u64,
    pub(crate) bids: Vec<BinanceOrderBookLevel>,
    pub(crate) asks: Vec<BinanceOrderBookLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct BinanceOrderBookLevel(pub(crate) Vec<String>);

impl BinanceOrderBookLevel {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn price(&self) -> Option<&str> {
        self.0.first().map(String::as_str)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn qty(&self) -> Option<&str> {
        self.0.get(1).map(String::as_str)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct BinanceKline(pub(crate) Vec<Value>);

impl BinanceKline {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn open(&self) -> Option<&str> {
        self.0.get(1).and_then(Value::as_str)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn high(&self) -> Option<&str> {
        self.0.get(2).and_then(Value::as_str)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn low(&self) -> Option<&str> {
        self.0.get(3).and_then(Value::as_str)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn close(&self) -> Option<&str> {
        self.0.get(4).and_then(Value::as_str)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn volume(&self) -> Option<&str> {
        self.0.get(5).and_then(Value::as_str)
    }
}

pub(crate) type BinanceKlineResponse = Vec<BinanceKline>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Binance24hrStats {
    pub(crate) symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) price_change_percent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last_price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) high_price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) low_price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) volume: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Binance24hrStatsResponse {
    Single(Binance24hrStats),
    Many(Vec<Binance24hrStats>),
}

impl Binance24hrStatsResponse {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn first(&self) -> Option<&Binance24hrStats> {
        match self {
            Self::Single(item) => Some(item),
            Self::Many(items) => items.first(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BinanceOrderResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) order_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BinanceAccountResponse {
    #[serde(default)]
    pub(crate) balances: Vec<BinanceBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BinanceBalance {
    pub(crate) asset: String,
    pub(crate) free: String,
    pub(crate) locked: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BinanceTrade {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) qty: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) quote_qty: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) time: Option<u64>,
}

pub(crate) type BinanceTradeList = Vec<BinanceTrade>;
