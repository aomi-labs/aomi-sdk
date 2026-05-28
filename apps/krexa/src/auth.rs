//! Hand-written companion to the generated `client.rs`.
//!
//! Krexa runs two API tiers:
//!   - Anonymous: 30 req/min, no key needed. The public reads (score,
//!     credit eligibility, vault stats, KYA lookup, …) work in this tier.
//!   - Authenticated: 60+ req/min and access to the Pay.sh authenticated
//!     surface (`call`, `discover`, `history`, `budget`, `confirm`,
//!     `tier`, `proxy/*`). Carry `X-API-Key: kx_...` on every authed
//!     request.
//!
//! Beyond the static API key, three endpoints require **Ed25519
//! signatures** from a Solana keypair — and Krexa uses *three different
//! signing recipes* across them. They all share the ed25519-dalek
//! primitive but differ in (message bytes, signer, output encoding):
//!
//! | Endpoint                                     | Message                                                | Signer        | Encoding |
//! |----------------------------------------------|--------------------------------------------------------|---------------|----------|
//! | `POST /access/redeem` (invite redemption)    | UTF-8 of `"Krexa invite redemption: <code> for <wallet>"` | agent keypair | base58   |
//! | `POST /solana/credit/{agent}/request`        | **Raw 32 bytes** of the agent's pubkey                  | owner keypair | base64   |
//! | `POST /access/provision-key` (deferred)      | UTF-8 of `"Krexa API key provision for <wallet>"`       | wallet keypair | base58   |
//!
//! `sign_credit_request` is the easiest to get wrong: it does NOT sign a
//! string, and the encoding is base64 (not base58 like the others).
//! Confirmed with the Krexa team (`<msg-id>`); do not change the recipe
//! without re-confirming.
//!
//! Secrets resolve through `aomi_sdk::resolve_secret_value(ctx, ...)` —
//! vault first, env-var fallback. Operators set:
//!   - `KREXA_API_KEY` (`kx_...`)
//!   - `KREXA_AGENT_SECRET_KEY` — base58 64-byte Solana keypair used for
//!     invite redemption (the agent signs its own activation)
//!   - `KREXA_OWNER_SECRET_KEY` — base58 64-byte Solana keypair used for
//!     KYA `ownerSignature` (typically the same key as the agent for
//!     self-custodied agents)

use base64::Engine;

/// Env var that holds the `kx_`-prefixed key. Provisioned via
/// `POST /access/provision-key` (offline, by the operator) or returned
/// from `POST /solana/paysh/onboard`.
pub const API_KEY_ENV: &str = "KREXA_API_KEY";

/// Env var holding the **agent** keypair, base58-encoded as a 64-byte
/// Solana keypair (`solana-keygen` / Phantom default). Used to sign the
/// invite-redemption challenge.
pub const AGENT_SECRET_ENV: &str = "KREXA_AGENT_SECRET_KEY";

/// Env var holding the **owner** keypair, base58-encoded as a 64-byte
/// Solana keypair. Used to sign the KYA `ownerSignature` (the raw 32
/// bytes of the agent's pubkey). For self-custodied agents this is the
/// same key as `KREXA_AGENT_SECRET_KEY`.
pub const OWNER_SECRET_ENV: &str = "KREXA_OWNER_SECRET_KEY";

/// Resolve the API key from (in order) the per-app secret vault and the
/// `KREXA_API_KEY` env var. Returning `Err` only when both miss, with the
/// tool-shaped message the curated tool layer propagates with `?`.
pub fn api_key(ctx: &aomi_sdk::DynToolCallCtx) -> Result<String, String> {
    aomi_sdk::resolve_secret_value(
        ctx,
        None,
        API_KEY_ENV,
        &format!(
            "[krexa] {API_KEY_ENV} not set; Pay.sh authenticated endpoints \
             require an X-API-Key header (`kx_...`)."
        ),
    )
}

// ---------------------------------------------------------------------------
// Invite redemption — `POST /access/redeem`
// ---------------------------------------------------------------------------

