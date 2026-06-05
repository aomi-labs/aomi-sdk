//! Discord activation-request delivery.
//!
//! A new contributor can't use the direct-Git transport until ops invites their
//! GitHub account to the platform repo, and can't self-activate until ops issues
//! an activation bearer. So the first step is to *ask* ops for onboarding. This
//! module posts that ask to the Aomi apps Discord
//! via an incoming webhook, tagging ops and carrying the requester's GitHub
//! account + email + app so ops can act without a round-trip.
//!
//! The activation bearer itself is NEVER part of this request - ops issues it and
//! delivers it to the requester over a secure channel out-of-band.
//!
//! ## Why a webhook, not a clickable link
//! A `discord.gg/...` invite only opens/joins the server - it cannot send a
//! message or ping anyone. Auto-sending requires an authenticated call, so we
//! POST to a Discord **incoming webhook** (`POST <webhook>` with a JSON body).
//!
//! ## Delivery configuration
//! The Discord target is intentionally code-owned: contributors should not
//! configure where activation requests go. Update the constants below when the
//! request channel or ops mention changes.

use anyhow::{Result, anyhow, bail};
use chrono::{SecondsFormat, Utc};
use serde_json::json;

/// Public invite to the Aomi apps Discord. Safe to print/commit - an invite
/// only lets someone *join*; it can't post or read.
pub const DISCORD_INVITE: &str = "https://discord.gg/VF5Zq8ddu";

/// Incoming webhook for the activation-request channel.
const DISCORD_WEBHOOK: &str = "https://discord.com/api/webhooks/1510784125009657876/DVnF_g6TgBsnrzRBBu5hKfvsRvA6U7fYFfJnDTQMWT5pkn6uxGJ1io4LyN9E7CrPDfWp";

/// Ops role/user mention (`<@&ID>` or `<@ID>`).
const DISCORD_ADMIN: &str = "<@&1510790865520693348>";

/// Embed accent color (blurple). Cosmetic only.
const EMBED_COLOR: u32 = 5_793_266;

/// Provenance stamp on the structured payload.
const SOURCE: &str = concat!("aomi-git/", env!("CARGO_PKG_VERSION"));

/// The onboarding/activation ask, independent of how it's delivered.
///
/// Carries the requester identity ops (and the downstream automation consuming
/// the webhook) need to act: the GitHub account to invite as a platform
/// collaborator and the email to deliver activation details to. The bearer
/// itself is NEVER part of this request.
pub struct ActivationRequest {
    pub email: String,
    pub git_account: String,
    pub app: String,
    pub platform: String,
    pub repo: String,
}

impl ActivationRequest {
    /// The canonical machine-readable payload. This is the single source of
    /// truth the downstream consumer parses; the human embed fields are
    /// rendered from the same values so the two can't drift.
    ///
    /// Note: this is a *pre-deploy* request, so it deliberately carries no
    /// `app_release_tag` / `server_tags` (no release exists yet), and never a
    /// token.
    pub fn payload(&self) -> serde_json::Value {
        json!({
            "kind": "activation_request",
            "email": self.email,
            "github_account": self.git_account,
            "app": self.app,
            "platform": self.platform,
            "repo": self.repo,
            "requested_at": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            "source": SOURCE,
        })
    }

    /// The JSON body sent to the webhook: a human-readable embed (fields) for
    /// ops, plus the canonical payload inlined as a fenced ```json block in the
    /// embed description for the automation to parse. `allowed_mentions` is
    /// scoped to users/roles only, never `@everyone`.
    pub fn webhook_body(&self) -> serde_json::Value {
        let payload = self.payload();
        let compact = serde_json::to_string(&payload).unwrap_or_default();
        json!({
            "content": DISCORD_ADMIN,
            "allowed_mentions": { "parse": ["users", "roles"] },
            "embeds": [{
                "title": "Activation request",
                "color": EMBED_COLOR,
                "fields": [
                    { "name": "Requester", "value": self.email, "inline": true },
                    { "name": "GitHub", "value": self.git_account, "inline": true },
                    { "name": "App", "value": self.app, "inline": true },
                    { "name": "Platform", "value": self.platform, "inline": true },
                    { "name": "Repo", "value": self.repo, "inline": false },
                ],
                "description": format!("```json\n{compact}\n```"),
            }],
        })
    }

    /// POST this activation request to the code-owned Discord webhook. Returns
    /// `Ok` on any 2xx (Discord answers 204 No Content on success).
    pub async fn post(&self) -> Result<()> {
        if DISCORD_WEBHOOK.contains("REPLACE_ME") || DISCORD_ADMIN.contains("REPLACE_ME") {
            bail!(
                "Discord request target is not configured in sdk/bin/git/discord.rs; \
                 run `aomi-git request --dry-run` and post the message manually"
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
            email: "alice@gmail.com".to_string(),
            git_account: "alice-git-acc".to_string(),
            app: "my-bot".to_string(),
            platform: "community".to_string(),
            repo: "aomi-labs/community-apps".to_string(),
        }
    }

    #[test]
    fn payload_carries_the_canonical_fields() {
        let p = sample().payload();
        assert_eq!(p["kind"], "activation_request");
        assert_eq!(p["email"], "alice@gmail.com");
        assert_eq!(p["github_account"], "alice-git-acc");
        assert_eq!(p["app"], "my-bot");
        assert_eq!(p["platform"], "community");
        assert_eq!(p["repo"], "aomi-labs/community-apps");
        assert!(p["requested_at"].as_str().unwrap().contains('T'), "{p}");
        assert!(
            p["source"].as_str().unwrap().starts_with("aomi-git/"),
            "{p}"
        );
    }

    #[test]
    fn payload_never_carries_a_token_or_release_tag() {
        // A pre-deploy request: no token, no release artifacts.
        let p = sample().payload();
        assert!(p.get("token").is_none(), "{p}");
        assert!(p.get("activation_token").is_none(), "{p}");
        assert!(p.get("app_release_tag").is_none(), "{p}");
        assert!(p.get("server_tags").is_none(), "{p}");
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

    #[test]
    fn webhook_body_embeds_human_fields_and_a_fenced_json_payload() {
        let body = sample().webhook_body();
        assert_eq!(body["content"], DISCORD_ADMIN);
        let embed = &body["embeds"][0];
        assert_eq!(embed["title"], "Activation request");

        // Human-readable fields carry the same values as the payload.
        let field_values: Vec<&str> = embed["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["value"].as_str().unwrap())
            .collect();
        assert!(field_values.contains(&"alice@gmail.com"), "{embed}");
        assert!(field_values.contains(&"alice-git-acc"), "{embed}");
        assert!(
            field_values.contains(&"aomi-labs/community-apps"),
            "{embed}"
        );

        // The description holds a fenced json block the consumer can parse back.
        let desc = embed["description"].as_str().unwrap();
        assert!(
            desc.starts_with("```json\n") && desc.ends_with("\n```"),
            "{desc}"
        );
        let inner = desc
            .trim_start_matches("```json\n")
            .trim_end_matches("\n```");
        let parsed: serde_json::Value =
            serde_json::from_str(inner).expect("description json parses");
        assert_eq!(parsed["kind"], "activation_request");
        assert_eq!(parsed["github_account"], "alice-git-acc");
    }
}
