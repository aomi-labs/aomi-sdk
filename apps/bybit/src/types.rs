use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateOrderRequest<'a> {
    pub(crate) category: &'a str,
    pub(crate) symbol: &'a str,
    pub(crate) side: &'a str,
    pub(crate) order_type: &'a str,
    pub(crate) qty: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) price: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CancelOrderRequest<'a> {
    pub(crate) category: &'a str,
    pub(crate) symbol: &'a str,
    pub(crate) order_id: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AmendOrderRequest<'a> {
    pub(crate) category: &'a str,
    pub(crate) symbol: &'a str,
    pub(crate) order_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) qty: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) price: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetLeverageRequest<'a> {
    pub(crate) category: &'a str,
    pub(crate) symbol: &'a str,
    pub(crate) buy_leverage: &'a str,
    pub(crate) sell_leverage: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BybitResponse<T> {
    pub(crate) ret_code: i64,
    pub(crate) ret_msg: String,
    pub(crate) result: T,
    #[serde(default)]
    pub(crate) ret_ext_info: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) time: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BybitTickerResult {
    pub(crate) category: String,
    pub(crate) list: Vec<BybitTicker>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BybitTicker {
    pub(crate) symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last_price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) volume24h: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) bid1_price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ask1_price: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BybitOrderbookResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) s: Option<String>,
    #[serde(default)]
    pub(crate) b: Vec<BybitOrderbookLevel>,
    #[serde(default)]
    pub(crate) a: Vec<BybitOrderbookLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct BybitOrderbookLevel(pub(crate) Vec<String>);

impl BybitOrderbookLevel {
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
#[serde(rename_all = "camelCase")]
pub(crate) struct BybitKlineResult {
    pub(crate) category: String,
    pub(crate) symbol: String,
    #[serde(default)]
    pub(crate) list: Vec<BybitKlineCandle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct BybitKlineCandle(pub(crate) Vec<String>);

impl BybitKlineCandle {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn open_time(&self) -> Option<&str> {
        self.0.first().map(String::as_str)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn open(&self) -> Option<&str> {
        self.0.get(1).map(String::as_str)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn high(&self) -> Option<&str> {
        self.0.get(2).map(String::as_str)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn low(&self) -> Option<&str> {
        self.0.get(3).map(String::as_str)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn close(&self) -> Option<&str> {
        self.0.get(4).map(String::as_str)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn volume(&self) -> Option<&str> {
        self.0.get(5).map(String::as_str)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BybitActionResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) order_link_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) list: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BybitPositionListResult {
    pub(crate) category: String,
    #[serde(default)]
    pub(crate) list: Vec<BybitPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BybitPosition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) side: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) avg_price: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BybitWalletBalanceResult {
    #[serde(default)]
    pub(crate) list: Vec<BybitWalletBalanceAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BybitWalletBalanceAccount {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) account_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) total_equity: Option<String>,
    #[serde(default)]
    pub(crate) coin: Vec<BybitWalletCoinBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BybitWalletCoinBalance {
    pub(crate) coin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) wallet_balance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) usd_value: Option<String>,
}
