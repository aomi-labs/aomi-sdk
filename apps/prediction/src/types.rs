use aomi_sdk::schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SimmerRegisterRequest {
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SimmerTradeRequest {
    pub(crate) market_id: String,
    pub(crate) side: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) shares: Option<f64>,
    pub(crate) venue: String,
    pub(crate) action: String,
    pub(crate) source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dry_run: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Eip712TypeField {
    pub(crate) name: &'static str,
    #[serde(rename = "type")]
    pub(crate) kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClobAuthDomain {
    pub(crate) name: &'static str,
    pub(crate) version: &'static str,
    #[serde(rename = "chainId")]
    pub(crate) chain_id: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClobAuthMessage {
    pub(crate) address: String,
    pub(crate) timestamp: String,
    pub(crate) nonce: u64,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClobAuthTypedData {
    pub(crate) domain: ClobAuthDomain,
    pub(crate) types: ClobAuthTypes,
    #[serde(rename = "primaryType")]
    pub(crate) primary_type: &'static str,
    pub(crate) message: ClobAuthMessage,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClobAuthTypes {
    #[serde(rename = "EIP712Domain")]
    pub(crate) eip712_domain: Vec<Eip712TypeField>,
    #[serde(rename = "ClobAuth")]
    pub(crate) clob_auth: Vec<Eip712TypeField>,
}

// Request body for POST /order. `extra_fields` is intentionally `flatten`ed to allow
// callers to merge additional top-level keys into the request payload.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SubmitOrderHttpBody {
    pub(crate) owner: String,
    pub(crate) signature: String,
    pub(crate) order: Value,
    #[serde(rename = "clientId", skip_serializing_if = "Option::is_none")]
    pub(crate) client_id: Option<String>,
    #[serde(flatten, skip_serializing_if = "Map::is_empty")]
    pub(crate) extra_fields: Map<String, Value>,
}

fn deserialize_stringish_vec(value: Value) -> Option<Vec<String>> {
    match value {
        Value::Array(items) => Some(
            items
                .into_iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect(),
        ),
        Value::String(raw) => serde_json::from_str::<Vec<String>>(&raw)
            .ok()
            .or_else(|| Some(vec![raw])),
        _ => None,
    }
}

pub(crate) fn deserialize_optional_string_list<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(deserialize_stringish_vec))
}

pub(crate) fn deserialize_optional_f64ish<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|raw| raw.parse::<f64>().ok()))
    }))
}

fn string_value(value: &Option<String>) -> Option<Value> {
    value.clone().map(Value::String)
}

fn bool_value(value: Option<bool>) -> Option<Value> {
    value.map(Value::Bool)
}

fn f64_value(value: Option<f64>) -> Option<Value> {
    serde_json::Number::from_f64(value?).map(Value::Number)
}

fn i64_value(value: Option<i64>) -> Option<Value> {
    Some(Value::Number(value?.into()))
}

fn string_list_value(value: &Option<Vec<String>>) -> Option<Value> {
    value
        .clone()
        .map(|items| Value::Array(items.into_iter().map(Value::String).collect()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PredictionTokenInfo {
    #[serde(default, alias = "token_id", alias = "tokenId", alias = "id")]
    pub(crate) token_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) outcome: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PredictionMarket {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) question: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) condition_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string_list")]
    pub(crate) outcomes: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_string_list")]
    pub(crate) outcome_prices: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_string_list")]
    pub(crate) clob_token_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) volume: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_f64ish")]
    pub(crate) volume_num: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) liquidity: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_f64ish")]
    pub(crate) liquidity_num: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) start_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) end_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) active: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) closed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) archived: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) market_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tokens: Option<Vec<PredictionTokenInfo>>,
}

impl PredictionMarket {
    pub(crate) fn get(&self, key: &str) -> Option<Value> {
        match key {
            "id" => string_value(&self.id),
            "question" => string_value(&self.question),
            "slug" => string_value(&self.slug),
            "conditionId" | "condition_id" => string_value(&self.condition_id),
            "description" => string_value(&self.description),
            "outcomes" => string_list_value(&self.outcomes),
            "outcomePrices" | "outcome_prices" => string_list_value(&self.outcome_prices),
            "clobTokenIds" | "clob_token_ids" => string_list_value(&self.clob_token_ids),
            "volume" => string_value(&self.volume),
            "volumeNum" | "volume_num" => f64_value(self.volume_num),
            "liquidity" => string_value(&self.liquidity),
            "liquidityNum" | "liquidity_num" => f64_value(self.liquidity_num),
            "startDate" | "start_date" => string_value(&self.start_date),
            "endDate" | "end_date" => string_value(&self.end_date),
            "image" => string_value(&self.image),
            "active" => bool_value(self.active),
            "closed" => bool_value(self.closed),
            "archived" => bool_value(self.archived),
            "category" => string_value(&self.category),
            "marketType" | "market_type" => string_value(&self.market_type),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PredictionTrade {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) market: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) asset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) side: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) price: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) timestamp: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) transaction_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) outcome: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) proxy_wallet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) condition_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) icon: Option<String>,
}

