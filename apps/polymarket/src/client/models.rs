use aomi_sdk::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct GetMarketsParams {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
    pub(crate) active: Option<bool>,
    pub(crate) closed: Option<bool>,
    pub(crate) archived: Option<bool>,
    pub(crate) tag: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct GetTradesParams {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
    pub(crate) market: Option<String>,
    pub(crate) user: Option<String>,
    pub(crate) side: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Market {
    pub(crate) id: Option<String>,
    pub(crate) question: Option<String>,
    pub(crate) slug: Option<String>,
    pub(crate) condition_id: Option<String>,
    pub(crate) description: Option<String>,
    #[serde(deserialize_with = "deserialize_string_or_array", default)]
    pub(crate) outcomes: Option<Vec<String>>,
    #[serde(deserialize_with = "deserialize_string_or_array", default)]
    pub(crate) outcome_prices: Option<Vec<String>>,
    pub(crate) volume: Option<String>,
    pub(crate) volume_num: Option<f64>,
    pub(crate) liquidity: Option<String>,
    pub(crate) liquidity_num: Option<f64>,
    pub(crate) start_date: Option<String>,
    pub(crate) end_date: Option<String>,
    pub(crate) image: Option<String>,
    pub(crate) active: Option<bool>,
    pub(crate) closed: Option<bool>,
    pub(crate) archived: Option<bool>,
    pub(crate) category: Option<String>,
    pub(crate) market_type: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

pub(crate) fn deserialize_string_or_array<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct StringOrArrayVisitor;

    impl<'de> Visitor<'de> for StringOrArrayVisitor {
        type Value = Option<Vec<String>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or array of strings")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D: serde::Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> Result<Self::Value, D::Error> {
            deserializer.deserialize_any(self)
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            match serde_json::from_str::<Vec<String>>(v) {
                Ok(arr) => Ok(Some(arr)),
                Err(_) => Ok(Some(vec![v.to_string()])),
            }
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut vec = Vec::new();
            while let Some(elem) = seq.next_element()? {
                vec.push(elem);
            }
            Ok(Some(vec))
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }

    deserializer.deserialize_option(StringOrArrayVisitor)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Trade {
    pub(crate) id: Option<String>,
    pub(crate) market: Option<String>,
    pub(crate) asset: Option<String>,
    pub(crate) side: Option<String>,
    pub(crate) size: Option<f64>,
    pub(crate) price: Option<f64>,
    pub(crate) timestamp: Option<i64>,
    pub(crate) transaction_hash: Option<String>,
    pub(crate) outcome: Option<String>,
    pub(crate) proxy_wallet: Option<String>,
    pub(crate) condition_id: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) slug: Option<String>,
    pub(crate) icon: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PolymarketOrderPlan {
    pub(crate) market_id_or_slug: String,
    pub(crate) market_id: Option<String>,
    pub(crate) slug: Option<String>,
    pub(crate) condition_id: Option<String>,
    pub(crate) question: Option<String>,
    pub(crate) close_time: Option<String>,
    pub(crate) token_id: String,
    pub(crate) outcome: String,
    pub(crate) side: String,
    pub(crate) execution_mode: String,
    pub(crate) order_kind: String,
    pub(crate) amount: Option<String>,
    pub(crate) amount_kind: Option<String>,
    pub(crate) price: Option<String>,
    pub(crate) size: Option<String>,
    pub(crate) reference_price: Option<String>,
    pub(crate) estimated_shares: Option<String>,
    pub(crate) order_type: String,
    pub(crate) post_only: bool,
    pub(crate) signature_type: String,
    pub(crate) funder: Option<String>,
    pub(crate) wallet_address: Option<String>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ClobAuthContext {
    pub(crate) address: String,
    pub(crate) timestamp: String,
    pub(crate) nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PreparedPolymarketOrder {
    pub(crate) order: PreparedPolymarketExchangeOrder,
    pub(crate) order_type: String,
    pub(crate) post_only: Option<bool>,
    pub(crate) verifying_contract: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PreparedPolymarketExchangeOrder {
    pub(crate) salt: u64,
    pub(crate) maker: String,
    pub(crate) signer: String,
    pub(crate) taker: String,
    pub(crate) token_id: String,
    pub(crate) maker_amount: String,
    pub(crate) taker_amount: String,
    pub(crate) expiration: String,
    pub(crate) nonce: String,
    pub(crate) fee_rate_bps: String,
    pub(crate) side: String,
    pub(crate) side_index: u8,
    pub(crate) signature_type: u8,
}
