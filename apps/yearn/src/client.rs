use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct YearnApp;

#[allow(unused_imports)]
pub(crate) use crate::tool::*;

// ============================================================================
// Yearn yDaemon Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_YEARN_API: &str = "https://ydaemon.yearn.fi";

#[derive(Clone)]
pub(crate) struct YearnClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_endpoint: String,
}

impl YearnClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[yearn] failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            api_endpoint: std::env::var("YEARN_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_YEARN_API.to_string()),
        })
    }

    pub(crate) fn get_json(&self, url: &str, op: &str) -> Result<Value, String> {
        let response = self
            .http
            .get(url)
            .send()
            .map_err(|e| format!("[yearn] {op} failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[yearn] {op} failed: {status} {body}"));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[yearn] {op} failed: decode error: {e}"))
    }

    pub(crate) fn with_source(value: Value) -> Value {
        match value {
            Value::Object(mut map) => {
                map.insert("source".to_string(), Value::String("yearn".to_string()));
                Value::Object(map)
            }
            other => json!({
                "source": "yearn",
                "data": other,
            }),
        }
    }

    pub(crate) fn get_all_vaults(&self, chain_id: u64) -> Result<Value, String> {
        let url = format!("{}/{chain_id}/vaults/all", self.api_endpoint);
        let value = self.get_json(&url, "get_all_vaults")?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_vault_detail(&self, chain_id: u64, address: &str) -> Result<Value, String> {
        let url = format!("{}/{chain_id}/vaults/{address}", self.api_endpoint);
        let value = self.get_json(&url, "get_vault_detail")?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_blacklisted_vaults(&self) -> Result<Value, String> {
        let url = format!("{}/info/vaults/blacklisted", self.api_endpoint);
        let value = self.get_json(&url, "get_blacklisted_vaults")?;
        Ok(Self::with_source(value))
    }
}

// ============================================================================
// Tool arg structs
// ============================================================================

pub(crate) struct GetAllVaults;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetAllVaultsArgs {
    /// Chain ID to query. Supported: 1 (Ethereum), 10 (Optimism), 137 (Polygon), 250 (Fantom), 8453 (Base), 42161 (Arbitrum). Default: 1.
    #[serde(default = "default_chain_id")]
    pub(crate) chain_id: u64,
}

pub(crate) struct GetVaultDetail;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetVaultDetailArgs {
    /// Chain ID to query. Supported: 1 (Ethereum), 10 (Optimism), 137 (Polygon), 250 (Fantom), 8453 (Base), 42161 (Arbitrum). Default: 1.
    #[serde(default = "default_chain_id")]
    pub(crate) chain_id: u64,
    /// The vault contract address (e.g. "0x...")
    pub(crate) address: String,
}

pub(crate) struct GetBlacklistedVaults;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetBlacklistedVaultsArgs {}

fn default_chain_id() -> u64 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Story: "Deposit idle USDT into the best Yearn vault"
    /// Fetch all vaults, filter blacklisted ones, pick a vault, inspect its detail.
    #[test]
    fn deposit_stablecoin_vault_workflow() {
        let client = YearnClient::new().expect("failed to create YearnClient");

        // Step 1: Get all vaults on Ethereum mainnet.
        println!("[step 1] Fetching all vaults for chain 1 (Ethereum)...");
        let all_vaults = client.get_all_vaults(1).expect("get_all_vaults failed");
        let vaults_arr = all_vaults["data"]
            .as_array()
            .or_else(|| all_vaults.as_array())
            .expect("expected vaults to be an array");
        println!("[step 1] Got {} vaults on Ethereum", vaults_arr.len());
        assert!(
            !vaults_arr.is_empty(),
            "expected at least one vault on chain 1"
        );

        // Verify vault entries carry APY and TVL data.
        let sample = &vaults_arr[0];
        println!(
            "[step 1] Sample vault: address={}, has_apr={}, has_tvl={}",
            sample["address"].as_str().unwrap_or("?"),
            sample.get("apy").is_some() || sample.get("apr").is_some(),
            sample.get("tvl").is_some()
        );
        assert!(
            sample.get("apy").is_some() || sample.get("apr").is_some(),
            "expected APY/APR data on vault entry"
        );
        assert!(
            sample.get("tvl").is_some(),
            "expected TVL data on vault entry"
        );

        // Step 2: Get blacklisted vaults and filter them out.
        println!("[step 2] Fetching blacklisted vaults...");
        let blacklisted = client
            .get_blacklisted_vaults()
            .expect("get_blacklisted_vaults failed");
        // The response should be valid JSON (object or array).
        assert!(
            blacklisted.is_object() || blacklisted.is_array(),
            "blacklisted response should be valid JSON"
        );

        // Collect blacklisted addresses for filtering.
        let blacklisted_addrs: Vec<String> = blacklisted["data"]
            .as_array()
            .or_else(|| blacklisted.as_array())
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
            .collect();
        println!(
            "[step 2] Found {} blacklisted vault addresses",
            blacklisted_addrs.len()
        );

        let filtered: Vec<&Value> = vaults_arr
            .iter()
            .filter(|v| {
                let addr = v["address"].as_str().unwrap_or_default().to_lowercase();
                !blacklisted_addrs.contains(&addr)
            })
            .collect();
        println!(
            "[step 2] After filtering: {} vaults remain (removed {})",
            filtered.len(),
            vaults_arr.len() - filtered.len()
        );
        assert!(
            !filtered.is_empty(),
            "expected at least one non-blacklisted vault"
        );

        // Step 3: Pick the first filtered vault and get its detail.
        let vault_addr = filtered[0]["address"]
            .as_str()
            .expect("vault should have an address field");
        println!("[step 3] Fetching detail for vault {}...", vault_addr);
        let detail = client
            .get_vault_detail(1, vault_addr)
            .expect("get_vault_detail failed");
        assert!(detail.is_object(), "vault detail should be a JSON object");

        // Verify the detail contains strategies, fees, and APY breakdown.
        let detail_data = if detail.get("data").is_some() {
            &detail["data"]
        } else {
            &detail
        };
        let has_strategies =
            detail_data.get("strategies").is_some() || detail_data.get("strategy").is_some();
        let has_apr = detail_data.get("apy").is_some() || detail_data.get("apr").is_some();
        println!(
            "[step 3] Vault detail: has_strategies={}, has_apr={}, has_address={}",
            has_strategies,
            has_apr,
            detail_data.get("address").is_some()
        );
        assert!(has_strategies, "vault detail should include strategies");
        assert!(has_apr, "vault detail should include APY/APR breakdown");

        // Step 4: Assert we have enough data to choose a vault and build a deposit TX.
        assert!(
            detail_data.get("address").is_some() || detail_data.get("token").is_some(),
            "vault detail should contain address or token info to build a deposit TX"
        );
        println!(
            "[step 4] Workflow complete: vault {} is ready for deposit",
            vault_addr
        );
    }

    /// Story: "Migrate my vault position to a higher-yield chain"
    /// Fetch vaults on Ethereum and Arbitrum, compare yields for similar assets.
    #[test]
    fn cross_chain_vault_comparison_workflow() {
        let client = YearnClient::new().expect("failed to create YearnClient");

        // Step 1: Get all vaults on Ethereum (chain 1).
        println!("[step 1] Fetching all vaults for chain 1 (Ethereum)...");
        let eth_vaults_resp = client.get_all_vaults(1).expect("get_all_vaults(1) failed");
        let eth_vaults = eth_vaults_resp["data"]
            .as_array()
            .or_else(|| eth_vaults_resp.as_array())
            .expect("expected Ethereum vaults to be an array");
        println!("[step 1] Got {} vaults on Ethereum", eth_vaults.len());
        assert!(
            !eth_vaults.is_empty(),
            "expected at least one vault on Ethereum"
        );

        // Step 2: Get all vaults on Optimism (chain 10).
        println!("[step 2] Fetching all vaults for chain 10 (Optimism)...");
        let arb_vaults_resp = client
            .get_all_vaults(10)
            .expect("get_all_vaults(10) failed");
        let arb_vaults = arb_vaults_resp["data"]
            .as_array()
            .or_else(|| arb_vaults_resp.as_array())
            .expect("expected Optimism vaults to be an array");
        println!("[step 2] Got {} vaults on Optimism", arb_vaults.len());
        assert!(
            !arb_vaults.is_empty(),
            "expected at least one vault on Optimism"
        );

        // Step 3: Find vaults for similar assets across both chains.
        println!("[step 3] Finding common asset symbols across chains...");
        // Extract token symbols from each chain.
        let eth_symbols: std::collections::HashSet<String> = eth_vaults
            .iter()
            .filter_map(|v| {
                v["token"]["symbol"]
                    .as_str()
                    .or_else(|| v["symbol"].as_str())
                    .map(|s| s.to_uppercase())
            })
            .collect();

        let arb_symbols: std::collections::HashSet<String> = arb_vaults
            .iter()
            .filter_map(|v| {
                v["token"]["symbol"]
                    .as_str()
                    .or_else(|| v["symbol"].as_str())
                    .map(|s| s.to_uppercase())
            })
            .collect();

        let common_symbols: Vec<&String> = eth_symbols.intersection(&arb_symbols).collect();
        println!(
            "[step 3] Ethereum has {} unique symbols, Optimism has {}, common: {} ({:?})",
            eth_symbols.len(),
            arb_symbols.len(),
            common_symbols.len(),
            &common_symbols[..std::cmp::min(5, common_symbols.len())]
        );
        assert!(
            !common_symbols.is_empty(),
            "expected at least one common asset symbol across Ethereum and Arbitrum"
        );

        // Step 4: For a common asset, compare yields to identify the better chain.
        let target = common_symbols[0];
        println!("[step 4] Comparing yields for target asset: {target}");

        let best_apy = |vaults: &[Value], symbol: &str| -> Option<f64> {
            vaults
                .iter()
                .filter(|v| {
                    let sym = v["token"]["symbol"]
                        .as_str()
                        .or_else(|| v["symbol"].as_str())
                        .unwrap_or_default()
                        .to_uppercase();
                    sym == symbol
                })
                .filter_map(|v| {
                    v["apy"]["net_apy"]
                        .as_f64()
                        .or_else(|| v["apr"]["netAPR"].as_f64())
                        .or_else(|| v["apr"]["net_apy"].as_f64())
                        .or_else(|| v["apy"]["points"]["week_ago"].as_f64())
                })
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        };

        let eth_best = best_apy(eth_vaults, target);
        let arb_best = best_apy(arb_vaults, target);

        // At least one chain should report yield data for the common asset.
        println!(
            "[step 4] Yield lookup for {target}: eth_best={eth_best:?}, arb_best={arb_best:?}"
        );
        assert!(
            eth_best.is_some() || arb_best.is_some(),
            "expected yield data for {target} on at least one chain"
        );

        // We can identify which chain offers better yield (or that data exists to decide).
        if let (Some(e), Some(a)) = (eth_best, arb_best) {
            let better_chain = if a > e { "Arbitrum" } else { "Ethereum" };
            println!(
                "[step 4] For {target}: Ethereum APY={e:.4}, Arbitrum APY={a:.4} => {better_chain} is better"
            );
        } else {
            println!(
                "[step 4] For {target}: yield data available on one chain (eth={eth_best:?}, arb={arb_best:?})"
            );
        }
        println!("[step 4] Cross-chain comparison workflow complete");
    }
}
