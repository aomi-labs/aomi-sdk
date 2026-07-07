//! Live smoke-test helpers. NOT used at runtime — all paths gated behind
//! integration tests that require an explicit private key in the environment.
//!
//! The helpers in this module bypass the `evm_commit_message` host route and sign
//! the action locally, so we can verify the full build → sign → submit
//! round-trip against the real `https://api.hyperliquid.xyz/exchange` endpoint
//! without needing a host wallet runtime.

use crate::client::lp::lp_client;
use crate::client::perps::{
    OrderInputs, build_cancel_action, build_exchange_body, build_order_action, parse_signature,
    perps_client, prepare_l1_action,
};
use crate::client::spot::spot_client;
use ethers::signers::{LocalWallet, Signer};
use ethers::types::H256;
use ethers::utils::keccak256;
use serde_json::{Value, json};
use std::env;

const SAFE_NOTIONAL_USD: f64 = 11.0; // just over the ~$10 exchange minimum
// 15% below mid: deep enough an Alo (post-only) buy never fills, shallow enough
// to clear Hyperliquid's price-deviation limit (~80% from mid) and the 5-sig-fig rule.
const FAR_OTM_FACTOR: f64 = 0.85;

/// Compute the EIP-712 digest for a Hyperliquid L1 Agent payload. Mirrors what
/// `hl_ranger::signature::agent::l1::Agent::encode_eip712()` does internally
/// (that struct is in a private module, so we replicate the math here).
fn l1_eip712_digest(connection_id: &[u8; 32]) -> [u8; 32] {
    // Domain separator
    let domain_typehash = keccak256(
        b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
    );
    let name_hash = keccak256(b"Exchange");
    let version_hash = keccak256(b"1");
    let mut chain_id = [0u8; 32];
    chain_id[24..32].copy_from_slice(&1337u64.to_be_bytes());
    let verifying_contract = [0u8; 32]; // 0x0 padded
    let mut domain_input = Vec::with_capacity(160);
    domain_input.extend_from_slice(&domain_typehash);
    domain_input.extend_from_slice(&name_hash);
    domain_input.extend_from_slice(&version_hash);
    domain_input.extend_from_slice(&chain_id);
    domain_input.extend_from_slice(&verifying_contract);
    let domain_separator = keccak256(&domain_input);

    // Agent struct hash
    let agent_typehash = keccak256(b"Agent(string source,bytes32 connectionId)");
    let source_hash = keccak256(b"a"); // mainnet source (matches client::MAINNET_SOURCE)
    let mut struct_input = Vec::with_capacity(96);
    struct_input.extend_from_slice(&agent_typehash);
    struct_input.extend_from_slice(&source_hash);
    struct_input.extend_from_slice(connection_id);
    let struct_hash = keccak256(&struct_input);

    // EIP-712 final digest
    let mut digest_input = Vec::with_capacity(66);
    digest_input.extend_from_slice(&[0x19, 0x01]);
    digest_input.extend_from_slice(&domain_separator);
    digest_input.extend_from_slice(&struct_hash);
    keccak256(&digest_input)
}

fn sign_l1_action(
    action: hl_ranger::Actions,
    wallet: &LocalWallet,
) -> Result<(Value, u64, String), String> {
    let (action_json, nonce, _typed_data) = prepare_l1_action(action, None)?;
    // Re-derive connection_id from the same nonce so the digest matches the one
    // that went on the wire. We round-trip the action through Actions to reuse
    // hl_ranger's msgpack canonicalization rather than trying to msgpack a Value.
    let cid = recompute_connection_id(&action_json, nonce)?;
    let digest = l1_eip712_digest(&cid);
    let sig = wallet
        .sign_hash(H256::from(digest))
        .map_err(|e| format!("[byreal] local sign failed: {e}"))?;
    let sig_bytes = sig.to_vec();
    if sig_bytes.len() != 65 {
        return Err(format!(
            "[byreal] expected 65-byte signature, got {}",
            sig_bytes.len()
        ));
    }
    Ok((action_json, nonce, format!("0x{}", hex::encode(sig_bytes))))
}

