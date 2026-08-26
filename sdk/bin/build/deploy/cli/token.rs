//! `token mint` — mint an activation token for an app developer.
//!
//! Operator-only. Minting signs a short-lived privileged admin AomiBearer
//! locally from `AOMI_ADMIN_KEY` / `AOMI_ADMIN_KID`; the backend verifies that
//! signature against its trusted issuer set. That signature — not the fact the
//! command is hidden from `--help` — is the authorization boundary.
//!
//! Listing and revoking stay backend-only (`GET`/`DELETE
//! /api/platforms/:name/tokens`) as an incident / break-glass path. They are
//! deliberately not exposed here: routine operation is mint-and-deliver.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use super::shared::{
    ACTIVATION_TOKEN_ENV, ADMIN_KEY_ENV, ADMIN_KID_ENV, env_value, missing_admin_key,
    missing_admin_kid, missing_backend, resolve_backend,
};
use crate::deploy::auth::AdminBearer;
use crate::deploy::backend::BackendClient;
use crate::deploy::platform::Platform;
use crate::deploy::types::MintTokenInput;

#[derive(Debug, Args, Clone)]
pub struct TokenArgs {
    #[command(subcommand)]
    pub cmd: TokenCmd,
}

#[derive(Debug, Subcommand, Clone)]
pub enum TokenCmd {
    /// Mint an activation token (signs an admin bearer from AOMI_ADMIN_KEY).
    Mint(TokenMintArgs),
}

impl TokenArgs {
    pub async fn run(self) -> Result<()> {
        match self.cmd {
            TokenCmd::Mint(a) => a.run().await,
        }
    }
}

/// What a minted token is allowed to act on. Deliberately expressed as two
/// conflicting flags over a safe default rather than a free-form `--scope`
/// string: the previous shape defaulted to the *widest* authority and let
/// `--scope` and `--app-id` disagree with no defined winner.
#[derive(Debug, Args, Clone)]
pub struct TokenScopeArgs {
    /// Bind the token to one existing application. It can deploy and activate
    /// that app and nothing else.
    #[arg(long = "app-id", value_name = "ID", conflicts_with = "platform_wide")]
    pub app_id: Option<i64>,

    /// Mint a platform-wide token: authority over EVERY app on the platform.
    /// Ops, CI, and bootstrap only — never hand one to an app developer.
    #[arg(long = "platform-wide")]
    pub platform_wide: bool,
}

impl TokenScopeArgs {
    /// Default (neither flag) is an unbound `app` token — the narrowest thing
    /// that still works for a brand-new app, since it binds 1-to-1 to whatever
    /// app its first deploy creates.
    fn to_input(&self) -> MintTokenInput {
        if self.platform_wide {
            MintTokenInput {
                scope: "platform".to_string(),
                app_id: None,
            }
        } else {
            MintTokenInput {
                scope: "app".to_string(),
                app_id: self.app_id,
            }
        }
    }

    fn describe(&self) -> &'static str {
        match (self.platform_wide, self.app_id) {
            (true, _) => "platform-wide — authority over every app on this platform",
            (false, Some(_)) => "app — bound to that one application",
            (false, None) => "app (unbound) — binds to the first app it deploys, 1-to-1",
        }
    }
}

#[derive(Debug, Args, Clone)]
pub struct TokenMintArgs {
    /// Platform tag the token is scoped to.
    #[arg(long, value_name = "NAME")]
    pub platform: Platform,

    #[command(flatten)]
    pub scope: TokenScopeArgs,

    /// Backend base URL (default: `AOMI_BACKEND_URL`).
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// File holding the admin Ed25519 private key PEM. Falls back to
    /// `AOMI_ADMIN_KEY` (PEM text, or a path to a PEM file).
    ///
    /// There is deliberately no flag that takes the key material itself:
    /// process arguments are world-readable via `ps` and land in shell history,
    /// so an inline PEM would leak the signing key to anyone on the box.
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
            .ok_or_else(|| missing_admin_kid("token mint"))?;

        let backend_url =
            resolve_backend(&self.backend).ok_or_else(|| missing_backend("token mint"))?;

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

        let request = self.scope.to_input();
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
            // The plaintext is printed EXACTLY once. Repeating it inside a
            // ready-to-run `deploy --activation-token <token>` line, or an
            // `export …=<token>` hint, would copy a live credential into
            // terminal scrollback, shell history, and CI logs. Name the env
            // var; never fill in its value.
            println!(
                "Minted token id {} for platform `{}`",
                result.id, self.platform
            );
            println!("  scope: {}", self.scope.describe());
            println!("  token: {}", result.token);
            println!();
            println!(
                "Deliver this to the intended developer over a trusted secret channel — \
                 the backend keeps only its hash, so it cannot be shown again."
            );
            println!(
                "They set it as {ACTIVATION_TOKEN_ENV}, or save it once with \
                 `aomi-build connect`, then:"
            );
            println!(
                "  aomi-build project create --platform {} --repo <owner/repo>",
                self.platform
            );
            println!("  # commit .aomi/config.json, push it, then run: aomi-build deploy")
        }
        Ok(())
    }
}

