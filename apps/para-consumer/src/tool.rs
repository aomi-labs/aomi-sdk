use crate::client::{ParaApp, para_client};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn validate_sign_raw_data(data: &str) -> Result<(), String> {
    if !data.starts_with("0x") {
        return Err(
            "sign_raw data must be a 0x-prefixed hex string. To sign text, convert it to hex first."
                .to_string(),
        );
    }

    let hex = &data[2..];
    if hex.is_empty() || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(
            "sign_raw data must be a valid 0x-prefixed hex string (for example 0x48656c6c6f)."
                .to_string(),
        );
    }

    Ok(())
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct CreateParaWalletArgs {
    /// User-supplied Para API key.
    api_key: String,
    /// Wallet type: EVM, SOLANA, or COSMOS.
    wallet_type: String,
    /// User identifier such as email, phone, or custom ID.
    user_identifier: String,
    /// Identifier type such as EMAIL, PHONE, CUSTOM_ID, GUEST_ID, TELEGRAM, DISCORD, or TWITTER.
    user_identifier_type: String,
    /// Optional signature scheme such as DKLS, CGGMP, or ED25519.
    scheme: Option<String>,
    /// Optional bech32 prefix for Cosmos wallets.
    cosmos_prefix: Option<String>,
}

pub(crate) struct CreateParaWallet;

impl DynAomiTool for CreateParaWallet {
    type App = ParaApp;
    type Args = CreateParaWalletArgs;

    const NAME: &'static str = "create_para_wallet";
    const DESCRIPTION: &'static str = "Create a new Para MPC wallet. Supports EVM, Solana, and Cosmos chains. The wallet is created asynchronously — status starts as 'creating' and transitions to 'ready' once MPC key generation completes. Use wait_for_para_wallet_ready to poll until ready.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = para_client()?;
        let mut payload = json!({
            "type": args.wallet_type,
            "userIdentifier": args.user_identifier,
            "userIdentifierType": args.user_identifier_type,
        });

        if let Some(scheme) = args.scheme {
            payload["scheme"] = Value::String(scheme);
        }
        if let Some(prefix) = args.cosmos_prefix {
            payload["cosmosPrefix"] = Value::String(prefix);
        }

        client.create_wallet(&args.api_key, payload)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetParaWalletArgs {
    /// User-supplied Para API key.
    api_key: String,
    /// Para wallet ID.
    wallet_id: String,
}

pub(crate) struct GetParaWallet;

impl DynAomiTool for GetParaWallet {
    type App = ParaApp;
    type Args = GetParaWalletArgs;

    const NAME: &'static str = "get_para_wallet";
    const DESCRIPTION: &'static str = "Fetch details for a single Para wallet by ID. Returns status, address, publicKey, and type.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        para_client()?.get_wallet(&args.api_key, &args.wallet_id)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct ListParaWalletsArgs {
    /// User-supplied Para API key.
    api_key: String,
    /// Up to 10 wallet IDs to fetch.
    wallet_ids: Vec<String>,
}

pub(crate) struct ListParaWallets;

impl DynAomiTool for ListParaWallets {
    type App = ParaApp;
    type Args = ListParaWalletsArgs;

    const NAME: &'static str = "list_para_wallets";
    const DESCRIPTION: &'static str = "Batch-fetch multiple Para wallets by their IDs (max 10). Para has no native list endpoint, so wallets are fetched individually. Each result includes the wallet data or a per-item error.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = para_client()?;
        let wallet_ids: Vec<String> = args.wallet_ids.into_iter().take(10).collect();
        if wallet_ids.is_empty() {
            return Err("wallet_ids must be a non-empty array".to_string());
        }

        let wallets: Vec<Value> = wallet_ids
            .iter()
            .map(
                |wallet_id| match client.get_wallet(&args.api_key, wallet_id) {
                    Ok(data) => json!({ "wallet_id": wallet_id, "data": data }),
                    Err(error) => json!({ "wallet_id": wallet_id, "error": error }),
                },
            )
            .collect();

        let count = wallets.len();
        Ok(json!({ "wallets": wallets, "count": count }))
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct SignRawWithParaWalletArgs {
    /// User-supplied Para API key.
    api_key: String,
    /// Para wallet ID to sign with.
    wallet_id: String,
    /// 0x-prefixed hex data to sign.
    data: String,
}

pub(crate) struct SignRawWithParaWallet;

impl DynAomiTool for SignRawWithParaWallet {
    type App = ParaApp;
    type Args = SignRawWithParaWalletArgs;