/// Re-hash the action JSON with the given nonce so the digest we sign matches
/// what the connection_id committed to. We round-trip through `Actions` to
/// reuse hl_ranger's msgpack canonicalization rules — going through the JSON
/// directly would risk diverging on field ordering or integer width.
fn recompute_connection_id(action_json: &Value, nonce: u64) -> Result<[u8; 32], String> {
    let action: hl_ranger::Actions = serde_json::from_value(action_json.clone())
        .map_err(|e| format!("[byreal] action round-trip deserialize failed: {e}"))?;
    let cid = action
        .hash(nonce, None)
        .map_err(|e| format!("[byreal] action re-hash failed: {e}"))?;
    let mut out = [0u8; 32];
    out.copy_from_slice(cid.as_bytes());
    Ok(out)
}

/// Place a tiny far-out-of-market post-only ETH limit buy with the supplied
/// (or env-supplied) private key, then immediately cancel it. Verifies the
/// full build → sign → submit → cancel loop end-to-end without risking a fill.
///
/// Reads `BANANA_PRIVATE_KEY` if `private_key` is None.
pub fn place_live_smoke_order(private_key: Option<String>) -> Result<Value, String> {
    let pk = private_key
        .or_else(|| env::var("BANANA_PRIVATE_KEY").ok())
        .ok_or_else(|| "BANANA_PRIVATE_KEY not set and no private_key arg supplied".to_string())?;
    let pk_trim = pk.trim().trim_start_matches("0x");
    let wallet: LocalWallet = pk_trim
        .parse()
        .map_err(|e| format!("[byreal] invalid private key: {e}"))?;
    let address = format!("{:#x}", wallet.address());

    let client = perps_client()?;

    // 1. Fetch live ETH mid price.
    let mids = client.get_all_mids()?;
    let mid_str = mids
        .get("ETH")
        .and_then(Value::as_str)
        .ok_or_else(|| "[byreal] ETH not in get_all_mids response".to_string())?;
    let mid: f64 = mid_str
        .parse()
        .map_err(|e| format!("[byreal] could not parse ETH mid '{mid_str}': {e}"))?;

    // 2. Look up ETH szDecimals so we can round size to the asset's precision.
    let meta = client.get_meta()?;
    let universe = meta
        .get("universe")
        .and_then(Value::as_array)
        .ok_or_else(|| "[byreal] meta missing universe".to_string())?;
    let eth_meta = universe
        .iter()
        .find(|a| a.get("name").and_then(Value::as_str) == Some("ETH"))
        .ok_or_else(|| "[byreal] ETH not in universe".to_string())?;
    let sz_decimals = eth_meta
        .get("szDecimals")
        .and_then(Value::as_u64)
        .ok_or_else(|| "[byreal] ETH meta missing szDecimals".to_string())?
        as u32;

    // 3. Compute a safe size (just over $10 minimum) rounded to szDecimals.
    let raw_sz = SAFE_NOTIONAL_USD / mid;
    let factor = 10f64.powi(sz_decimals as i32);
    let sz = (raw_sz * factor).ceil() / factor;

    // 4. Far-OTM buy that won't fill, post-only. Round to ETH tick size ($0.10)
    // so we don't trip Hyperliquid's tick-size or 5-sig-fig price rules.
    let raw_px = mid * FAR_OTM_FACTOR;
    let limit_px = (raw_px * 10.0).round() / 10.0;

    let coin_to_asset = client.coin_to_asset()?;
    let order_action = build_order_action(
        &OrderInputs {
            coin: "ETH",
            is_buy: true,
            limit_px,
            sz,
            reduce_only: false,
            tif: "Alo".to_string(), // post-only — never crosses the spread
        },
        coin_to_asset,
    )?;
    let (order_json, order_nonce, order_sig_hex) = sign_l1_action(order_action, &wallet)?;
    let order_sig = parse_signature(&order_sig_hex)?;
    let order_body = build_exchange_body(order_json, order_nonce, &order_sig, None);
    let place_response = client.post_exchange(order_body)?;

    // 5. If the order rested, dig out the oid and cancel it.
    let oid = extract_resting_oid(&place_response);
    let cancel_response = if let Some(oid) = oid {
        let asset_index = client.lookup_asset("ETH")?;
        let cancel_action = build_cancel_action(asset_index, oid);
        let (cancel_json, cancel_nonce, cancel_sig_hex) = sign_l1_action(cancel_action, &wallet)?;
        let cancel_sig = parse_signature(&cancel_sig_hex)?;
        let cancel_body = build_exchange_body(cancel_json, cancel_nonce, &cancel_sig, None);
        Some(client.post_exchange(cancel_body)?)
    } else {
        None
    };

    Ok(json!({
        "wallet_address": address,
        "eth_mid_price": mid,
        "size": sz,
        "limit_px": limit_px,
        "tif": "Alo",
        "place_response": place_response,
        "resting_oid": oid,
        "cancel_response": cancel_response,
        "cleaned_up": cancel_response.is_some(),
    }))
}