/// Resolve the admin signing key: `--admin-key-file`, else `AOMI_ADMIN_KEY`
/// (PEM text, or a path to a PEM file). This is the privileged, out-of-band
/// signing key — not an activation token.
///
/// Key material is never accepted as a command-line argument. `ps` exposes
/// another user's arguments and the shell records them, so the only ways in are
/// a file the operator controls the permissions of, or the environment.
fn resolve_admin_key(file: &Option<PathBuf>) -> Result<Vec<u8>> {
    if let Some(path) = file {
        return std::fs::read(path)
            .with_context(|| format!("failed to read admin key file {}", path.display()));
    }
    let raw = env_value(ADMIN_KEY_ENV).ok_or_else(|| missing_admin_key("token mint"))?;
    read_admin_key_value(&raw, ADMIN_KEY_ENV)
}

fn read_admin_key_value(raw: &str, source: &str) -> Result<Vec<u8>> {
    if raw.contains("BEGIN") {
        Ok(raw.as_bytes().to_vec())
    } else {
        std::fs::read(raw)
            .with_context(|| format!("{source} is neither a PEM nor a readable file path: {raw}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(app_id: Option<i64>, platform_wide: bool) -> TokenScopeArgs {
        TokenScopeArgs {
            app_id,
            platform_wide,
        }
    }

    /// The safe default. An operator who types the shortest possible command
    /// must get the narrowest useful token, not the widest one — the previous
    /// `--scope` default was `platform`.
    #[test]
    fn no_flags_mints_an_unbound_app_token() {
        let input = scope(None, false).to_input();
        assert_eq!(input.scope, "app");
        assert_eq!(input.app_id, None);
        assert_eq!(
            serde_json::to_value(&input).unwrap(),
            serde_json::json!({ "scope": "app" }),
            "`app_id` must be omitted entirely, not sent as null"
        );
    }

    #[test]
    fn app_id_mints_a_bound_app_token() {
        let input = scope(Some(42), false).to_input();
        assert_eq!(
            serde_json::to_value(&input).unwrap(),
            serde_json::json!({ "scope": "app", "app_id": 42 })
        );
    }

    #[test]
    fn platform_wide_mints_a_platform_token() {
        let input = scope(None, true).to_input();
        assert_eq!(
            serde_json::to_value(&input).unwrap(),
            serde_json::json!({ "scope": "platform" })
        );
    }

    /// The admin signing key must never be passable as an argument: `ps` shows
    /// other users' arguments and the shell records them. Only a file path or
    /// the environment are accepted.
    #[test]
    fn there_is_no_flag_that_takes_admin_key_material() {
        use clap::CommandFactory;

        #[derive(Debug, clap::Parser)]
        struct Harness {
            #[command(flatten)]
            mint: TokenMintArgs,
        }

        let inline_pem = "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----";
        for flag in ["--admin-key", "--admin-key-inline", "--admin-pem"] {
            assert!(
                Harness::command()
                    .try_get_matches_from(["h", "--platform", "community", flag, inline_pem])
                    .is_err(),
                "`{flag}` must not accept key material"
            );
        }

        // The file path form is the supported one.
        assert!(
            Harness::command()
                .try_get_matches_from([
                    "h",
                    "--platform",
                    "community",
                    "--admin-key-file",
                    "/tmp/admin.pem",
                ])
                .is_ok()
        );
    }

    /// `--app-id` and `--platform-wide` name different authorities, so the
    /// parser must reject the combination rather than silently pick one.
    #[test]
    fn app_id_and_platform_wide_conflict_at_parse_time() {
        use clap::{CommandFactory, FromArgMatches};

        #[derive(Debug, clap::Parser)]
        struct Harness {
            #[command(flatten)]
            scope: TokenScopeArgs,
        }

        let err = Harness::command()
            .try_get_matches_from(["harness", "--app-id", "42", "--platform-wide"])
            .expect_err("conflicting scope flags must not parse");
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);

        // Each flag on its own still parses.
        for args in [
            vec!["harness", "--app-id", "42"],
            vec!["harness", "--platform-wide"],
            vec!["harness"],
        ] {
            let matches = Harness::command()
                .try_get_matches_from(&args)
                .unwrap_or_else(|e| panic!("{args:?} should parse: {e}"));
            Harness::from_arg_matches(&matches).expect("harness builds");
        }
    }
}
