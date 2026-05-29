//! Discord activation-request delivery.
//!
//! Contributors don't hold the activation token — platform ops do (ADR 0009).
//! So after a deploy, the "next action" is to *ask* ops to activate. This
//! module can post that ask to the Aomi apps Discord via an incoming webhook,
//! tagging ops and carrying the repo / app / release tag so ops can act
//! without a round-trip.
//!
//! ## Why a webhook, not a clickable link
//! A `discord.gg/...` invite only opens/joins the server — it cannot send a
//! message or ping anyone. Auto-sending requires an authenticated call, so we
//! POST to a Discord **incoming webhook** (`POST <webhook>` with a JSON body).
//!
//! ## Delivery configuration
//! Webhook URLs are credentials, so the binary does not hardcode one. Set
//! `AOMI_DISCORD_WEBHOOK_URL` when posting is enabled. An optional
//! `AOMI_DISCORD_ADMIN_MENTION` value (`<@&ROLE_ID>` or `<@USER_ID>`) can be
//! used to ping ops; otherwise the message posts without a mention.

use anyhow::{Result, anyhow, bail};
use serde_json::json;

/// Public invite to the Aomi apps Discord. Safe to print/commit — an invite
/// only lets someone *join*; it can't post or read.
pub const DISCORD_INVITE: &str = "https://discord.gg/VF5Zq8ddu";

/// Env var containing the Discord incoming-webhook URL for activation requests.
pub const DISCORD_WEBHOOK_ENV: &str = "AOMI_DISCORD_WEBHOOK_URL";

/// Optional env var containing an ops role/user mention (`<@&ID>` or `<@ID>`).
pub const DISCORD_ADMIN_ENV: &str = "AOMI_DISCORD_ADMIN_MENTION";

/// Posting configuration for the activation-request Discord channel.
pub struct DiscordConfig {
    webhook_url: String,
    admin_mention: Option<String>,
}

impl DiscordConfig {
    pub fn from_env() -> Result<Self> {
        let webhook_url = std::env::var(DISCORD_WEBHOOK_ENV)
            .map(|v| v.trim().to_string())
            .unwrap_or_default();
        if webhook_url.is_empty() {
            bail!(
                "Discord webhook is not configured; set {DISCORD_WEBHOOK_ENV}, or run \
                 `aomi-git activate --request --dry-run` and post the message manually"
            );
        }
        let admin_mention = std::env::var(DISCORD_ADMIN_ENV)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        Ok(Self {
            webhook_url,
            admin_mention,
        })
    }

    /// POST the activation request to the configured Discord webhook. Returns
    /// `Ok` on any 2xx (Discord answers 204 No Content on success).
    pub async fn post(&self, request: &ActivationRequest) -> Result<()> {
        let response = reqwest::Client::new()
            .post(&self.webhook_url)
            .json(&request.webhook_body(self.admin_mention.as_deref()))
            .send()
            .await
            .map_err(|e| anyhow!("failed to POST Discord webhook: {e}"))?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Discord webhook returned {status}: {}", text.trim()));
        }
        Ok(())
    }
}

/// The activation ask, independent of how it's delivered.
pub struct ActivationRequest {
    pub app: String,
    pub repo: String,
    pub release_tag: String,
    pub server_tags: Vec<String>,
}

impl ActivationRequest {
    /// Render the human message body, optionally prefixed with an ops mention.
    pub fn message(&self, admin_mention: Option<&str>) -> String {
        let tags = if self.server_tags.is_empty() {
            "<none>".to_string()
        } else {
            self.server_tags.join(", ")
        };
        let mention = admin_mention
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| format!("{s} "))
            .unwrap_or_default();
        format!(
            "{mention}**Activation requested**\n\
             - app: `{}`\n\
             - repo: `{}`\n\
             - release: `{}`\n\
             - target tags: `{}`\n\
             Please activate when you have a chance.",
            self.app, self.repo, self.release_tag, tags
        )
    }

    /// The JSON body sent to the webhook. `allowed_mentions` is scoped to
    /// users/roles only, never `@everyone`.
    fn webhook_body(&self, admin_mention: Option<&str>) -> serde_json::Value {
        json!({
            "content": self.message(admin_mention),
            "allowed_mentions": { "parse": ["users", "roles"] },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ActivationRequest {
        ActivationRequest {
            app: "my-bot".to_string(),
            repo: "aomi-labs/community-apps".to_string(),
            release_tag: "apps-my-bot-abc1234".to_string(),
            server_tags: vec!["staging".to_string()],
        }
    }

    #[test]
    fn message_includes_optional_admin_repo_app_release_and_tags() {
        let msg = sample().message(Some("<@&123>"));
        assert!(msg.starts_with("<@&123>"), "{msg}");
        assert!(msg.contains("my-bot"), "{msg}");
        assert!(msg.contains("aomi-labs/community-apps"), "{msg}");
        assert!(msg.contains("apps-my-bot-abc1234"), "{msg}");
        assert!(msg.contains("staging"), "{msg}");
    }

    #[test]
    fn webhook_body_scopes_mentions_and_never_everyone() {
        let body = sample().webhook_body(Some("<@&123>"));
        let parse: Vec<&str> = body["allowed_mentions"]["parse"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(parse, vec!["users", "roles"]);
        assert!(!parse.contains(&"everyone"), "{body}");
    }
}