fn extract_resting_oid(response: &Value) -> Option<u64> {
    response
        .get("response")?
        .get("data")?
        .get("statuses")?
        .as_array()?
        .first()?
        .get("resting")?
        .get("oid")?
        .as_u64()
}

// ===========================================================================
// byreal Solana side smoke helpers
//
// These hit the live `api2.byreal.io` API. No keys, no signing, no funds —
// just verifies our envelope unwrap, URL construction, and response shape
// work end-to-end, independent of the host's commit pipeline.
// ===========================================================================

/// Hit the spot reads we'll exercise most: global overview, top-TVL pools,
/// top-volume tokens. Returns the three responses for inspection.
pub fn smoke_byreal_spot_reads() -> Result<Value, String> {
    let client = spot_client()?;
    let overview = client.get_global_overview()?;
    let pools = client.list_pools(1, 5, "tvl", "desc", None, None, None)?;
    let tokens = client.list_tokens(1, 5, "volumeUsd24h", "desc", None, None, None)?;
    Ok(json!({
        "overview": overview,
        "pools_top5_by_tvl": pools,
        "tokens_top5_by_volume": tokens,
    }))
}

/// Hit byreal's swap-quote endpoint with a tiny USDC -> SOL quote and verify
/// the response carries an unsigned `transaction` ready to be signed. Does
/// NOT submit. `wallet` is required (the quote pre-builds the tx for that
/// signer), but no funds are required — byreal returns a quote even if the
/// wallet is empty.
pub fn smoke_byreal_swap_quote(wallet: &str) -> Result<Value, String> {
    let usdc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    let sol_mint = "So11111111111111111111111111111111111111112";
    let quote = spot_client()?.get_swap_quote(
        usdc_mint, sol_mint, "1000000", // 1 USDC
        "in", 100, // 1% slippage
        wallet,
    )?;
    let tx = quote
        .get("transaction")
        .and_then(Value::as_str)
        .ok_or_else(|| "[byreal] swap quote missing `transaction` field".to_string())?;
    if tx.is_empty() {
        return Err("[byreal] swap quote returned empty transaction".to_string());
    }
    Ok(json!({
        "router_type": quote.get("routerType"),
        "in_amount": quote.get("inAmount"),
        "out_amount": quote.get("outAmount"),
        "price_impact_pct": quote.get("priceImpactPct"),
        "unsigned_tx_len_b64": tx.len(),
        "quote_id": quote.get("quoteId"),
        "order_id": quote.get("orderId"),
    }))
}

/// Hit the Copy Farming top-performers endpoint — the marquee LP read.
/// Returns the top 5 by liquidity, no auth required.
pub fn smoke_byreal_lp_top_performers() -> Result<Value, String> {
    lp_client()?.list_top_positions(None, 1, 5, "liquidity", "desc", Some(0))
}
