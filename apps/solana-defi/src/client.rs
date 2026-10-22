use aomi_sdk::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct SolanaDefiApp;

pub(crate) struct Client {
    http: reqwest::blocking::Client,
    endpoint: reqwest::Url,
}

impl Client {
    pub(crate) fn new() -> Result<Self, String> {
        let endpoint = std::env::var("AOMI_SOLANA_DEFI_URL")
            .map_err(|_| "AOMI_SOLANA_DEFI_URL must point to the preparation service")?;
        let endpoint =
            reqwest::Url::parse(&endpoint).map_err(|_| "Invalid preparation service URL")?;
        if !matches!(endpoint.scheme(), "http" | "https")
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(
                "Preparation service URL must use HTTP(S) without credentials or query".into(),
            );
        }
        Ok(Self {
            http: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(110))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| "Unable to initialize preparation service client")?,
            endpoint,
        })
    }

    pub(crate) fn call(&self, operation: &str, args: &impl Serialize) -> Result<Value, String> {
        let url = self
            .endpoint
            .join(operation)
            .map_err(|_| "Invalid operation URL")?;
        let response = self
            .http
            .post(url)
            .json(args)
            .send()
            .map_err(|_| "Solana preparation service unavailable")?;
        let status = response.status();
        if !status.is_success() {
            // Do not relay arbitrary upstream bodies or URLs into tool output.
            return Err(format!(
                "Solana preparation refused or unavailable (HTTP {})",
                status.as_u16()
            ));
        }
        response
            .json()
            .map_err(|_| "Invalid preparation service response".into())
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EmptyArgs {}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Venue {
    JupiterLend,
    KaminoEarn,
    Pumpswap,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MarketArgs {
    pub(crate) venue: Venue,
    /// Exact lending market, vault, or liquidity pool address, not a token symbol.
    pub(crate) market: String,
    /// Public address of the connected signing wallet. No private key.
    pub(crate) wallet: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Action {
    Deposit,
    Withdraw,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PrepareArgs {
    pub(crate) venue: Venue,
    pub(crate) market: String,
    pub(crate) wallet: String,
    pub(crate) action: Action,
    /// Positive integer string in raw asset units for deposits/Jupiter withdrawals,
    /// or raw share units for Kamino/PumpSwap withdrawals. Omit for withdraw_all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) amount_raw: Option<String>,
    /// Withdraw the current wallet position; amount_raw must be omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) withdraw_all: Option<bool>,
    /// PumpSwap price tolerance in basis points, 0 through 500. Default 50.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) slippage_bps: Option<u16>,
}
