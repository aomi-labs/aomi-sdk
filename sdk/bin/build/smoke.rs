//! Read-only post-activation checks through the same public Agent API as the widget.
use crate::deploy::types::ActivatedApp;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub(crate) fn validate_origin(origin: &str) -> Result<reqwest::Url> {
    let origin = reqwest::Url::parse(origin).context("invalid smoke Portal origin")?;
    if origin.scheme() != "https"
        && !(origin.scheme() == "http"
            && matches!(origin.host_str(), Some("localhost" | "127.0.0.1")))
    {
        bail!("smoke Portal origin must use HTTPS (HTTP is supported for local development)");
    }
    if !origin.username().is_empty()
        || origin.password().is_some()
        || origin.query().is_some()
        || origin.fragment().is_some()
        || origin.path() != "/"
    {
        bail!("smoke Portal URL must be an origin without credentials, path, query or fragment");
    }
    Ok(origin)
}

pub async fn run(origin: &str, apps: &[ActivatedApp]) -> Result<()> {
    let origin = validate_origin(origin)?;
    let http = crate::deploy::backend::http_client();
    let auth: Value = http
        .post(origin.join("/api/auth/sign-in/anonymous")?)
        .json(&json!({}))
        .send()
        .await?
        .error_for_status()
        .context("smoke guest sign-in failed")?
        .json()
        .await?;
    let token = auth["token"]
        .as_str()
        .context("smoke guest sign-in omitted its bearer")?;
    for app in apps {
        let id = app
            .application_id
            .context("activation omitted applicationId required for smoke chat")?;
        let request = json!({"sessionId": uuid::Uuid::new_v4().to_string(), "message": "Describe this app in one short sentence. Do not execute any tools or wallet actions.",
            "applicationId": id, "mode": "direct" });
        let mut page: Value = http
            .post(origin.join("/v1/agent/chat")?)
            .bearer_auth(token)
            .header("Idempotency-Key", uuid::Uuid::new_v4().to_string())
            .json(&request)
            .send()
            .await?
            .error_for_status()
            .context("smoke chat start failed")?
            .json()
            .await?;
        let session = page["session_id"]
            .as_str()
            .or_else(|| page["sessionId"].as_str())
            .context("smoke chat omitted session id")?
            .to_string();
        let started = Instant::now();
        loop {
            let state = page["events"].as_array().and_then(|events| {
                events
                    .iter()
                    .rev()
                    .find(|event| event["type"] == "turn_state_changed")
            });
            if let Some(state) = state {
                match state["state"].as_str() {
                    Some("complete") => {
                        eprintln!(
                            "Smoke chat passed: {} (application {id}, session {session})",
                            app.name
                        );
                        break;
                    }
                    Some("failed" | "interrupted" | "awaiting_action") => bail!(
                        "smoke chat for {} stopped with {} (session {session}, code {})",
                        app.name,
                        state["state"].as_str().unwrap_or("unknown"),
                        state["error_code"].as_str().unwrap_or("unknown")
                    ),
                    _ => {}
                }
            }
            if started.elapsed() > Duration::from_secs(120) {
                bail!("smoke chat timed out for {} (session {session})", app.name);
            }
            let mut url = origin.join(&format!("/v1/agent/chat/{session}"))?;
            url.query_pairs_mut().append_pair("wait", "10000");
            if let Some(cursor) = page["cursor"].as_str() {
                url.query_pairs_mut().append_pair("cursor", cursor);
            }
            page = http
                .get(url)
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()
                .context("smoke chat poll failed")?
                .json()
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smoke_requires_a_portal_origin() {
        assert!(validate_origin("https://chat.aomi.dev").is_ok());
        assert!(validate_origin("http://localhost:3000").is_ok());
        for invalid in [
            "http://chat.aomi.dev",
            "file://localhost",
            "https://secret@chat.aomi.dev",
            "https://chat.aomi.dev/path",
        ] {
            assert!(validate_origin(invalid).is_err(), "{invalid}");
        }
    }
}
