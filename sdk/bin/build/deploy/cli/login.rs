//! `login` — authenticate the local CLI as a verified GitHub Builder.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use clap::Args;
use reqwest::Url;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::shared::{
    BACKEND_URL_ENV, BUILD_TOKEN_ENV, BUILD_URL_ENV, env_value, resolve_backend, resolve_build_url,
};
use crate::deploy::build_client::{BuildClient, exchange_cli_code};
use crate::deploy::config::AomiConfig;
use crate::deploy::types::CliStatusResult;

const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub async fn run(args: LoginArgs) -> eyre::Result<()> {
    args.run().await.map_err(crate::git_error)
}

#[derive(Debug, Args, Clone)]
pub struct LoginArgs {
    /// Aomi Build URL. Defaults to `AOMI_BUILD_URL`, saved config, or the known
    /// staging/production URL associated with the backend.
    #[arg(long = "build-url", value_name = "URL")]
    pub build_url: Option<String>,

    /// Backend URL used only to infer the corresponding Build environment.
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// Print the login URL instead of opening it automatically.
    #[arg(long)]
    pub no_browser: bool,
}

impl LoginArgs {
    pub async fn run(self) -> Result<()> {
        let backend_url = resolve_backend(&self.backend);
        let build_url =
            resolve_build_url(&self.build_url, backend_url.as_deref()).ok_or_else(|| {
                anyhow!(
                    "login needs an Aomi Build URL — set --build-url or {BUILD_URL_ENV}; \
                     known {BACKEND_URL_ENV} staging/production URLs are inferred automatically"
                )
            })?;
        let (authenticated, path) =
            browser_login_and_save(&build_url, backend_url, self.no_browser).await?;

        println!("✓ Logged in as @{}", authenticated.status.github_login);
        println!("  saved to {}", path.display());
        Ok(())
    }
}

pub struct AuthenticatedBuild {
    pub client: BuildClient,
    pub status: CliStatusResult,
}

pub async fn ensure_logged_in(build_url: &str) -> Result<AuthenticatedBuild> {
    let mut config = AomiConfig::load();
    let env_token = env_value(BUILD_TOKEN_ENV);
    let saved_token = config.cli_access_token.clone();
    if let Some(token) = env_token.clone().or(saved_token) {
        let client = BuildClient::new(build_url, token)?;
        if let Ok(status) = client.whoami().await
            && status.signed_in
        {
            save_identity(&mut config, build_url, &status, None)?;
            println!("✓ Logged in as @{}\n", status.github_login);
            return Ok(AuthenticatedBuild { client, status });
        }
    }
    if env_token.is_some() {
        bail!(
            "{BUILD_TOKEN_ENV} was rejected by Aomi Build; replace or unset it before logging in"
        );
    }

    println!("You need to log in to Aomi Build.");
    let (authenticated, _) = browser_login_and_save(build_url, None, false).await?;
    println!("✓ Logged in as @{}\n", authenticated.status.github_login);
    Ok(authenticated)
}

struct LoginIdentity {
    access_token: String,
    github_login: String,
    github_user_id: String,
}

async fn browser_login_and_save(
    build_url: &str,
    backend_url: Option<String>,
    no_browser: bool,
) -> Result<(AuthenticatedBuild, std::path::PathBuf)> {
    let identity = browser_login(build_url, no_browser).await?;
    let status = CliStatusResult {
        signed_in: true,
        github_login: identity.github_login,
        github_user_id: identity.github_user_id,
    };
    let client = BuildClient::new(build_url, identity.access_token.clone())?;
    let mut config = AomiConfig::load();
    if let Some(backend_url) = backend_url {
        config.backend_url = Some(backend_url);
    }
    let path = save_identity(&mut config, build_url, &status, Some(identity.access_token))?;
    Ok((AuthenticatedBuild { client, status }, path))
}

fn save_identity(
    config: &mut AomiConfig,
    build_url: &str,
    status: &CliStatusResult,
    access_token: Option<String>,
) -> Result<std::path::PathBuf> {
    config.build_url = Some(build_url.to_string());
    config.github_user_id = Some(status.github_user_id.clone());
    config.github_login = Some(status.github_login.clone());
    if let Some(access_token) = access_token {
        config.cli_access_token = Some(access_token);
    }
    config.save()
}