impl PredictionTrade {
    pub(crate) fn get(&self, key: &str) -> Option<Value> {
        match key {
            "id" => string_value(&self.id),
            "market" => string_value(&self.market),
            "asset" => string_value(&self.asset),
            "side" => string_value(&self.side),
            "size" => f64_value(self.size),
            "price" => f64_value(self.price),
            "timestamp" => i64_value(self.timestamp),
            "transactionHash" | "transaction_hash" => string_value(&self.transaction_hash),
            "outcome" => string_value(&self.outcome),
            "proxyWallet" | "proxy_wallet" => string_value(&self.proxy_wallet),
            "conditionId" | "condition_id" => string_value(&self.condition_id),
            "title" => string_value(&self.title),
            "slug" => string_value(&self.slug),
            "icon" => string_value(&self.icon),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SimmerRegisterResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) claim_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) claim_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) starting_balance: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) limits: Option<Value>,
}

impl SimmerRegisterResponse {
    pub(crate) fn get(&self, key: &str) -> Option<Value> {
        match key {
            "agent_id" => string_value(&self.agent_id),
            "api_key" => string_value(&self.api_key),
            "claim_code" => string_value(&self.claim_code),
            "claim_url" => string_value(&self.claim_url),
            "starting_balance" => self.starting_balance.clone(),
            "limits" => self.limits.clone(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SimmerAgentStatusResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sim_balance: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "balance_usdc")]
    pub(crate) balance_usdc: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) real_trading_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) claim_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) limits: Option<Value>,
}

impl SimmerAgentStatusResponse {
    pub(crate) fn get(&self, key: &str) -> Option<Value> {
        match key {
            "agent_id" => string_value(&self.agent_id),
            "name" => string_value(&self.name),
            "status" => string_value(&self.status),
            "sim_balance" => self.sim_balance.clone(),
            "balance_usdc" => self.balance_usdc.clone(),
            "real_trading_enabled" => bool_value(self.real_trading_enabled),
            "claim_url" => string_value(&self.claim_url),
            "limits" => self.limits.clone(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SimmerBriefingResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) portfolio: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) positions: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) opportunities: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) risk_alerts: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) performance: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) checked_at: Option<Value>,
}

impl SimmerBriefingResponse {
    pub(crate) fn get(&self, key: &str) -> Option<Value> {
        match key {
            "portfolio" => self.portfolio.clone(),
            "positions" => self.positions.clone(),
            "opportunities" => self.opportunities.clone(),
            "risk_alerts" => self.risk_alerts.clone(),
            "performance" => self.performance.clone(),
            "checked_at" => self.checked_at.clone(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SimmerMarketContextResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) market: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) position: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) warnings: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) slippage_estimate: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) time_to_resolution: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) resolution_criteria: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) is_paid: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fee_rate_bps: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fee_note: Option<Value>,
}

impl SimmerMarketContextResponse {
    pub(crate) fn get(&self, key: &str) -> Option<Value> {
        match key {
            "market" => self.market.clone(),
            "position" => self.position.clone(),
            "warnings" => self.warnings.clone(),
            "slippage_estimate" => self.slippage_estimate.clone(),
            "time_to_resolution" => self.time_to_resolution.clone(),
            "resolution_criteria" => self.resolution_criteria.clone(),
            "is_paid" => self.is_paid.clone(),
            "fee_rate_bps" => self.fee_rate_bps.clone(),
            "fee_note" => self.fee_note.clone(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SimmerTradeResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) trade_id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) market_id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) side: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) shares_bought: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) shares_sold: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cost: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) average_price: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) venue: Option<Value>,
}

impl SimmerTradeResponse {
    pub(crate) fn get(&self, key: &str) -> Option<Value> {
        match key {
            "trade_id" => self.trade_id.clone(),
            "market_id" => self.market_id.clone(),
            "side" => self.side.clone(),
            "shares_bought" => self.shares_bought.clone(),
            "shares_sold" => self.shares_sold.clone(),
            "cost" => self.cost.clone(),
            "average_price" => self.average_price.clone(),
            "venue" => self.venue.clone(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SimmerPosition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) pnl: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SimmerPositionsResponse {
    #[serde(default)]
    pub(crate) positions: Vec<SimmerPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SimmerPortfolioResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) balance: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) currency: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) positions_value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) total_value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) realized_pnl: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) unrealized_pnl: Option<Value>,
}

impl SimmerPortfolioResponse {
    pub(crate) fn get(&self, key: &str) -> Option<Value> {
        match key {
            "balance" => self.balance.clone(),
            "currency" => self.currency.clone(),
            "positions_value" => self.positions_value.clone(),
            "total_value" => self.total_value.clone(),
            "realized_pnl" => self.realized_pnl.clone(),
            "unrealized_pnl" => self.unrealized_pnl.clone(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SimmerMarketsResponse {
    #[serde(default)]
    pub(crate) markets: Vec<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PredictionCredentialFields {
    #[serde(default, alias = "apiKey", alias = "api_key", alias = "key")]
    pub(crate) key: Option<String>,
    #[serde(default, alias = "secret", alias = "apiSecret", alias = "api_secret")]
    pub(crate) secret: Option<String>,
    #[serde(
        default,
        alias = "passphrase",
        alias = "apiPassphrase",
        alias = "api_passphrase"
    )]
    pub(crate) passphrase: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PredictionCredentialApiResponse {
    #[serde(flatten)]
    pub(crate) fields: PredictionCredentialFields,
    #[serde(default)]
    pub(crate) data: Option<PredictionCredentialFields>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PredictionOrderSubmissionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) success: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "orderID", alias = "order_id")]
    pub(crate) order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) error_msg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) making_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) taking_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) transaction_hashes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) trade_ids: Option<Vec<String>>,
}
