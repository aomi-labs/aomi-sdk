//! CLI-side EdDSA signer for the privileged **admin AomiBearer** that bootstraps
//! platform tokens (`token mint`).
//!
//! This is the minting twin of the backend's verify-only `aomi-service` path:
//! the backend holds the issuer's **public** key and only *verifies* admin
//! bearers; operator tooling holds the matching **private** key and *mints*
//! them. We vendor just the sign half here — claims shape + EdDSA encode — so
//! `aomi-build token mint` can present a `role = "admin"` bearer itself instead
//! of requiring a separate out-of-band signing step.
//!
//! The claims/header MUST match the backend's `AomiBearerClaims` and trusted
//! issuer config (`service.<env>.toml`) exactly, or verification fails:
//! `alg = EdDSA`, header `kid` selects the issuer key, and `iss`/`aud`/`role`
//! are authorized against that issuer.

use anyhow::{Context, Result};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;

/// Claims carried by an AomiBearer assertion — mirrors the backend's
/// `aomi_service::AomiBearerClaims`.
#[derive(Debug, Serialize)]
struct AomiBearerClaims {
    sub: String,
    iss: String,
    aud: String,
    role: String,
    iat: i64,
    exp: i64,
}

/// Inputs for signing a short-lived admin bearer. Everything except
/// `private_key_pem` is non-secret and per-environment (it comes from the
/// backend's trusted `aomi-admin` issuer in `service.<env>.toml`).
pub(crate) struct AdminBearer<'a> {
    /// PKCS#8 Ed25519 private key, PEM (`AOMI_ADMIN_KEY`). The secret.
    pub private_key_pem: &'a [u8],
    /// Key id; selects the issuer's verify key (e.g. `aomi-admin-staging-1`).
    pub kid: &'a str,
    /// Issuer identity (e.g. `aomi-admin`).
    pub iss: &'a str,
    /// Audience the backend accepts (e.g. `aomi-backend`).
    pub aud: &'a str,
    /// Subject — the operator/principal id recorded in the token.
    pub sub: &'a str,
    /// Lifetime in seconds.
    pub ttl_secs: i64,
}

impl AdminBearer<'_> {
    /// Sign a `role = "admin"` AomiBearer (EdDSA / Ed25519). `now` (unix
    /// seconds) is passed in so signing stays pure and testable.
    pub(crate) fn sign(&self, now: i64) -> Result<String> {
        let key = EncodingKey::from_ed_pem(self.private_key_pem).context(
            "AOMI_ADMIN_KEY is not a valid PKCS#8 Ed25519 private key (expected PEM)",
        )?;
        let claims = AomiBearerClaims {
            sub: self.sub.to_string(),
            iss: self.iss.to_string(),
            aud: self.aud.to_string(),
            role: "admin".to_string(),
            iat: now,
            exp: now + self.ttl_secs,
        };
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some(self.kid.to_string());
        encode(&header, &claims, &key).context("failed to sign admin AomiBearer")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test-only Ed25519 PKCS#8 private key (matches the service crate fixture).
    const PRIV_PEM: &[u8] = b"-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIA3YGS2n6pAbisXZxFbDPdncGRxMXI2m4eJN2gNSf+wi
-----END PRIVATE KEY-----
";

    #[test]
    fn signs_a_three_part_jwt_with_kid_header() {
        let token = AdminBearer {
            private_key_pem: PRIV_PEM,
            kid: "aomi-admin-staging-1",
            iss: "aomi-admin",
            aud: "aomi-backend",
            sub: "aomi-build-cli",
            ttl_secs: 900,
        }
        .sign(1_700_000_000)
        .expect("sign");

        // header.payload.signature
        assert_eq!(token.split('.').count(), 3);
        let header = jsonwebtoken::decode_header(&token).expect("decode header");
        assert_eq!(header.alg, Algorithm::EdDSA);
        assert_eq!(header.kid.as_deref(), Some("aomi-admin-staging-1"));
    }

    #[test]
    fn rejects_a_malformed_key() {
        let err = AdminBearer {
            private_key_pem: b"not a pem",
            kid: "k",
            iss: "aomi-admin",
            aud: "aomi-backend",
            sub: "s",
            ttl_secs: 900,
        }
        .sign(0)
        .unwrap_err();
        assert!(err.to_string().contains("AOMI_ADMIN_KEY"));
    }
}
