use crate::client::{ParaApp, para_client};
use crate::types::{CreateWalletRequest, WalletLookupResult};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[para] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("para".to_string()));
            Value::Object(map)
        }
        other => json!({ "source": "para", "data": other }),
    })
}

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

fn resolve_para_api_key(ctx: &DynToolCallCtx) -> Result<String, String> {
    ctx.attribute_string(&["para_api_key"])
        .or_else(|| ctx.attribute_string(&["PARA_API_KEY"]))
        .ok_or_else(|| {
            "para_api_key not found in context state_attributes — ensure the integration sets para_api_key or PARA_API_KEY".to_string()
        })
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct CreateParaWalletArgs {
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

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_para_api_key(&ctx)?;
        let payload = CreateWalletRequest {
            wallet_type: args.wallet_type,
            user_identifier: args.user_identifier,
            user_identifier_type: args.user_identifier_type,
            scheme: args.scheme,
            cosmos_prefix: args.cosmos_prefix,
        };
        ok(para_client()?.create_wallet(&api_key, &payload)?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetParaWalletArgs {
    /// Para wallet ID.
    wallet_id: String,
}

pub(crate) struct GetParaWallet;

impl DynAomiTool for GetParaWallet {
    type App = ParaApp;
    type Args = GetParaWalletArgs;

    const NAME: &'static str = "get_para_wallet";
    const DESCRIPTION: &'static str = "Fetch details for a single Para wallet by ID. Returns status, address, publicKey, and type.";

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_para_api_key(&ctx)?;
        ok(para_client()?.get_wallet(&api_key, &args.wallet_id)?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct ListParaWalletsArgs {
    /// Up to 10 wallet IDs to fetch.
    wallet_ids: Vec<String>,
}

pub(crate) struct ListParaWallets;

impl DynAomiTool for ListParaWallets {
    type App = ParaApp;
    type Args = ListParaWalletsArgs;

    const NAME: &'static str = "list_para_wallets";
    const DESCRIPTION: &'static str = "Batch-fetch multiple Para wallets by their IDs (max 10). Para has no native list endpoint, so wallets are fetched individually. Each result includes the wallet data or a per-item error.";

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = para_client()?;
        let api_key = resolve_para_api_key(&ctx)?;
        let wallet_ids: Vec<String> = args.wallet_ids.into_iter().take(10).collect();
        if wallet_ids.is_empty() {
            return Err("wallet_ids must be a non-empty array".to_string());
        }

        let wallets: Vec<WalletLookupResult> = wallet_ids
            .iter()
            .map(|wallet_id| match client.get_wallet(&api_key, wallet_id) {
                Ok(data) => WalletLookupResult {
                    wallet_id: wallet_id.clone(),
                    data: Some(data),
                    error: None,
                },
                Err(error) => WalletLookupResult {
                    wallet_id: wallet_id.clone(),
                    data: None,
                    error: Some(error),
                },
            })
            .collect();

        let count = wallets.len();
        ok(json!({ "wallets": wallets, "count": count }))
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct SignRawWithParaWalletArgs {
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

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_para_api_key(&ctx)?;
        validate_sign_raw_data(&args.data)?;
        ok(para_client()?.sign_raw(&api_key, &args.wallet_id, &args.data)?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct WaitForParaWalletReadyArgs {
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

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = para_client()?;
        let api_key = resolve_para_api_key(&ctx)?;
        let deadline = Instant::now() + Duration::from_millis(args.max_wait_ms.unwrap_or(30_000));

        loop {
            let wallet = client.get_wallet(&api_key, &args.wallet_id)?;
            let status = wallet
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");

            match status {
                "ready" => return ok(wallet),
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
    use super::{
        CreateParaWalletArgs, GetParaWalletArgs, ListParaWalletsArgs, SignRawWithParaWalletArgs,
        WaitForParaWalletReadyArgs, resolve_para_api_key, validate_sign_raw_data,
    };
    use crate::types::CreateWalletRequest;
    use aomi_sdk::testing::TestCtxBuilder;
    use serde_json::json;

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

    #[test]
    fn resolve_para_api_key_prefers_lowercase_context_key() {
        let ctx = TestCtxBuilder::new("get_para_wallet")
            .attribute("PARA_API_KEY", "para_upper")
            .attribute("para_api_key", "para_lower")
            .build();

        assert_eq!(resolve_para_api_key(&ctx).as_deref(), Ok("para_lower"));
    }

    #[test]
    fn resolve_para_api_key_falls_back_to_uppercase_context_key() {
        let ctx = TestCtxBuilder::new("get_para_wallet")
            .attribute("PARA_API_KEY", "para_upper")
            .build();

        assert_eq!(resolve_para_api_key(&ctx).as_deref(), Ok("para_upper"));
    }

    #[test]
    fn resolve_para_api_key_errors_when_missing() {
        let ctx = TestCtxBuilder::new("get_para_wallet").build();

        assert!(resolve_para_api_key(&ctx).is_err());
    }

    #[test]
    fn para_tool_args_no_longer_require_api_key() {
        let create: CreateParaWalletArgs = serde_json::from_value(json!({
            "wallet_type": "EVM",
            "user_identifier": "user@example.com",
            "user_identifier_type": "EMAIL"
        }))
        .expect("create args should deserialize without api_key");
        assert_eq!(create.wallet_type, "EVM");

        let get: GetParaWalletArgs = serde_json::from_value(json!({
            "wallet_id": "wallet-123"
        }))
        .expect("get args should deserialize without api_key");
        assert_eq!(get.wallet_id, "wallet-123");

        let list: ListParaWalletsArgs = serde_json::from_value(json!({
            "wallet_ids": ["wallet-123"]
        }))
        .expect("list args should deserialize without api_key");
        assert_eq!(list.wallet_ids, vec!["wallet-123"]);

        let sign: SignRawWithParaWalletArgs = serde_json::from_value(json!({
            "wallet_id": "wallet-123",
            "data": "0x1234"
        }))
        .expect("sign args should deserialize without api_key");
        assert_eq!(sign.data, "0x1234");

        let wait: WaitForParaWalletReadyArgs = serde_json::from_value(json!({
            "wallet_id": "wallet-123"
        }))
        .expect("wait args should deserialize without api_key");
        assert_eq!(wait.wallet_id, "wallet-123");
    }

    // ========================================================================
    // Integration tests — require PARA_API_KEY env var to run.
    //
    //   PARA_API_KEY=<key> cargo test -p para -- --ignored
    // ========================================================================

    use crate::client::ParaClient;

    fn create_wallet_request(user_identifier: String) -> CreateWalletRequest {
        CreateWalletRequest {
            wallet_type: "EVM".to_string(),
            user_identifier,
            user_identifier_type: "EMAIL".to_string(),
            scheme: None,
            cosmos_prefix: None,
        }
    }

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
        let body = create_wallet_request(uid);
        let created = client
            .create_wallet(&key, &body)
            .expect("create_wallet failed");

        let wallet_id = created["id"].as_str().expect("response missing 'id'");
        assert!(!wallet_id.is_empty(), "wallet id should not be empty");

        let fetched = client
            .get_wallet(&key, wallet_id)
            .expect("get_wallet failed");
        assert_eq!(fetched["id"].as_str(), Some(wallet_id));
        assert!(fetched.get("status").is_some());
    }

    #[test]
    #[ignore]
    fn get_wallet_not_found() {
        let client = client();
        let fake_id = "00000000-0000-0000-0000-000000000000";
        let err = client.get_wallet(&api_key(), fake_id).unwrap_err();
        assert!(err.contains("404"), "expected 404 error, got: {err}");
    }

    #[test]
    #[ignore]
    fn create_wallet_bad_api_key() {
        let client = client();
        let body = create_wallet_request("bad-key-test@aomi.test".to_string());
        let err = client.create_wallet("invalid-key", &body).unwrap_err();
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
        let body = create_wallet_request(uid);
        let created = client
            .create_wallet(&key, &body)
            .expect("create_wallet failed");
        let wallet_id = created["id"]
            .as_str()
            .expect("response missing 'id'")
            .to_string();

        // Poll until ready (max 30s).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let wallet = client
                .get_wallet(&key, &wallet_id)
                .expect("get_wallet failed");
            if wallet["status"].as_str().unwrap_or("unknown") == "ready" {
                break;
            }
            if std::time::Instant::now() >= deadline {
                panic!("wallet did not become ready within 30s");
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        let result = client
            .sign_raw(&key, &wallet_id, "0x48656c6c6f")
            .expect("sign_raw failed");
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

        let ids: Vec<String> = (0..2)
            .map(|i| {
                let uid = format!("test-list-{i}-{}@aomi.test", uuid::Uuid::new_v4());
                let body = create_wallet_request(uid);
                let created = client
                    .create_wallet(&key, &body)
                    .expect("create_wallet failed");
                created["id"].as_str().unwrap().to_string()
            })
            .collect();

        let wallets: Vec<serde_json::Value> = ids
            .iter()
            .map(|wallet_id| match client.get_wallet(&key, wallet_id) {
                Ok(data) => json!({ "wallet_id": wallet_id, "data": data }),
                Err(error) => json!({ "wallet_id": wallet_id, "error": error }),
            })
            .collect();

        assert_eq!(wallets.len(), 2);
        for w in &wallets {
            assert!(w.get("data").is_some(), "each entry should have data: {w}");
        }
    }
}
