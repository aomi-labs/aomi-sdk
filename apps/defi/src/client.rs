use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct DefiApp;

pub(crate) use crate::tool::*;

pub(crate) fn extract_first_price(response: &Value) -> Option<f64> {
    response
        .get("coins")
        .and_then(Value::as_object)
        .and_then(|coins| coins.values().next())
        .and_then(|coin| coin.get("price"))
        .and_then(Value::as_f64)
}

// ============================================================================
// DefiLama Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_DEFILLAMA_API: &str = "https://api.llama.fi";
pub(crate) const DEFAULT_DEFILLAMA_COINS_API: &str = "https://coins.llama.fi";
pub(crate) const DEFAULT_DEFILLAMA_YIELDS_API: &str = "https://yields.llama.fi";
pub(crate) const DEFAULT_DEFILLAMA_BRIDGES_API: &str = "https://bridges.llama.fi";

#[derive(Clone)]
pub(crate) struct DefiLamaClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_endpoint: String,
    pub(crate) coins_endpoint: String,
    pub(crate) yields_endpoint: String,
    pub(crate) bridges_endpoint: String,
}

impl DefiLamaClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            api_endpoint: std::env::var("DEFILLAMA_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_DEFILLAMA_API.to_string()),
            coins_endpoint: std::env::var("DEFILLAMA_COINS_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_DEFILLAMA_COINS_API.to_string()),
            yields_endpoint: std::env::var("DEFILLAMA_YIELDS_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_DEFILLAMA_YIELDS_API.to_string()),
            bridges_endpoint: std::env::var("DEFILLAMA_BRIDGES_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_DEFILLAMA_BRIDGES_API.to_string()),
        })
    }

    pub(crate) fn get_json(&self, url: &str, op: &str) -> Result<Value, String> {
        let response = self
            .http
            .get(url)
            .send()
            .map_err(|e| format!("[defillama] {op} request failed ({url}): {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "[defillama] {op} request failed ({url}): {status} {body}"
            ));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[defillama] {op} decode failed ({url}): {e}; body: {body}"))
    }

    pub(crate) fn with_source(value: Value) -> Value {
        match value {
            Value::Object(mut map) => {
                map.insert("source".to_string(), Value::String("defillama".to_string()));
                Value::Object(map)
            }
            other => json!({
                "source": "defillama",
                "data": other,
            }),
        }
    }

    pub(crate) fn get_token_price(&self, token: &str) -> Result<Value, String> {
        let coin_id = normalize_token_id(token);
        let url = format!("{}/prices/current/{}", self.coins_endpoint, coin_id);
        let value = self.get_json(&url, "token price")?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_yield_pools(
        &self,
        chain: Option<&str>,
        project: Option<&str>,
    ) -> Result<Value, String> {
        let url = format!("{}/pools", self.yields_endpoint);
        let mut value = self.get_json(&url, "yield pools")?;

        if let Some(data) = value.get_mut("data").and_then(Value::as_array_mut) {
            data.retain(|pool| {
                let chain_ok = chain
                    .map(|c| {
                        pool.get("chain")
                            .and_then(Value::as_str)
                            .map(|s| s.eq_ignore_ascii_case(c))
                            .unwrap_or(false)
                    })
                    .unwrap_or(true);
                let project_ok = project
                    .map(|p| {
                        let p_lower = p.to_lowercase();
                        pool.get("project")
                            .and_then(Value::as_str)
                            .map(|s| s.to_lowercase().contains(&p_lower))
                            .unwrap_or(false)
                    })
                    .unwrap_or(true);
                let apy_ok = pool.get("apy").and_then(Value::as_f64).unwrap_or(0.0) > 0.0;
                chain_ok && project_ok && apy_ok
            });
        }

        Ok(Self::with_source(value))
    }

    pub(crate) fn get_protocols(&self, category: Option<&str>) -> Result<Value, String> {
        let url = format!("{}/protocols", self.api_endpoint);
        let mut value = self.get_json(&url, "protocols")?;

        if let Some(arr) = value.as_array_mut() {
            if let Some(category_filter) = category {
                let category_filter = category_filter.to_lowercase();
                arr.retain(|protocol| {
                    protocol
                        .get("category")
                        .and_then(Value::as_str)
                        .map(|s| s.to_lowercase().contains(&category_filter))
                        .unwrap_or(false)
                });
            }
            arr.sort_by(|a, b| {
                let at = a.get("tvl").and_then(Value::as_f64).unwrap_or(0.0);
                let bt = b.get("tvl").and_then(Value::as_f64).unwrap_or(0.0);
                bt.partial_cmp(&at).unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        Ok(Self::with_source(value))
    }

    pub(crate) fn get_chains_tvl(&self) -> Result<Value, String> {
        let url = format!("{}/v2/chains", self.api_endpoint);
        let mut value = self.get_json(&url, "chains tvl")?;

        if let Some(arr) = value.as_array_mut() {
            arr.sort_by(|a, b| {
                let at = a.get("tvl").and_then(Value::as_f64).unwrap_or(0.0);
                let bt = b.get("tvl").and_then(Value::as_f64).unwrap_or(0.0);
                bt.partial_cmp(&at).unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        Ok(Self::with_source(value))
    }

    pub(crate) fn get_bridges(&self) -> Result<Value, String> {
        let url = format!("{}/bridges", self.bridges_endpoint);
        let mut value = self.get_json(&url, "bridges")?;

        if let Some(arr) = value.get_mut("bridges").and_then(Value::as_array_mut) {
            arr.sort_by(|a, b| {
                let at = a
                    .get("lastDailyVolume")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let bt = b
                    .get("lastDailyVolume")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                bt.partial_cmp(&at).unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        Ok(Self::with_source(value))
    }
}

pub(crate) fn normalize_token_id(token: &str) -> String {
    let token_lower = token.to_lowercase();
    match token_lower.as_str() {
        "eth" | "ethereum" => "coingecko:ethereum".to_string(),
        "btc" | "bitcoin" => "coingecko:bitcoin".to_string(),
        "usdc" => "coingecko:usd-coin".to_string(),
        "usdt" | "tether" => "coingecko:tether".to_string(),
        "dai" => "coingecko:dai".to_string(),
        "sol" | "solana" => "coingecko:solana".to_string(),
        "bnb" => "coingecko:binancecoin".to_string(),
        "avax" | "avalanche" => "coingecko:avalanche-2".to_string(),
        "matic" | "polygon" => "coingecko:matic-network".to_string(),
        "arb" | "arbitrum" => "coingecko:arbitrum".to_string(),
        "op" | "optimism" => "coingecko:optimism".to_string(),
        "uni" | "uniswap" => "coingecko:uniswap".to_string(),
        "aave" => "coingecko:aave".to_string(),
        "link" | "chainlink" => "coingecko:chainlink".to_string(),
        "mkr" | "maker" => "coingecko:maker".to_string(),
        "crv" | "curve" => "coingecko:curve-dao-token".to_string(),
        "ldo" | "lido" => "coingecko:lido-dao".to_string(),
        _ => format!("coingecko:{}", token_lower),
    }
}

// ============================================================================
// Aggregator Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_ZEROX_ENDPOINT: &str = "https://api.0x.org";
pub(crate) const DEFAULT_LIFI_ENDPOINT: &str = "https://li.quest";
pub(crate) const DEFAULT_COW_ENDPOINT: &str = "https://api.cow.fi/mainnet";

#[derive(Clone)]
pub(crate) struct Aggregator {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) zerox_endpoint: String,
    pub(crate) lifi_endpoint: String,
    pub(crate) cow_endpoint: String,
    pub(crate) zerox_api_key: Option<String>,
    pub(crate) lifi_api_key: Option<String>,
    pub(crate) cow_api_key: Option<String>,
}

impl Aggregator {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            zerox_endpoint: std::env::var("ZEROX_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_ZEROX_ENDPOINT.to_string()),
            lifi_endpoint: std::env::var("LIFI_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_LIFI_ENDPOINT.to_string()),
            cow_endpoint: std::env::var("COW_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_COW_ENDPOINT.to_string()),
            zerox_api_key: std::env::var("ZEROX_API_KEY").ok(),
            lifi_api_key: std::env::var("LIFI_API_KEY").ok(),
            cow_api_key: std::env::var("COW_API_KEY").ok(),
        })
    }

    pub(crate) fn with_source(value: Value, source: &str) -> Value {
        match value {
            Value::Object(mut map) => {
                map.insert("source".to_string(), Value::String(source.to_string()));
                Value::Object(map)
            }
            other => json!({
                "source": source,
                "data": other,
            }),
        }
    }

    pub(crate) fn send_json(
        request: reqwest::blocking::RequestBuilder,
        source: &str,
        operation: &str,
    ) -> Result<Value, String> {
        let response = request
            .send()
            .map_err(|e| format!("[{source}] {operation} request failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "[{source}] {operation} request failed: {status} {body}"
            ));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[{source}] {operation} decode failed: {e}; body: {body}"))
    }

    pub(crate) fn amount_to_base_units(amount: f64, decimals: u8) -> Result<String, String> {
        if !amount.is_finite() || amount < 0.0 {
            return Err("amount must be a finite non-negative number".to_string());
        }
        let scaled = amount * 10f64.powi(decimals as i32);
        if scaled > (u128::MAX as f64) {
            return Err("amount is too large to convert to base units".to_string());
        }
        Ok((scaled.round() as u128).to_string())
    }

    pub(crate) fn quote_amount_base_units(
        &self,
        chain: &str,
        token: &str,
        amount: f64,
    ) -> Result<String, String> {
        let (chain_name, _) = Self::get_chain_info(chain)?;
        let decimals = Self::get_token_decimals(chain_name, token);
        Self::amount_to_base_units(amount, decimals)
    }

    pub(crate) fn resolve_token_address(&self, chain: &str, token: &str) -> Result<String, String> {
        let (chain_name, _) = Self::get_chain_info(chain)?;
        Self::get_token_address(chain_name, token)
    }

    pub(crate) fn build_lifi_main_tx(quote: &Value) -> Value {
        let main_tx = quote
            .get("transactionRequest")
            .cloned()
            .unwrap_or(Value::Null);

        json!({
            "to": main_tx.get("to").cloned().unwrap_or(Value::Null),
            "data": main_tx.get("data").cloned().unwrap_or(Value::String("0x".to_string())),
            "value": main_tx.get("value").cloned().unwrap_or(Value::String("0".to_string())),
            "gas_limit": main_tx
                .get("gasLimit")
                .cloned()
                .or_else(|| main_tx.get("gas").cloned())
                .unwrap_or(Value::Null),
            "description": "LI.FI main transaction",
        })
    }

    pub(crate) fn build_lifi_approval_tx(
        quote: &Value,
        from_amount: &str,
    ) -> Result<Value, String> {
        let approval_address = quote
            .get("estimate")
            .and_then(|e| e.get("approvalAddress"))
            .and_then(Value::as_str);
        let from_token_address = quote
            .get("action")
            .and_then(|a| a.get("fromToken"))
            .and_then(|t| t.get("address"))
            .and_then(Value::as_str);

        if let (Some(spender), Some(token_address)) = (approval_address, from_token_address) {
            let is_native = token_address
                .eq_ignore_ascii_case("0x0000000000000000000000000000000000000000")
                || token_address.eq_ignore_ascii_case("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
            if !is_native && Self::is_hex_address(token_address) && Self::is_hex_address(spender) {
                let approve_calldata = Self::encode_approve_calldata(spender, from_amount)?;
                return Ok(json!({
                    "to": token_address,
                    "data": approve_calldata,
                    "value": "0",
                    "gas_limit": Value::Null,
                    "description": "LI.FI token approval",
                }));
            }
        }

        Ok(Value::Null)
    }

    pub(crate) fn normalize_lifi_chain_id(chain: &str) -> Result<String, String> {
        let normalized = chain.to_lowercase();
        let chain_id = match normalized.as_str() {
            "ethereum" | "eth" | "mainnet" => "1",
            "polygon" | "matic" => "137",
            "arbitrum" | "arb" | "arbitrum_one" => "42161",
            "optimism" | "op" => "10",
            "base" => "8453",
            "bsc" | "bnb" | "binance" => "56",
            "avalanche" | "avax" => "43114",
            "gnosis" | "xdai" => "100",
            "fantom" | "ftm" => "250",
            "linea" => "59144",
            "scroll" => "534352",
            "zksync" | "zksync_era" => "324",
            _ => {
                if chain.chars().all(|c| c.is_ascii_digit()) {
                    return Ok(chain.to_string());
                }
                return Err(format!(
                    "[lifi] unsupported chain '{chain}'. Use a known chain name or numeric chain id"
                ));
            }
        };

        Ok(chain_id.to_string())
    }

    pub(crate) fn get_quote_0x(
        &self,
        chain: &str,
        from_token: &str,
        to_token: &str,
        amount: f64,
        sender_address: Option<&str>,
        slippage: Option<f64>,
    ) -> Result<Value, String> {
        let api_key = self
            .zerox_api_key
            .as_ref()
            .ok_or_else(|| "[0x] missing ZEROX_API_KEY".to_string())?;
        let (chain_name, chain_id) = Self::get_chain_info(chain)?;
        let from_addr = Self::get_token_address(chain_name, from_token)?;
        let to_addr = Self::get_token_address(chain_name, to_token)?;
        let decimals = Self::get_token_decimals(chain_name, from_token);
        let amount_wei = Self::amount_to_base_units(amount, decimals)?;

        let mut request = self
            .http
            .get(format!("{}/swap/permit2/price", self.zerox_endpoint))
            .query(&[
                ("chainId", chain_id.to_string()),
                ("sellToken", from_addr),
                ("buyToken", to_addr),
                ("sellAmount", amount_wei),
                ("slippagePercentage", slippage.unwrap_or(0.01).to_string()),
            ])
            .header("0x-api-key", api_key)
            .header("0x-version", "v2");
        if let Some(sender_address) = sender_address {
            request = request.query(&[("taker", sender_address)]);
        }

        let value = Self::send_json(request, "0x", "quote")?;
        Ok(Self::with_source(value, "0x"))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn get_quote_lifi(
        &self,
        from_chain: &str,
        to_chain: &str,
        from_token: &str,
        to_token: &str,
        from_amount: &str,
        from_address: &str,
        to_address: Option<&str>,
    ) -> Result<Value, String> {
        let from_chain_id = Self::normalize_lifi_chain_id(from_chain)?;
        let to_chain_id = Self::normalize_lifi_chain_id(to_chain)?;

        let mut request = self
            .http
            .get(format!("{}/v1/quote", self.lifi_endpoint))
            .query(&[
                ("fromChain", from_chain_id.as_str()),
                ("toChain", to_chain_id.as_str()),
                ("fromToken", from_token),
                ("toToken", to_token),
                ("fromAmount", from_amount),
                ("fromAddress", from_address),
            ]);

        if let Some(to_address) = to_address {
            request = request.query(&[("toAddress", to_address)]);
        }
        if let Some(api_key) = self.lifi_api_key.as_ref() {
            request = request.header("x-lifi-api-key", api_key);
        }

        let value = Self::send_json(request, "lifi", "quote")?;
        Ok(Self::with_source(value, "lifi"))
    }

    pub(crate) fn get_quote_cow(&self, chain: &str, payload: Value) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let mut request = self.http.post(format!("{base}/quote")).json(&payload);
        if let Some(api_key) = self.cow_api_key.as_ref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }

        let value = Self::send_json(request, "cow", "quote")?;
        Ok(Self::with_source(value, "cow"))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn get_bridge_quote(
        &self,
        from_chain: &str,
        to_chain: &str,
        from_token: &str,
        to_token: &str,
        amount: f64,
        from_address: Option<&str>,
        to_address: Option<&str>,
        slippage_bps: Option<u32>,
    ) -> Result<Value, String> {
        let (from_chain_name, _) = Self::get_chain_info(from_chain)?;
        let (to_chain_name, _) = Self::get_chain_info(to_chain)?;
        let from_token_addr = Self::get_token_address(from_chain_name, from_token)?;
        let to_token_addr = Self::get_token_address(to_chain_name, to_token)?;
        let from_decimals = Self::get_token_decimals(from_chain_name, from_token);
        let to_decimals = Self::get_token_decimals(to_chain_name, to_token);
        let from_amount_wei = Self::amount_to_base_units(amount, from_decimals)?;
        let slippage_bps = slippage_bps.unwrap_or(50);
        let slippage = (slippage_bps as f64) / 10_000.0;

        let from_addr = from_address.unwrap_or("");
        let to_addr = to_address.unwrap_or("");
        let has_wallet_addresses = Self::is_hex_address(from_addr) && Self::is_hex_address(to_addr);
        if !has_wallet_addresses {
            return Ok(json!({
                "from": format!("{amount} {} on {}", from_token.to_uppercase(), from_chain.to_lowercase()),
                "to": format!("{} on {}", to_token.to_uppercase(), to_chain.to_lowercase()),
                "to_amount_estimate": Value::Null,
                "min_received": Value::Null,
                "bridge": "planning-only",
                "estimated_duration_seconds": Value::Null,
                "estimated_fee_usd": Value::Null,
                "route_summary": ["Source and destination wallet addresses are required"],
                "executable_tx": Value::Null,
                "execution_supported": false,
                "warning": "Provide source and destination wallet addresses to request an executable bridge route.",
            }));
        }

        let mut request = self
            .http
            .get(format!("{}/v1/quote", self.lifi_endpoint))
            .query(&[
                ("fromChain", Self::normalize_lifi_chain_id(from_chain)?),
                ("toChain", Self::normalize_lifi_chain_id(to_chain)?),
                ("fromToken", from_token_addr),
                ("toToken", to_token_addr),
                ("fromAmount", from_amount_wei),
                ("fromAddress", from_addr.to_string()),
                ("toAddress", to_addr.to_string()),
                ("slippage", format!("{slippage:.4}")),
            ]);
        if let Some(api_key) = self.lifi_api_key.as_ref() {
            request = request.header("x-lifi-api-key", api_key);
        }

        if let Ok(quote) = Self::send_json(request, "lifi", "bridge quote") {
            if let Some(estimate) = quote.get("estimate") {
                let to_amount = estimate
                    .get("toAmount")
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|raw| raw / 10f64.powi(to_decimals as i32));
                let min_received = estimate
                    .get("toAmountMin")
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|raw| raw / 10f64.powi(to_decimals as i32));
                let fee_costs = estimate
                    .get("feeCosts")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .chain(
                        estimate
                            .get("gasCosts")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten(),
                    )
                    .filter_map(|cost| {
                        cost.get("amountUSD")
                            .and_then(Value::as_str)
                            .and_then(|s| s.parse::<f64>().ok())
                    })
                    .sum::<f64>();
                let bridge_name = quote
                    .get("toolDetails")
                    .and_then(|details| details.get("name"))
                    .and_then(Value::as_str)
                    .or_else(|| quote.get("tool").and_then(Value::as_str))
                    .unwrap_or("unknown");
                let route_summary: Vec<Value> = estimate
                    .get("steps")
                    .and_then(Value::as_array)
                    .map(|steps| {
                        steps
                            .iter()
                            .filter_map(|step| step.get("tool").and_then(Value::as_str))
                            .map(|tool| Value::String(tool.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let executable_tx = quote.get("transactionRequest").map(|tx| {
                    json!({
                        "to": tx.get("to").cloned().unwrap_or(Value::Null),
                        "data": tx.get("data").cloned().unwrap_or(Value::Null),
                        "value": tx.get("value").cloned().unwrap_or(Value::Null),
                        "gas_limit": tx.get("gasLimit").cloned().unwrap_or(Value::Null),
                    })
                });

                return Ok(json!({
                    "from": format!("{amount} {} on {}", from_token.to_uppercase(), from_chain.to_lowercase()),
                    "to": format!("{} on {}", to_token.to_uppercase(), to_chain.to_lowercase()),
                    "to_amount_estimate": to_amount.map(|v| format!("{v:.6}")),
                    "min_received": min_received.map(|v| format!("{v:.6}")),
                    "bridge": bridge_name,
                    "estimated_duration_seconds": estimate.get("executionDuration").cloned().unwrap_or(Value::Null),
                    "estimated_fee_usd": if fee_costs > 0.0 { Some(format!("{fee_costs:.2}")) } else { None },
                    "route_summary": route_summary,
                    "executable_tx": executable_tx,
                    "execution_supported": executable_tx.is_some(),
                    "warning": Value::Null,
                }));
            }
        }

        let price_client = DefiLamaClient::new()?;
        let from_price =
            extract_first_price(&price_client.get_token_price(from_token)?).unwrap_or(1.0);
        let to_price = extract_first_price(&price_client.get_token_price(to_token)?).unwrap_or(1.0);
        let estimated_to_amount = (amount * from_price) / to_price * (1.0 - slippage.max(0.001));
        let min_received = estimated_to_amount * (1.0 - slippage);

        Ok(json!({
            "from": format!("{amount} {} on {}", from_token.to_uppercase(), from_chain.to_lowercase()),
            "to": format!("{} on {}", to_token.to_uppercase(), to_chain.to_lowercase()),
            "to_amount_estimate": format!("{estimated_to_amount:.6}"),
            "min_received": format!("{min_received:.6}"),
            "bridge": "planning-only",
            "estimated_duration_seconds": Value::Null,
            "estimated_fee_usd": Value::Null,
            "route_summary": ["No executable bridge payload available"],
            "executable_tx": Value::Null,
            "execution_supported": false,
            "warning": "Bridge quote is planning-only. Provide source and destination wallet addresses for executable routing.",
        }))
    }

    pub(crate) fn place_order_0x(
        &self,
        chain: &str,
        sell_token: &str,
        buy_token: &str,
        amount: f64,
        sender_address: &str,
        slippage: Option<f64>,
    ) -> Result<Value, String> {
        let api_key = self
            .zerox_api_key
            .as_ref()
            .ok_or_else(|| "[0x] missing ZEROX_API_KEY".to_string())?;
        let (chain_name, chain_id) = Self::get_chain_info(chain)?;
        let sell_addr = Self::get_token_address(chain_name, sell_token)?;
        let buy_addr = Self::get_token_address(chain_name, buy_token)?;
        let decimals = Self::get_token_decimals(chain_name, sell_token);
        let amount_wei = Self::amount_to_base_units(amount, decimals)?;

        let response = self
            .http
            .get(format!(
                "{}/swap/allowance-holder/quote",
                self.zerox_endpoint
            ))
            .query(&[
                ("chainId", chain_id.to_string()),
                ("sellToken", sell_addr),
                ("buyToken", buy_addr),
                ("sellAmount", amount_wei),
                ("taker", sender_address.to_string()),
                ("slippagePercentage", slippage.unwrap_or(0.01).to_string()),
            ])
            .header("0x-api-key", api_key)
            .header("0x-version", "v2");

        let value = Self::send_json(response, "0x", "place order")?;
        Ok(Self::with_source(value, "0x"))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn place_order_lifi(
        &self,
        from_chain: &str,
        to_chain: &str,
        sell_token: &str,
        buy_token: &str,
        from_amount: &str,
        from_address: &str,
        receiver_address: Option<&str>,
        slippage: Option<f64>,
    ) -> Result<Value, String> {
        let from_chain_id = Self::normalize_lifi_chain_id(from_chain)?;
        let to_chain_id = Self::normalize_lifi_chain_id(to_chain)?;

        let mut request = self
            .http
            .get(format!("{}/v1/quote", self.lifi_endpoint))
            .query(&[
                ("fromChain", from_chain_id.as_str()),
                ("toChain", to_chain_id.as_str()),
                ("fromToken", sell_token),
                ("toToken", buy_token),
                ("fromAmount", from_amount),
                ("fromAddress", from_address),
            ]);
        if let Some(receiver_address) = receiver_address {
            request = request.query(&[("toAddress", receiver_address)]);
        }
        if let Some(slippage) = slippage {
            request = request.query(&[("slippage", slippage.to_string())]);
        }
        if let Some(api_key) = self.lifi_api_key.as_ref() {
            request = request.header("x-lifi-api-key", api_key);
        }

        let quote = Self::send_json(request, "lifi", "place order")?;
        let mut out = json!({
            "source": "lifi",
            "raw_quote": quote,
            "main_tx": Value::Null,
            "approval_tx": Value::Null,
        });

        out["main_tx"] = Self::build_lifi_main_tx(&out["raw_quote"]);
        out["approval_tx"] = Self::build_lifi_approval_tx(&out["raw_quote"], from_amount)?;

        Ok(out)
    }

    pub(crate) fn place_order_cow(&self, chain: &str, payload: Value) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let mut request = self.http.post(format!("{}/orders", base)).json(&payload);
        if let Some(api_key) = self.cow_api_key.as_ref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }

        let value = Self::send_json(request, "cow", "post order")?;
        Ok(Self::with_source(value, "cow"))
    }

    pub(crate) fn get_chain_info(chain: &str) -> Result<(&'static str, u64), String> {
        match chain.to_lowercase().as_str() {
            "ethereum" | "eth" | "mainnet" => Ok(("ethereum", 1)),
            "polygon" | "matic" => Ok(("polygon", 137)),
            "arbitrum" | "arb" => Ok(("arbitrum", 42161)),
            "optimism" | "op" => Ok(("optimism", 10)),
            "base" => Ok(("base", 8453)),
            "bsc" | "binance" => Ok(("bsc", 56)),
            "avalanche" | "avax" => Ok(("avalanche", 43114)),
            _ => Err(format!("[0x] unsupported chain: {chain}")),
        }
    }

    pub(crate) fn is_hex_address(token: &str) -> bool {
        token.len() == 42
            && token.starts_with("0x")
            && token[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    pub(crate) fn get_token_address(chain: &str, token: &str) -> Result<String, String> {
        let native = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";
        let token_lower = token.to_lowercase();

        if token_lower == native.to_lowercase() {
            return Ok(native.to_string());
        }
        if Self::is_hex_address(token) {
            return Ok(token.to_string());
        }

        match (chain, token_lower.as_str()) {
            (_, "eth") | (_, "matic") | (_, "bnb") | (_, "avax") => Ok(native.to_string()),
            ("ethereum", "usdc") => Ok("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string()),
            ("ethereum", "usdt") => Ok("0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string()),
            ("ethereum", "dai") => Ok("0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string()),
            ("ethereum", "weth") => Ok("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()),
            ("ethereum", "wbtc") => Ok("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string()),
            ("ethereum", "uni") => Ok("0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984".to_string()),
            ("ethereum", "aave") => Ok("0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DdAE9".to_string()),
            ("ethereum", "link") => Ok("0x514910771AF9Ca656af840dff83E8264EcF986CA".to_string()),
            ("ethereum", "mkr") => Ok("0x9f8F72aA9304c8B593d555F12ef6589cC3A579A2".to_string()),
            ("ethereum", "crv") => Ok("0xD533a949740bb3306d119CC777fa900bA034cd52".to_string()),
            ("ethereum", "ldo") => Ok("0x5A98FcBEA516Cf06857215779Fd812CA3beF1B32".to_string()),
            ("arbitrum", "usdc") => Ok("0xaf88d065e77c8cC2239327C5EDb3A432268e5831".to_string()),
            ("arbitrum", "usdt") => Ok("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9".to_string()),
            ("arbitrum", "weth") => Ok("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".to_string()),
            ("base", "usdc") => Ok("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string()),
            ("base", "weth") => Ok("0x4200000000000000000000000000000000000006".to_string()),
            ("polygon", "usdc") => Ok("0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359".to_string()),
            ("polygon", "usdt") => Ok("0xc2132D05D31c914a87C6611C10748AEb04B58e8F".to_string()),
            ("polygon", "weth") => Ok("0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619".to_string()),
            _ => Err(format!("[0x] unknown token {token} on chain {chain}")),
        }
    }

    pub(crate) fn get_token_decimals(chain: &str, token: &str) -> u8 {
        let token_lower = token.to_lowercase();

        if Self::is_hex_address(token) {
            return match (chain, token_lower.as_str()) {
                ("ethereum", "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48") => 6,
                ("ethereum", "0xdac17f958d2ee523a2206206994597c13d831ec7") => 6,
                ("arbitrum", "0xaf88d065e77c8cc2239327c5edb3a432268e5831") => 6,
                ("arbitrum", "0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9") => 6,
                ("polygon", "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359") => 6,
                ("polygon", "0xc2132d05d31c914a87c6611c10748aeb04b58e8f") => 6,
                ("base", "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913") => 6,
                ("ethereum", "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599") => 8,
                _ => 18,
            };
        }

        match token_lower.as_str() {
            "usdc" | "usdt" => 6,
            "wbtc" => 8,
            _ => 18,
        }
    }

    pub(crate) fn cow_api_base_for_chain(&self, chain: &str) -> Result<String, String> {
        let path = match chain.to_lowercase().as_str() {
            "ethereum" | "eth" | "mainnet" => "mainnet",
            "gnosis" | "xdai" => "xdai",
            "arbitrum" | "arb" | "arbitrum_one" => "arbitrum_one",
            "base" => "base",
            "polygon" | "matic" => "polygon",
            "avalanche" | "avax" => "avalanche",
            "bnb" | "bsc" => "bsc",
            "sepolia" => "sepolia",
            other => return Err(format!("[cow] unsupported chain for orderbook: {other}")),
        };

        let endpoint = self.cow_endpoint.trim_end_matches('/');
        if let Some((prefix, _)) = endpoint.rsplit_once('/') {
            return Ok(format!("{prefix}/{path}/api/v1"));
        }
        Ok(format!("{endpoint}/{path}/api/v1"))
    }

    pub(crate) fn encode_approve_calldata(
        spender: &str,
        amount_decimal: &str,
    ) -> Result<String, String> {
        let selector = "095ea7b3"; // approve(address,uint256)
        let spender_clean = spender.trim_start_matches("0x").to_lowercase();
        if spender_clean.len() != 40 || !spender_clean.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("[lifi] invalid approval spender address".to_string());
        }
        let amount = amount_decimal
            .parse::<u128>()
            .map_err(|e| format!("[lifi] invalid approval amount {amount_decimal}: {e}"))?;
        let amount_hex = format!("{amount:x}");

        let spender_slot = format!("{:0>64}", spender_clean);
        let amount_slot = format!("{:0>64}", amount_hex);
        Ok(format!("0x{selector}{spender_slot}{amount_slot}"))
    }
}

// ============================================================================
// Tool 1: Get Token Price
// ============================================================================

pub(crate) struct GetLammaTokenPrice;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLammaTokenPriceArgs {
    /// Token symbol or name (e.g., "ETH", "bitcoin", "USDC")
    pub(crate) token: String,
}
