//! Discord activation-request delivery.
//!
//! Contributors don't hold the activation token - platform ops do (ADR 0009).
//! So after a deploy, the "next action" is to *ask* ops to activate. This
//! module can post that ask to the Aomi apps Discord via an incoming webhook,
//! tagging ops and carrying the repo / app / release tag so ops can act
//! without a round-trip.
//!
//! ## Why a webhook, not a clickable link
//! A `discord.gg/...` invite only opens/joins the server - it cannot send a
//! message or ping anyone. Auto-sending requires an authenticated call, so we
//! POST to a Discord **incoming webhook** (`POST <webhook>` with a JSON body).
//!
//! ## Delivery configuration
//! The Discord target is intentionally code-owned: contributors should not
//! configure where activation requests go. Update the constants below when the
//! activation channel or ops mention changes.

use anyhow::{Result, anyhow, bail};
use serde_json::json;

/// Public invite to the Aomi apps Discord. Safe to print/commit - an invite
/// only lets someone *join*; it can't post or read.
pub const DISCORD_INVITE: &str = "https://discord.gg/VF5Zq8ddu";

/// Incoming webhook for the activation-request channel.
const DISCORD_WEBHOOK: &str = "https://discord.com/api/webhooks/1510784125009657876/DVnF_g6TgBsnrzRBBu5hKfvsRvA6U7fYFfJnDTQMWT5pkn6uxGJ1io4LyN9E7CrPDfWp";

/// Ops role/user mention (`<@&ID>` or `<@ID>`).
const DISCORD_ADMIN: &str = "<@&1510790865520693348>";

/// The activation ask, independent of how it's delivered.
pub struct ActivationRequest {
    pub app: String,
    pub repo: String,
    pub release_tag: String,
    pub server_tags: Vec<String>,
}

impl ActivationRequest {
    /// Render the human message body.
    pub fn message(&self) -> String {
        let tags = if self.server_tags.is_empty() {
            "<none>".to_string()
        } else {
            self.server_tags.join(", ")
        };
        format!(
            "{DISCORD_ADMIN} **Activation requested**\n\
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
    fn webhook_body(&self) -> serde_json::Value {
        json!({
            "content": self.message(),
            "allowed_mentions": { "parse": ["users", "roles"] },
        })
    }

    /// POST this activation request to the code-owned Discord webhook. Returns
    /// `Ok` on any 2xx (Discord answers 204 No Content on success).
    pub async fn post(&self) -> Result<()> {
        if DISCORD_WEBHOOK.contains("REPLACE_ME") || DISCORD_ADMIN.contains("REPLACE_ME") {
            bail!(
                "Discord activation target is not configured in sdk/bin/git/discord.rs; \
                 run `aomi-git activate --request --dry-run` and post the message manually"
            );
        }
        let response = reqwest::Client::new()
            .post(DISCORD_WEBHOOK)
            .json(&self.webhook_body())
            .send()
            .await
            .map_err(|e| anyhow!("failed to POST Discord webhook: {e}"))?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Discord webhook returned {status}: {}",
                text.trim()
            ));
        }
        Ok(())
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
    fn message_includes_admin_repo_app_release_and_tags() {
        let msg = sample().message();
        assert!(msg.starts_with(DISCORD_ADMIN), "{msg}");
        assert!(msg.contains("my-bot"), "{msg}");
        assert!(msg.contains("aomi-labs/community-apps"), "{msg}");
        assert!(msg.contains("apps-my-bot-abc1234"), "{msg}");
        assert!(msg.contains("staging"), "{msg}");
    }

    #[test]
    fn webhook_body_scopes_mentions_and_never_everyone() {
        let body = sample().webhook_body();
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