async fn browser_login(build_url: &str, no_browser: bool) -> Result<LoginIdentity> {
    let listener =
        TcpListener::bind("127.0.0.1:0").context("failed to start local login callback")?;
    listener
        .set_nonblocking(true)
        .context("failed to configure local login callback")?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let state = random_token();
    let code_verifier = random_token();
    let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));

    let mut login_url = Url::parse(&format!(
        "{}/api/bff/cli/login",
        build_url.trim().trim_end_matches('/')
    ))
    .context("invalid Aomi Build URL")?;
    login_url
        .query_pairs_mut()
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("state", &state)
        .append_pair("code_challenge", &code_challenge);

    println!("Open this URL to log in:");
    println!("  {login_url}");
    if !no_browser {
        match open::that(login_url.as_str()) {
            Ok(()) => println!("  (opened in your browser)"),
            Err(error) => eprintln!("  (couldn't open browser automatically: {error})"),
        }
    }

    let callback = wait_for_callback(&listener, &state, build_url)?;
    let result = exchange_cli_code(build_url, callback, code_verifier).await?;
    if !result.token_type.eq_ignore_ascii_case("bearer") || result.expires_in <= 0 {
        bail!("Aomi Build returned an invalid CLI session");
    }
    Ok(LoginIdentity {
        access_token: result.access_token,
        github_login: result.github_login,
        github_user_id: result.github_user_id,
    })
}

fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn wait_for_callback(
    listener: &TcpListener,
    expected_state: &str,
    build_url: &str,
) -> Result<String> {
    let started = Instant::now();
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                let mut bytes = [0_u8; 16 * 1024];
                let read = stream.read(&mut bytes)?;
                let request = String::from_utf8_lossy(&bytes[..read]);
                let target = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .ok_or_else(|| anyhow!("invalid local login callback"))?;
                let callback = Url::parse(&format!("http://127.0.0.1{target}"))
                    .context("invalid local login callback URL")?;
                let params = callback
                    .query_pairs()
                    .collect::<std::collections::HashMap<_, _>>();
                let state = params.get("state").map(|value| value.as_ref());
                let code = params.get("code").map(|value| value.to_string());
                let error = params.get("error").map(|value| value.to_string());
                let success = state == Some(expected_state) && code.is_some();
                write_callback_response(&mut stream, build_url, success)?;

                if state != Some(expected_state) {
                    bail!("Aomi Build login returned an invalid state");
                }
                if let Some(error) = error {
                    bail!("Aomi Build login failed: {error}");
                }
                return code.ok_or_else(|| anyhow!("Aomi Build login returned no code"));
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if started.elapsed() >= LOGIN_TIMEOUT {
                    bail!("timed out waiting for browser login");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(error).context("local login callback failed"),
        }
    }
}

fn completion_url(build_url: &str, success: bool) -> Result<Url> {
    let mut url = Url::parse(build_url.trim()).context("invalid Aomi Build URL")?;
    url.set_path("/cli/auth-complete");
    url.set_query(None);
    url.set_fragment(None);
    url.query_pairs_mut()
        .append_pair("status", if success { "complete" } else { "failed" });
    Ok(url)
}

fn write_callback_response(
    stream: &mut std::net::TcpStream,
    build_url: &str,
    success: bool,
) -> Result<()> {
    let location = completion_url(build_url, success)?;
    write!(
        stream,
        "HTTP/1.1 303 See Other\r\nLocation: {location}\r\nCache-Control: no-store\r\n\
         Content-Length: 0\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::completion_url;

    #[test]
    fn completion_url_exposes_only_the_result() {
        let complete = completion_url(
            "https://build.example.test/old?secret=hidden#fragment",
            true,
        )
        .unwrap();
        assert_eq!(
            complete.as_str(),
            "https://build.example.test/cli/auth-complete?status=complete"
        );

        let failed = completion_url("https://build.example.test", false).unwrap();
        assert_eq!(
            failed.as_str(),
            "https://build.example.test/cli/auth-complete?status=failed"
        );
    }
}
