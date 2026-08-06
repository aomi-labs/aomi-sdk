//! `token` — mint, list, or revoke platform/app activation tokens.
//!
//! `token mint` signs a privileged admin AomiBearer locally (from
//! `AOMI_ADMIN_KEY`); list/revoke run on the activation token mint produces.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand};

use super::shared::{
    ACTIVATION_TOKEN_ENV, ADMIN_KEY_ENV, ADMIN_KID_ENV, BACKEND_URL_ENV, env_value,
    resolve_activation, resolve_backend,
};
use crate::deploy::auth::AdminBearer;
use crate::deploy::backend::BackendClient;
use crate::deploy::platform::Platform;
use crate::deploy::types::MintTokenInput;

pub async fn run(args: TokenArgs) -> eyre::Result<()> {
    args.run().await.map_err(crate::to_eyre)
}

#[derive(Debug, Args, Clone)]
pub struct TokenArgs {
    #[command(subcommand)]
    pub cmd: TokenCmd,
}

#[derive(Debug, Subcommand, Clone)]
pub enum TokenCmd {
    /// Mint a platform/app activation token (signs an admin bearer from AOMI_ADMIN_KEY).
    Mint(TokenMintArgs),
    /// List a platform's activation tokens.
    List(TokenListArgs),
    /// Revoke a token by id.
    Revoke(TokenRevokeArgs),
}

impl TokenArgs {
    pub async fn run(self) -> Result<()> {
        match self.cmd {
            TokenCmd::Mint(a) => a.run().await,
            TokenCmd::List(a) => a.run().await,
            TokenCmd::Revoke(a) => a.run().await,
        }
    }
}

#[derive(Debug, Args, Clone)]
pub struct TokenMintArgs {
    /// Platform tag the token is scoped to.
    #[arg(long, value_name = "NAME")]
    pub platform: Platform,

    /// Token scope: `platform` or `app`.
    #[arg(long, default_value = "platform")]
    pub scope: String,

    /// App id for `--scope app`. Omit to mint an *unbound* app token that binds
    /// to the app it deploys first, then is locked to it 1-to-1.
    #[arg(long = "app-id", value_name = "ID")]
    pub app_id: Option<i64>,

    /// Backend base URL (default: `AOMI_BACKEND_URL`).
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// File holding the admin Ed25519 private key PEM. Falls back to
    /// `AOMI_ADMIN_KEY` (the PEM text itself, or a path to it).
    #[arg(long = "admin-key-file", value_name = "PATH")]
    pub admin_key_file: Option<PathBuf>,

    /// Admin issuer key id (e.g. `aomi-admin-staging-1`). Falls back to
    /// `AOMI_ADMIN_KID`.
    #[arg(long = "admin-kid", value_name = "KID")]
    pub admin_kid: Option<String>,

    /// Admin issuer name (the trusted `iss` on the backend).
    #[arg(long = "admin-iss", default_value = "aomi-admin")]
    pub admin_iss: String,

    /// Audience the backend accepts.
    #[arg(long = "admin-aud", default_value = "aomi-backend")]
    pub admin_aud: String,

    /// Subject recorded in the bearer.
    #[arg(long = "admin-sub", default_value = "aomi-build-cli")]
    pub admin_sub: String,

    /// Bearer lifetime, seconds.
    #[arg(long = "admin-ttl", value_name = "SECS", default_value_t = 900)]
    pub admin_ttl: i64,