/// Format the literal challenge the agent must sign before redeeming an
/// invite code, exactly as returned by `POST /access/challenge`.
pub fn invite_challenge(code: &str, wallet: &str) -> String {
    format!("Krexa invite redemption: {code} for {wallet}")
}

/// Sign the invite-redemption challenge with the **agent** keypair (from
/// `KREXA_AGENT_SECRET_KEY`). Output is the base58 64-byte signature, as
/// required by `POST /access/redeem`.
pub fn sign_invite_challenge(
    ctx: &aomi_sdk::DynToolCallCtx,
    code: &str,
    wallet: &str,
) -> Result<String, String> {
    let challenge = invite_challenge(code, wallet);
    let signature = sign_with_keypair(ctx, AGENT_SECRET_ENV, challenge.as_bytes())?;
    Ok(bs58::encode(signature).into_string())
}

// ---------------------------------------------------------------------------
// KYA ownerSignature — `POST /solana/credit/{agent}/request`
// ---------------------------------------------------------------------------
//
// Confirmed wire format (from the Krexa team):
//   1. Decode the agent's base58 pubkey to its raw 32 bytes.
//   2. Sign those 32 bytes (NOT a UTF-8 challenge string) with the
//      owner's keypair using ed25519.
//   3. Base64-encode the 64-byte signature.
//
// JS reference:
//   nacl.sign.detached(agentPubkey.toBytes(), ownerKeypair.secretKey)

/// Sign the KYA owner proof for `agent_pubkey`. Signs the raw 32-byte
/// agent pubkey with the owner keypair (`KREXA_OWNER_SECRET_KEY`) and
/// returns the base64-encoded signature — the value to drop into the
/// `ownerSignature` body field.
pub fn sign_credit_request(
    ctx: &aomi_sdk::DynToolCallCtx,
    agent_pubkey: &str,
) -> Result<String, String> {
    let agent_bytes = bs58::decode(agent_pubkey)
        .into_vec()
        .map_err(|e| format!("[krexa] agent_pubkey is not valid base58: {e}"))?;
    if agent_bytes.len() != 32 {
        return Err(format!(
            "[krexa] agent_pubkey decoded to {} bytes; expected 32 (Solana \
             pubkey)",
            agent_bytes.len()
        ));
    }
    let signature = sign_with_keypair(ctx, OWNER_SECRET_ENV, &agent_bytes)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(signature))
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Resolve a Solana keypair from the given env var (or vault), decode
/// it from base58, and sign `message` with Ed25519. Returns the raw
/// 64-byte signature — the caller picks the wire encoding.
fn sign_with_keypair(
    ctx: &aomi_sdk::DynToolCallCtx,
    env_var: &str,
    message: &[u8],
) -> Result<[u8; 64], String> {
    use ed25519_dalek::{Signer, SigningKey};

    let secret_b58 = aomi_sdk::resolve_secret_value(
        ctx,
        None,
        env_var,
        &format!(
            "[krexa] {env_var} not set; expected the wallet's secret key \
             as a base58-encoded 64-byte Solana keypair (`solana-keygen` \
             / Phantom default)."
        ),
    )?;

    let bytes = bs58::decode(&secret_b58)
        .into_vec()
        .map_err(|e| format!("[krexa] {env_var} is not valid base58: {e}"))?;

    if bytes.len() != 64 {
        return Err(format!(
            "[krexa] {env_var} decoded to {} bytes; expected 64 \
             (Solana keypair = 32 secret + 32 public)",
            bytes.len()
        ));
    }

    // ed25519-dalek derives the public key from the 32-byte seed; the
    // trailing 32 bytes (public key) of the Solana keypair are redundant
    // for signing.
    let seed: [u8; 32] = bytes[..32]
        .try_into()
        .expect("len check above guarantees 64 bytes");
    let signing_key = SigningKey::from_bytes(&seed);
    Ok(signing_key.sign(message).to_bytes())
}
