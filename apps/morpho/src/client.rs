use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

#[allow(unused_imports)]
pub(crate) use crate::tool::*;

#[derive(Clone, Default)]
pub(crate) struct MorphoApp;

// ============================================================================
// Morpho GraphQL Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_MORPHO_API: &str = "https://blue-api.morpho.org/graphql";

#[derive(Clone)]
pub(crate) struct MorphoClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_endpoint: String,
}

impl MorphoClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[morpho] failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            api_endpoint: std::env::var("MORPHO_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_MORPHO_API.to_string()),
        })
    }

    pub(crate) fn post_graphql(
        &self,
        query: &str,
        variables: Option<Value>,
        op: &str,
    ) -> Result<Value, String> {
        let body = match variables {
            Some(vars) => json!({ "query": query, "variables": vars }),
            None => json!({ "query": query }),
        };

        let response = self
            .http
            .post(&self.api_endpoint)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| format!("[morpho] {op} failed: {e}"))?;

        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[morpho] {op} failed: {status} {text}"));
        }

        let parsed: Value = serde_json::from_str(&text)
            .map_err(|e| format!("[morpho] {op} failed: decode error: {e}; body: {text}"))?;

        if let Some(errors) = parsed.get("errors") {
            return Err(format!("[morpho] {op} failed: GraphQL errors: {errors}"));
        }

        Ok(parsed.get("data").cloned().unwrap_or(parsed))
    }

    pub(crate) fn get_markets(&self) -> Result<Value, String> {
        let query = r#"
            query {
                markets(first: 100) {
                    items {
                        uniqueKey
                        lltv
                        collateralAsset {
                            symbol
                            address
                            decimals
                        }
                        loanAsset {
                            symbol
                            address
                            decimals
                        }
                        state {
                            supplyApy
                            borrowApy
                            supplyAssetsUsd
                            borrowAssetsUsd
                            liquidityAssetsUsd
                            utilization
                        }
                    }
                }
            }
        "#;
        let data = self.post_graphql(query, None, "get_markets")?;
        Ok(json!({
            "source": "morpho",
            "markets": data.get("markets").cloned().unwrap_or(Value::Null),
        }))
    }

    pub(crate) fn get_vaults(&self) -> Result<Value, String> {
        let query = r#"
            query {
                vaults(first: 100) {
                    items {
                        address
                        name
                        symbol
                        asset {
                            symbol
                            address
                        }
                        state {
                            apy
                            netApy
                            totalAssetsUsd
                            allocation {
                                market {
                                    uniqueKey
                                    collateralAsset {
                                        symbol
                                    }
                                    loanAsset {
                                        symbol
                                    }
                                }
                            }
                        }
                    }
                }
            }
        "#;
        let data = self.post_graphql(query, None, "get_vaults")?;
        Ok(json!({
            "source": "morpho",
            "vaults": data.get("vaults").cloned().unwrap_or(Value::Null),
        }))
    }

    pub(crate) fn get_user_positions(&self, address: &str) -> Result<Value, String> {
        let query = r#"
            query GetUserPositions($address: String!) {
                userByAddress(address: $address) {
                    address
                    marketPositions {
                        market {
                            uniqueKey
                            collateralAsset {
                                symbol
                            }
                            loanAsset {
                                symbol
                            }
                        }
                        supplyAssetsUsd
                        borrowAssetsUsd
                        collateralUsd
                    }
                    vaultPositions {
                        vault {
                            address
                            name
                            symbol
                        }
                        assetsUsd
                    }
                }
            }
        "#;
        let variables = json!({ "address": address });
        let data = self.post_graphql(query, Some(variables), "get_user_positions")?;
        Ok(json!({
            "source": "morpho",
            "user": data.get("userByAddress").cloned().unwrap_or(Value::Null),
        }))
    }
}

// ============================================================================
// Tool arg structs
// ============================================================================

pub(crate) struct GetMorphoMarkets;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetMorphoMarketsArgs {}

pub(crate) struct GetMorphoVaults;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetMorphoVaultsArgs {}

pub(crate) struct GetMorphoUserPositions;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetMorphoUserPositionsArgs {
    /// Ethereum wallet address (e.g. "0xabc...def")
    pub(crate) address: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Story: "Deposit USDC into the highest-yield Morpho vault"
    /// Fetch vaults and markets, then compare vault APY vs direct market supply APY
    /// to choose the best deposit option.
    #[test]
    fn deposit_best_vault_workflow() {
        let client = MorphoClient::new().expect("failed to create MorphoClient");

        // Step 1: Get vaults and assert we get vault data with APY and TVL.
        println!("[step 1] Fetching vaults...");
        let vaults_response = client.get_vaults().expect("get_vaults failed");
        let vault_items = vaults_response["vaults"]["items"]
            .as_array()
            .expect("vaults response should contain items array");
        assert!(!vault_items.is_empty(), "should have at least one vault");
        println!("[step 1] Got {} vaults", vault_items.len());

        let first_vault = &vault_items[0];
        let first_vault_name = first_vault["name"].as_str().unwrap_or("unknown");
        let first_vault_apy = first_vault["state"]["netApy"]
            .as_f64()
            .or_else(|| first_vault["state"]["apy"].as_f64());
        println!("[step 1] First vault: name={first_vault_name}, apy={first_vault_apy:?}");
        assert!(
            first_vault["state"]["apy"].is_number() || first_vault["state"]["netApy"].is_number(),
            "vault should have APY data"
        );
        assert!(
            first_vault["state"]["totalAssetsUsd"].is_number()
                || first_vault["state"]["totalAssetsUsd"].is_string(),
            "vault should have TVL data"
        );

        // Step 2: Get markets and assert we get market data with supply/borrow rates.
        println!("[step 2] Fetching markets...");
        let markets_response = client.get_markets().expect("get_markets failed");
        let market_items = markets_response["markets"]["items"]
            .as_array()
            .expect("markets response should contain items array");
        assert!(!market_items.is_empty(), "should have at least one market");
        println!("[step 2] Got {} markets", market_items.len());

        let first_market = &market_items[0];
        let first_market_key = first_market["uniqueKey"].as_str().unwrap_or("unknown");
        let first_market_supply_apy = first_market["state"]["supplyApy"].as_f64();
        let first_market_borrow_apy = first_market["state"]["borrowApy"].as_f64();
        println!(
            "[step 2] First market: key={first_market_key}, supplyApy={first_market_supply_apy:?}, borrowApy={first_market_borrow_apy:?}"
        );
        assert!(
            first_market["state"]["supplyApy"].is_number(),
            "market should have supply APY"
        );
        assert!(
            first_market["state"]["borrowApy"].is_number(),
            "market should have borrow APY"
        );

        // Step 3: Assert we can compare vault APY vs direct market supply APY.
        println!("[step 3] Comparing vault APY vs direct market supply APY...");
        let best_vault_apy = vault_items
            .iter()
            .filter_map(|v| {
                v["state"]["netApy"]
                    .as_f64()
                    .or_else(|| v["state"]["apy"].as_f64())
            })
            .fold(f64::NEG_INFINITY, f64::max);

        let best_market_supply_apy = market_items
            .iter()
            .filter_map(|m| m["state"]["supplyApy"].as_f64())
            .fold(f64::NEG_INFINITY, f64::max);

        println!(
            "[step 3] Best vault APY: {best_vault_apy:.4}, best market supply APY: {best_market_supply_apy:.4}"
        );

        assert!(
            best_vault_apy.is_finite(),
            "should be able to extract at least one vault APY"
        );
        assert!(
            best_market_supply_apy.is_finite(),
            "should be able to extract at least one market supply APY"
        );

        // Step 4: Assert we have enough data to choose the best deposit option.
        let best_option = if best_vault_apy >= best_market_supply_apy {
            "vault"
        } else {
            "direct_market"
        };
        println!(
            "[step 4] Best deposit option: {best_option} (vault_apy={best_vault_apy:.4}, market_apy={best_market_supply_apy:.4})"
        );
    }