    const NAME: &'static str = "sign_raw_with_para_wallet";
    const DESCRIPTION: &'static str = "Sign arbitrary raw data with a Para MPC wallet. The data must be a 0x-prefixed hex string. The wallet must have status 'ready' before signing.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        validate_sign_raw_data(&args.data)?;
        para_client()?.sign_raw(&args.api_key, &args.wallet_id, &args.data)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct WaitForParaWalletReadyArgs {
    /// User-supplied Para API key.
    api_key: String,
    /// Para wallet ID to poll.
    wallet_id: String,
    /// Maximum wait time in milliseconds.
    max_wait_ms: Option<u64>,
}

pub(crate) struct WaitForParaWalletReady;

impl DynAomiTool for WaitForParaWalletReady {
    type App = ParaApp;
    type Args = WaitForParaWalletReadyArgs;

    const NAME: &'static str = "wait_for_para_wallet_ready";
    const DESCRIPTION: &'static str = "Poll a Para wallet every 2 seconds until its status becomes 'ready' (MPC key generation complete). Returns wallet details when ready, or an error on timeout or wallet creation failure.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = para_client()?;
        let deadline = Instant::now() + Duration::from_millis(args.max_wait_ms.unwrap_or(30_000));

        loop {
            let wallet = client.get_wallet(&args.api_key, &args.wallet_id)?;
            let status = wallet
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");

            match status {
                "ready" => return Ok(wallet),
                "error" => {
                    return Err(format!(
                        "Para wallet '{}' creation failed with status 'error'. Try creating a new wallet.",
                        args.wallet_id
                    ));
                }
                _ => {}
            }

            if Instant::now() >= deadline {
                return Err(format!(
                    "Wallet '{}' did not become ready within {}ms. Current status is still '{}'. You can call wait_for_para_wallet_ready again to keep polling.",
                    args.wallet_id,
                    args.max_wait_ms.unwrap_or(30_000),
                    status
                ));
            }

            std::thread::sleep(Duration::from_secs(2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate_sign_raw_data;

    #[test]
    fn validate_sign_raw_data_rejects_non_hex_inputs() {
        assert!(validate_sign_raw_data("hello").is_err());
        assert!(validate_sign_raw_data("0xzz").is_err());
        assert!(validate_sign_raw_data("0x").is_err());
    }

    #[test]
    fn validate_sign_raw_data_accepts_hex_payload() {
        assert!(validate_sign_raw_data("0x48656c6c6f").is_ok());
    }

    // ========================================================================
    // Integration tests — require PARA_API_KEY env var to run.
    //
    //   PARA_API_KEY=<key> cargo test -p para-consumer -- --ignored
    // ========================================================================

    use crate::client::ParaClient;
    use serde_json::json;

    fn api_key() -> String {
        std::env::var("PARA_API_KEY").expect("PARA_API_KEY must be set for integration tests")
    }

    fn client() -> ParaClient {
        ParaClient::new().expect("failed to build ParaClient")
    }

    #[test]
    #[ignore]
    fn create_and_get_wallet() {
        let client = client();
        let key = api_key();

        let uid = format!("test-{}@aomi.test", uuid::Uuid::new_v4());
        let body = json!({
            "type": "EVM",
            "userIdentifier": uid,
            "userIdentifierType": "EMAIL",
        });
        println!("\n>>> POST /v1/wallets");
        println!(
            "    request body: {}",
            serde_json::to_string_pretty(&body).unwrap()
        );

        let created = client
            .create_wallet(&key, body)
            .expect("create_wallet failed");
        println!(
            "    response: {}",
            serde_json::to_string_pretty(&created).unwrap()
        );

        let wallet_id = created["id"].as_str().expect("response missing 'id'");
        assert!(!wallet_id.is_empty(), "wallet id should not be empty");

        println!("\n>>> GET /v1/wallets/{wallet_id}");
        let fetched = client
            .get_wallet(&key, wallet_id)
            .expect("get_wallet failed");
        println!(
            "    response: {}",
            serde_json::to_string_pretty(&fetched).unwrap()
        );

        assert_eq!(fetched["id"].as_str(), Some(wallet_id));
        assert!(
            fetched.get("status").is_some(),
            "wallet response should include 'status'"
        );
    }

    #[test]
    #[ignore]
    fn get_wallet_not_found() {
        let client = client();
        let fake_id = "00000000-0000-0000-0000-000000000000";
        println!("\n>>> GET /v1/wallets/{fake_id}");
        let err = client.get_wallet(&api_key(), fake_id).unwrap_err();
        println!("    error: {err}");
        assert!(err.contains("404"), "expected 404 error, got: {err}");
    }

    #[test]
    #[ignore]
    fn create_wallet_bad_api_key() {
        let client = client();
        let body = json!({
            "type": "EVM",
            "userIdentifier": "bad-key-test@aomi.test",
            "userIdentifierType": "EMAIL",
        });
        println!("\n>>> POST /v1/wallets (bad api key)");
        println!(
            "    request body: {}",
            serde_json::to_string_pretty(&body).unwrap()
        );

        let err = client.create_wallet("invalid-key", body).unwrap_err();
        println!("    error: {err}");
        assert!(
            err.contains("401") || err.contains("403"),
            "expected 401/403 error, got: {err}"
        );
    }

    #[test]
    #[ignore]
    fn sign_raw_with_wallet() {
        let client = client();
        let key = api_key();

        let uid = format!("test-sign-{}@aomi.test", uuid::Uuid::new_v4());
        let body = json!({
            "type": "EVM",
            "userIdentifier": uid,
            "userIdentifierType": "EMAIL",
        });
        println!("\n>>> POST /v1/wallets (create for signing)");
        println!(
            "    request body: {}",
            serde_json::to_string_pretty(&body).unwrap()
        );

        let created = client
            .create_wallet(&key, body)
            .expect("create_wallet failed");
        println!(
            "    response: {}",
            serde_json::to_string_pretty(&created).unwrap()
        );

        let wallet_id = created["id"]
            .as_str()
            .expect("response missing 'id'")
            .to_string();

        // Poll until ready (max 30s).
        println!("\n>>> polling GET /v1/wallets/{wallet_id} until ready...");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let wallet = client
                .get_wallet(&key, &wallet_id)
                .expect("get_wallet failed");
            let status = wallet["status"].as_str().unwrap_or("unknown");
            println!("    status: {status}");
            if status == "ready" {
                println!(
                    "    wallet ready: {}",
                    serde_json::to_string_pretty(&wallet).unwrap()
                );
                break;
            }
            if std::time::Instant::now() >= deadline {
                panic!("wallet did not become ready within 30s");
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        let sign_data = "0x48656c6c6f";
        println!("\n>>> POST /v1/wallets/{wallet_id}/sign-raw");
        println!("    request body: {{\"data\": \"{sign_data}\"}}");

        let result = client
            .sign_raw(&key, &wallet_id, sign_data)
            .expect("sign_raw failed");
        println!(
            "    response: {}",
            serde_json::to_string_pretty(&result).unwrap()
        );

        assert!(
            result.get("signature").is_some() || result.get("sig").is_some(),
            "sign response should include a signature field: {result}"
        );
    }

    #[test]
    #[ignore]
    fn list_wallets_fetches_multiple() {
        let client = client();
        let key = api_key();

        println!("\n>>> creating 2 wallets...");
        let ids: Vec<String> = (0..2)
            .map(|i| {
                let uid = format!("test-list-{i}-{}@aomi.test", uuid::Uuid::new_v4());
                let body = json!({
                    "type": "EVM",
                    "userIdentifier": uid,
                    "userIdentifierType": "EMAIL",
                });
                println!(
                    "    POST /v1/wallets body: {}",
                    serde_json::to_string_pretty(&body).unwrap()
                );
                let created = client
                    .create_wallet(&key, body)
                    .expect("create_wallet failed");
                println!(
                    "    response: {}",
                    serde_json::to_string_pretty(&created).unwrap()
                );
                created["id"].as_str().unwrap().to_string()
            })
            .collect();

        println!("\n>>> fetching {} wallets sequentially...", ids.len());
        let wallets: Vec<serde_json::Value> = ids
            .iter()
            .map(|wallet_id| {
                println!("    GET /v1/wallets/{wallet_id}");
                match client.get_wallet(&key, wallet_id) {
                    Ok(data) => {
                        println!(
                            "    response: {}",
                            serde_json::to_string_pretty(&data).unwrap()
                        );
                        json!({ "wallet_id": wallet_id, "data": data })
                    }
                    Err(error) => {
                        println!("    error: {error}");
                        json!({ "wallet_id": wallet_id, "error": error })
                    }
                }
            })
            .collect();

        assert_eq!(wallets.len(), 2);
        for w in &wallets {
            assert!(w.get("data").is_some(), "each entry should have data: {w}");
        }
    }
}
