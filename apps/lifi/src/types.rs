use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuoteQuery<'a> {
    pub(crate) from_chain: &'a str,
    pub(crate) to_chain: &'a str,
    pub(crate) from_token: &'a str,
    pub(crate) to_token: &'a str,
    pub(crate) from_amount: &'a str,
    pub(crate) from_address: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) to_address: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) slippage: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatusQuery<'a> {
    pub(crate) tx_hash: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) from_chain: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) to_chain: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bridge: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChainsQuery<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chain_types: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokensQuery<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chains: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chain_types: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TokenQuery<'a> {
    pub(crate) chain: &'a str,
    pub(crate) token: &'a str,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RouteOptions<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) slippage: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) order: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RouteRequest<'a> {
    pub(crate) from_chain_id: u64,
    pub(crate) to_chain_id: u64,
    pub(crate) from_token_address: &'a str,
    pub(crate) to_token_address: &'a str,
    pub(crate) from_amount: &'a str,
    pub(crate) from_address: &'a str,
    pub(crate) options: RouteOptions<'a>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConnectionsQuery<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) from_chain: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) to_chain: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) from_token: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) to_token: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ToolsQuery<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chains: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReverseQuoteQuery<'a> {
    pub(crate) from_chain: &'a str,
    pub(crate) to_chain: &'a str,
    pub(crate) from_token: &'a str,
    pub(crate) to_token: &'a str,
    pub(crate) to_amount: &'a str,
    pub(crate) from_address: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) to_address: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GasSuggestionQuery<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) from_chain: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) from_token: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PreparedTransaction {
    pub(crate) to: Value,
    pub(crate) data: Value,
    pub(crate) value: Value,
    pub(crate) gas_limit: Value,
    pub(crate) description: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct PreparedOrder {
    pub(crate) raw_quote: Value,
    pub(crate) main_tx: PreparedTransaction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) approval_tx: Option<PreparedTransaction>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LifiToolDetails {
    pub(crate) key: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) logo_uri: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LifiTokenRef {
    pub(crate) address: Option<String>,
    pub(crate) symbol: Option<String>,
    pub(crate) decimals: Option<u8>,
    pub(crate) chain_id: Option<u64>,
    pub(crate) name: Option<String>,
    pub(crate) coin_key: Option<String>,
    #[serde(rename = "priceUSD")]
    pub(crate) price_usd: Option<String>,
    pub(crate) logo_uri: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LifiAction {
    pub(crate) from_chain_id: Option<u64>,
    pub(crate) to_chain_id: Option<u64>,
    pub(crate) from_token: Option<LifiTokenRef>,
    pub(crate) to_token: Option<LifiTokenRef>,
    pub(crate) from_amount: Option<String>,
    pub(crate) slippage: Option<f64>,
    pub(crate) from_address: Option<String>,
    pub(crate) to_address: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct LifiCost {
    #[serde(rename = "amountUSD")]
    pub(crate) amount_usd: Option<String>,
}

impl LifiCost {
    pub(crate) fn amount_usd_f64(&self) -> Option<f64> {
        self.amount_usd.as_deref()?.parse::<f64>().ok()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LifiEstimate {
    pub(crate) from_amount: Option<String>,
    pub(crate) to_amount: Option<String>,
    pub(crate) to_amount_min: Option<String>,
    #[serde(rename = "fromAmountUSD")]
    pub(crate) from_amount_usd: Option<String>,
    #[serde(rename = "toAmountUSD")]
    pub(crate) to_amount_usd: Option<String>,
    pub(crate) approval_address: Option<String>,
    pub(crate) execution_duration: Option<u64>,
    #[serde(default)]
    pub(crate) fee_costs: Vec<LifiCost>,
    #[serde(default)]
    pub(crate) gas_costs: Vec<LifiCost>,
    pub(crate) tool: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LifiStep {
    pub(crate) id: Option<String>,
    #[serde(rename = "type")]
    pub(crate) step_type: Option<String>,
    pub(crate) tool: Option<String>,
    pub(crate) tool_details: Option<LifiToolDetails>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LifiTransactionRequest {
    pub(crate) to: Option<Value>,
    pub(crate) data: Option<Value>,
    pub(crate) value: Option<Value>,
    pub(crate) gas_limit: Option<Value>,
    pub(crate) gas: Option<Value>,
    pub(crate) from: Option<Value>,
    pub(crate) gas_price: Option<Value>,
    pub(crate) chain_id: Option<Value>,
}

impl LifiTransactionRequest {
    pub(crate) fn to_prepared_transaction(&self, description: &'static str) -> PreparedTransaction {
        PreparedTransaction {
            to: self.to.clone().unwrap_or(Value::Null),
            data: self
                .data
                .clone()
                .unwrap_or_else(|| Value::String("0x".to_string())),
            value: self
                .value
                .clone()
                .unwrap_or_else(|| Value::String("0".to_string())),
            gas_limit: self
                .gas_limit
                .clone()
                .or_else(|| self.gas.clone())
                .unwrap_or(Value::Null),
            description,
        }
    }

    pub(crate) fn to_executable_transaction(&self) -> ExecutableTransaction {
        ExecutableTransaction {
            to: self.to.clone().unwrap_or(Value::Null),
            data: self.data.clone().unwrap_or(Value::Null),
            value: self.value.clone().unwrap_or(Value::Null),
            gas_limit: self.gas_limit.clone().unwrap_or(Value::Null),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LifiQuoteResponse {
    pub(crate) id: Option<String>,
    #[serde(rename = "type")]
    pub(crate) quote_type: Option<String>,
    pub(crate) tool: Option<String>,
    pub(crate) tool_details: Option<LifiToolDetails>,
    pub(crate) action: Option<LifiAction>,
    pub(crate) estimate: LifiEstimate,
    #[serde(default)]
    pub(crate) included_steps: Vec<LifiStep>,
    pub(crate) integrator: Option<String>,
    pub(crate) transaction_id: Option<String>,
    pub(crate) transaction_request: Option<LifiTransactionRequest>,
}

impl LifiQuoteResponse {
    pub(crate) fn bridge_name(&self) -> String {
        self.tool_details
            .as_ref()
            .and_then(|details| details.name.clone())
            .or_else(|| self.tool.clone())
            .or_else(|| self.estimate.tool.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }

    pub(crate) fn route_summary(&self) -> Vec<String> {
        let mut steps: Vec<String> = self
            .included_steps
            .iter()
            .filter_map(|step| step.tool.clone())
            .collect();
        if steps.is_empty() {
            steps.push(self.bridge_name());
        }
        steps
    }

    pub(crate) fn total_fee_usd(&self) -> Option<String> {
        let total = self
            .estimate
            .fee_costs
            .iter()
            .chain(self.estimate.gas_costs.iter())
            .filter_map(LifiCost::amount_usd_f64)
            .sum::<f64>();
        (total > 0.0).then(|| format!("{total:.2}"))
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ExecutableTransaction {
    pub(crate) to: Value,
    pub(crate) data: Value,
    pub(crate) value: Value,
    pub(crate) gas_limit: Value,
}

#[derive(Debug, Serialize)]
pub(crate) struct BridgeQuoteResponse {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) to_amount_estimate: Option<String>,
    pub(crate) min_received: Option<String>,
    pub(crate) bridge: String,
    pub(crate) estimated_duration_seconds: Option<u64>,
    pub(crate) estimated_fee_usd: Option<String>,
    pub(crate) route_summary: Vec<String>,
    pub(crate) executable_tx: Option<ExecutableTransaction>,
    pub(crate) execution_supported: bool,
    pub(crate) warning: Option<String>,
}

impl BridgeQuoteResponse {
    pub(crate) fn planning_only(
        from: String,
        to: String,
        route_summary: Vec<String>,
        warning: Option<String>,
    ) -> Self {
        Self {
            from,
            to,
            to_amount_estimate: None,
            min_received: None,
            bridge: "planning-only".to_string(),
            estimated_duration_seconds: None,
            estimated_fee_usd: None,
            route_summary,
            executable_tx: None,
            execution_supported: false,
            warning,
        }
    }

    pub(crate) fn from_lifi_quote(
        quote: &LifiQuoteResponse,
        from: String,
        to: String,
        to_decimals: u8,
    ) -> Self {
        let executable_tx = quote
            .transaction_request
            .as_ref()
            .map(LifiTransactionRequest::to_executable_transaction);

        Self {
            from,
            to,
            to_amount_estimate: quote
                .estimate
                .to_amount
                .as_deref()
                .and_then(|value| format_base_units(value, to_decimals)),
            min_received: quote
                .estimate
                .to_amount_min
                .as_deref()
                .and_then(|value| format_base_units(value, to_decimals)),
            bridge: quote.bridge_name(),
            estimated_duration_seconds: quote.estimate.execution_duration,
            estimated_fee_usd: quote.total_fee_usd(),
            route_summary: quote.route_summary(),
            execution_supported: executable_tx.is_some(),
            executable_tx,
            warning: None,
        }
    }
}

fn format_base_units(value: &str, decimals: u8) -> Option<String> {
    value
        .parse::<f64>()
        .ok()
        .map(|raw| raw / 10f64.powi(decimals as i32))
        .map(|amount| format!("{amount:.6}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bridge_quote_response_uses_typed_lifi_quote_shape() {
        let quote: LifiQuoteResponse = serde_json::from_value(json!({
            "type": "lifi",
            "tool": "across",
            "toolDetails": {"name": "Across"},
            "action": {
                "fromToken": {"address": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"}
            },
            "estimate": {
                "toAmount": "9900000",
                "toAmountMin": "9800000",
                "executionDuration": 420,
                "approvalAddress": "0x1111111111111111111111111111111111111111",
                "feeCosts": [{"amountUSD": "0.12"}],
                "gasCosts": [{"amountUSD": "1.38"}]
            },
            "includedSteps": [
                {"tool": "across"},
                {"tool": "uniswap"}
            ],
            "transactionRequest": {
                "to": "0x2222222222222222222222222222222222222222",
                "data": "0xdeadbeef",
                "value": "0x0",
                "gasLimit": "0x12345"
            }
        }))
        .expect("quote should deserialize");

        let response = BridgeQuoteResponse::from_lifi_quote(
            &quote,
            "1 USDC on ethereum".to_string(),
            "USDC on polygon".to_string(),
            6,
        );

        assert_eq!(response.bridge, "Across");
        assert_eq!(response.to_amount_estimate.as_deref(), Some("9.900000"));
        assert_eq!(response.min_received.as_deref(), Some("9.800000"));
        assert_eq!(response.estimated_fee_usd.as_deref(), Some("1.50"));
        assert_eq!(response.route_summary, vec!["across", "uniswap"]);
        assert!(response.execution_supported);
        assert_eq!(
            response
                .executable_tx
                .as_ref()
                .and_then(|tx| tx.gas_limit.as_str()),
            Some("0x12345")
        );
    }
}