    /// Print machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

impl TokenMintArgs {
    pub async fn run(self) -> Result<()> {
        // Fail fast: minting needs the privileged signing key. Resolve it (and
        // the kid) before any network call so a missing credential is an
        // immediate, clear error rather than a 401 round-trip.
        let key = resolve_admin_key(&self.admin_key_file)?;
        let kid = self
            .admin_kid
            .clone()
            .or_else(|| env_value(ADMIN_KID_ENV))
            .ok_or_else(|| {
                anyhow!(
                    "`token mint` requires the admin issuer key id — set --admin-kid or \
                     {ADMIN_KID_ENV} (e.g. aomi-admin-staging-1)."
                )
            })?;

        let scope = self.scope.trim().to_string();
        if scope != "app" && scope != "platform" {
            bail!("--scope must be `platform` or `app` (got `{scope}`)");
        }

        let backend_url = resolve_backend(&self.backend).ok_or_else(|| {
            anyhow!("token mint needs a backend URL — set --backend or {BACKEND_URL_ENV}")
        })?;

        let now = chrono::Utc::now().timestamp();
        let bearer = AdminBearer {
            private_key_pem: &key,
            kid: &kid,
            iss: &self.admin_iss,
            aud: &self.admin_aud,
            sub: &self.admin_sub,
            ttl_secs: self.admin_ttl,
        }
        .sign(now)?;

        let request = MintTokenInput {
            scope: scope.clone(),
            app_id: self.app_id,
        };
        let result = BackendClient::new(backend_url, bearer)?
            .mint_token(&self.platform, &request)
            .await?;

        if self.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "id": result.id,
                    "token": result.token,
                    "scope": result.scope,
                }))?
            );
        } else {
            println!(
                "Minted `{}` token id {} for platform `{}`",
                result.scope, result.id, self.platform
            );
            println!("  token: {}", result.token);
            println!(
                "  store this now — the backend keeps only the hash, the plaintext is shown once."
            );
            if scope == "app" && self.app_id.is_none() {
                println!(
                    "  (unbound app token — its first deploy binds it to that one app, 1-to-1)"
                );
            }
            println!();
            println!("Use it for deploy/activate:");
            println!("  export {ACTIVATION_TOKEN_ENV}={}", result.token);
        }
        Ok(())
    }
}

/// Resolve the admin signing key: `--admin-key-file`, else `AOMI_ADMIN_KEY`
/// (PEM text, or a path to a PEM file). This is the privileged, out-of-band
/// signing key — not an activation token.
fn resolve_admin_key(file: &Option<PathBuf>) -> Result<Vec<u8>> {
    if let Some(path) = file {
        return std::fs::read(path)
            .with_context(|| format!("failed to read admin key file {}", path.display()));
    }
    let raw = env_value(ADMIN_KEY_ENV).ok_or_else(|| {
        anyhow!(
            "`token mint` requires a privileged admin signing key. Set {ADMIN_KEY_ENV} \
             (a PKCS#8 Ed25519 private key PEM) or pass --admin-key-file. This is an \
             out-of-band admin/service signing key, not an activation token."
        )
    })?;
    if raw.contains("BEGIN") {
        Ok(raw.into_bytes())
    } else {
        std::fs::read(&raw).with_context(|| {
            format!("{ADMIN_KEY_ENV} is neither a PEM nor a readable file path: {raw}")
        })
    }
}

#[derive(Debug, Args, Clone)]
pub struct TokenListArgs {
    #[arg(long, value_name = "NAME")]
    pub platform: Platform,
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,
}

impl TokenListArgs {
    pub async fn run(self) -> Result<()> {
        let (url, token) = resolve_activation(&self.backend, &self.activation_token)?;
        let value = BackendClient::new(url, token)?
            .list_tokens(&self.platform)
            .await?;
        println!("{}", serde_json::to_string_pretty(&value)?);
        Ok(())
    }
}

#[derive(Debug, Args, Clone)]
pub struct TokenRevokeArgs {
    /// Token id to revoke.
    #[arg(value_name = "ID")]
    pub id: i64,
    #[arg(long, value_name = "NAME")]
    pub platform: Platform,
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,
}

impl TokenRevokeArgs {
    pub async fn run(self) -> Result<()> {
        let (url, token) = resolve_activation(&self.backend, &self.activation_token)?;
        BackendClient::new(url, token)?
            .revoke_token(&self.platform, self.id)
            .await?;
        println!(
            "Revoked token id {} on platform `{}`",
            self.id, self.platform
        );
        Ok(())
    }
}