    /// Story: "Borrow against my ETH collateral at the best rate"
    /// Fetch markets and user positions, then identify the cheapest borrow market
    /// and verify we have enough info to calculate a safe borrow amount.
    #[test]
    fn borrow_against_collateral_workflow() {
        let client = MorphoClient::new().expect("failed to create MorphoClient");

        // Step 1: Get markets and assert we get markets with LTV and borrow APY data.
        println!("[step 1] Fetching markets...");
        let markets_response = client.get_markets().expect("get_markets failed");
        let market_items = markets_response["markets"]["items"]
            .as_array()
            .expect("markets response should contain items array");
        assert!(!market_items.is_empty(), "should have at least one market");
        println!("[step 1] Got {} markets", market_items.len());

        let markets_with_ltv: Vec<&Value> = market_items
            .iter()
            .filter(|m| m["lltv"].is_number() || m["lltv"].is_string())
            .collect();
        assert!(
            !markets_with_ltv.is_empty(),
            "should have at least one market with LTV data"
        );
        println!(
            "[step 1] Found {} markets with LTV data",
            markets_with_ltv.len()
        );

        let first = &markets_with_ltv[0];
        let first_key = first["uniqueKey"].as_str().unwrap_or("unknown");
        let first_lltv = &first["lltv"];
        let first_borrow_apy = first["state"]["borrowApy"].as_f64();
        println!(
            "[step 1] First LTV market: key={first_key}, lltv={first_lltv}, borrowApy={first_borrow_apy:?}"
        );
        assert!(
            first["state"]["borrowApy"].is_number(),
            "market should have borrow APY"
        );

        // Step 2: Get user positions for a zero address and assert response structure.
        let zero_addr = "0x0000000000000000000000000000000000000000";
        println!("[step 2] Fetching user positions for {zero_addr}...");
        let positions_response = client
            .get_user_positions(zero_addr)
            .expect("get_user_positions failed");
        assert_eq!(
            positions_response["source"]
                .as_str()
                .expect("should have source"),
            "morpho",
            "response source should be morpho"
        );
        // The zero address will likely have no positions; that is fine.
        // We just verify the response structure is present.
        let user = &positions_response["user"];
        let user_is_null = user.is_null();
        println!("[step 2] User response is_null={user_is_null}");
        if !user_is_null {
            // If the API returns a user object, it should have position arrays.
            let market_pos_count = user
                .get("marketPositions")
                .and_then(|p| p.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let vault_pos_count = user
                .get("vaultPositions")
                .and_then(|p| p.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            println!(
                "[step 2] User has {market_pos_count} market positions, {vault_pos_count} vault positions"
            );
            assert!(
                user.get("marketPositions").is_some() || user.get("vaultPositions").is_some(),
                "user object should contain position fields"
            );
        }

        // Step 3: Assert we can identify the cheapest borrow market from the data.
        println!("[step 3] Identifying cheapest borrow market...");
        let cheapest_borrow = market_items
            .iter()
            .filter_map(|m| {
                let apy = m["state"]["borrowApy"].as_f64()?;
                let key = m["uniqueKey"].as_str()?;
                Some((key, apy))
            })
            .filter(|(_, apy)| *apy > 0.0)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        let (cheapest_key, cheapest_apy) =
            cheapest_borrow.expect("should find at least one market with a positive borrow APY");
        println!(
            "[step 3] Cheapest borrow market: key={cheapest_key}, borrowApy={cheapest_apy:.4}"
        );
        assert!(cheapest_apy > 0.0, "cheapest borrow APY should be positive");

        // Step 4: Assert we have enough info to calculate a safe borrow amount.
        // We need: LTV ratio, collateral value, and borrow APY.
        println!(
            "[step 4] Verifying cheapest market has required fields for safe borrow calculation..."
        );
        let cheapest_market = market_items
            .iter()
            .find(|m| m["uniqueKey"].as_str() == Some(cheapest_key))
            .expect("should find the cheapest market in items");

        let has_ltv = cheapest_market["lltv"].is_number() || cheapest_market["lltv"].is_string();
        let has_borrow_apy = cheapest_market["state"]["borrowApy"].is_number();
        let has_collateral_info = cheapest_market.get("collateralAsset").is_some();
        let collateral_symbol = cheapest_market["collateralAsset"]["symbol"]
            .as_str()
            .unwrap_or("unknown");
        let loan_symbol = cheapest_market["loanAsset"]["symbol"]
            .as_str()
            .unwrap_or("unknown");
        println!(
            "[step 4] Market {cheapest_key}: has_ltv={has_ltv}, has_borrow_apy={has_borrow_apy}, \
             has_collateral_info={has_collateral_info}, collateral={collateral_symbol}, loan={loan_symbol}"
        );

        assert!(
            has_ltv,
            "cheapest market should have LTV for safe borrow calculation"
        );
        assert!(has_borrow_apy, "cheapest market should have borrow APY");
        assert!(
            has_collateral_info,
            "cheapest market should have collateral asset info"
        );

        println!(
            "[step 4] Borrow workflow complete: cheapest_market={cheapest_key}, \
             borrow_apy={cheapest_apy:.4}, collateral={collateral_symbol}/{loan_symbol}"
        );
    }
}
