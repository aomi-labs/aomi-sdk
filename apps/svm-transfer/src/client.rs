//! HTTP plumbing for the configurable cluster RPC + the app marker
//! struct `SvmTransferApp`.
//!
//! Lane 1 doesn't need an RPC client (the host attaches blockhash at
//! commit time), but Lane 2 builds the full VersionedTransaction
//! client-side and needs a fresh blockhash before it can produce a
//! blob the host's stage_tx will accept. Both tools share this app
//! marker so the macro can register them off a single trait impl.

use serde::Deserialize;

const CLUSTER_ENV: &str = "SVM_TRANSFER_CLUSTER";
const DEFAULT_CLUSTER: &str = "devnet";

const DEVNET_RPC: &str = "https://api.devnet.solana.com";
const TESTNET_RPC: &str = "https://api.testnet.solana.com";
const MAINNET_RPC: &str = "https://api.mainnet-beta.solana.com";

/// Marker struct. `Default` is what the macro registration path uses.
#[derive(Debug, Clone, Default)]
pub struct SvmTransferApp;

/// Resolve the configured cluster name and its public RPC URL.
///
/// We don't accept a custom URL via env — the smoke test should run
/// against the public Solana endpoints. Apps that want their own RPC
/// should ship that as a separate concern.
pub(crate) fn rpc_url() -> (&'static str, &'static str) {
    let cluster = std::env::var(CLUSTER_ENV).unwrap_or_else(|_| DEFAULT_CLUSTER.to_string());
    match cluster.as_str() {
        "mainnet" | "mainnet-beta" => ("mainnet-beta", MAINNET_RPC),
        "testnet" => ("testnet", TESTNET_RPC),
        // Default: anything we don't recognize falls through to devnet
        // — that's the cluster smoke tests should run on.
        _ => ("devnet", DEVNET_RPC),
    }
}

/// The host's expected `cluster` string on staged records (see
/// `Cluster::from_str` on the host side). Matches what `aomi-cli`
/// derives from `--cluster`.
pub(crate) fn cluster_id() -> &'static str {
    let (name, _) = rpc_url();
    match name {
        "mainnet-beta" => "solana:mainnet",
        "testnet" => "solana:testnet",
        _ => "solana:devnet",
    }
}

#[derive(Deserialize)]
struct GetLatestBlockhashEnvelope {
    result: GetLatestBlockhashResult,
}

#[derive(Deserialize)]
struct GetLatestBlockhashResult {
    value: GetLatestBlockhashValue,
}

#[derive(Deserialize)]
struct GetLatestBlockhashValue {
    blockhash: String,
    #[serde(default)]
    #[allow(dead_code)] // surfaced for future use; the host commit_tx
                       // also fetches and stamps last_valid_block_height
                       // when blob carries none.
    last_valid_block_height: Option<u64>,
}

/// Fetch the latest blockhash from the configured cluster's RPC.
/// Blocking — Lane 2's tool body is sync (DynAomiTool::run is sync; the
/// macro wires it into the async dispatcher). Devnet RPC is fast enough
/// (~150-250ms) that the blocking call is fine for a smoke test.
pub(crate) fn fetch_recent_blockhash() -> Result<String, String> {
    let (_, url) = rpc_url();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getLatestBlockhash",
        "params": [{"commitment": "finalized"}],
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("[svm-transfer] reqwest client init failed: {e}"))?;
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .map_err(|e| format!("[svm-transfer] {url} getLatestBlockhash request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("[svm-transfer] {url} returned non-2xx: {e}"))?
        .json::<GetLatestBlockhashEnvelope>()
        .map_err(|e| format!("[svm-transfer] {url} returned malformed JSON: {e}"))?;
    Ok(resp.result.value.blockhash)
}
