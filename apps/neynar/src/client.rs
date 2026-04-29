use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct NeynarApp;

#[allow(unused_imports)]
pub(crate) use crate::tool::*;

// ============================================================================
// Client
// ============================================================================

pub(crate) const API_BASE: &str = "https://api.neynar.com/v2";

#[derive(Clone)]
pub(crate) struct NeynarClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_key: String,
}

impl NeynarClient {
    pub(crate) fn new(api_key: Option<&str>) -> Result<Self, String> {
        let api_key = resolve_secret_value(
            api_key,
            "NEYNAR_API_KEY",
            "[neynar] missing api_key argument and NEYNAR_API_KEY environment variable",
        )?;
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[neynar] failed to build HTTP client: {e}"))?;
        Ok(Self { http, api_key })
    }

    pub(crate) fn get<Q: Serialize>(
        &self,
        path: &str,
        query: &Q,
        op: &str,
    ) -> Result<Value, String> {
        let url = format!("{API_BASE}{path}");
        let resp = self
            .http
            .get(&url)
            .header("x-api-key", &self.api_key)
            .query(query)
            .send()
            .map_err(|e| format!("[neynar] {op} failed: {e}"))?;

        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[neynar] {op} failed: {status} {text}"));
        }

        serde_json::from_str::<Value>(&text)
            .map_err(|e| format!("[neynar] {op} decode failed: {e}"))
    }

    pub(crate) fn post_json<B: Serialize>(
        &self,
        path: &str,
        body: &B,
        op: &str,
    ) -> Result<Value, String> {
        let url = format!("{API_BASE}{path}");
        let resp = self
            .http
            .post(&url)
            .header("x-api-key", &self.api_key)
            .json(body)
            .send()
            .map_err(|e| format!("[neynar] {op} failed: {e}"))?;

        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[neynar] {op} failed: {status} {text}"));
        }

        serde_json::from_str::<Value>(&text)
            .map_err(|e| format!("[neynar] {op} decode failed: {e}"))
    }
}

// ============================================================================
// Tool 1: GetUserByUsername
// ============================================================================

pub(crate) struct GetUserByUsername;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetUserByUsernameArgs {
    /// Optional Neynar API key. Falls back to NEYNAR_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Farcaster username to look up
    pub(crate) username: String,
}

// ============================================================================
// Tool 2: SearchUsers
// ============================================================================

pub(crate) struct SearchUsers;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchUsersArgs {
    /// Optional Neynar API key. Falls back to NEYNAR_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Search query string to find users
    pub(crate) q: String,
}

// ============================================================================
// Tool 3: GetFeed
// ============================================================================

pub(crate) struct GetFeed;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetFeedArgs {
    /// Optional Neynar API key. Falls back to NEYNAR_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Feed type, e.g. 'filter' or 'following'
    pub(crate) feed_type: String,
    /// Farcaster ID to filter the feed by (optional for some feed types)
    pub(crate) fid: Option<u64>,
    /// Maximum number of results to return (default 25, max 100)
    pub(crate) limit: Option<u32>,
}

// ============================================================================
// Tool 4: GetCast
// ============================================================================

pub(crate) struct GetCast;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCastArgs {
    /// Optional Neynar API key. Falls back to NEYNAR_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Cast identifier: a cast hash (0x...) or a Warpcast URL
    pub(crate) identifier: String,
    /// Type of the identifier: 'hash' or 'url'
    #[serde(rename = "type")]
    pub(crate) id_type: String,
}

// ============================================================================
// Tool 5: SearchCasts
// ============================================================================

pub(crate) struct SearchCasts;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchCastsArgs {
    /// Optional Neynar API key. Falls back to NEYNAR_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Search query string to find casts
    pub(crate) q: String,
    /// Maximum number of results to return (default 25, max 100)
    pub(crate) limit: Option<u32>,
}

// ============================================================================
// Tool 6: PublishCast
// ============================================================================

pub(crate) struct PublishCast;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PublishCastArgs {
    /// Optional Neynar API key. Falls back to NEYNAR_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// UUID of the signer authorized to publish on behalf of the user
    pub(crate) signer_uuid: String,
    /// Text content of the cast (up to 1024 bytes)
    pub(crate) text: String,
    /// Optional list of embed URLs to attach to the cast
    pub(crate) embeds: Option<Vec<EmbedArg>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct EmbedArg {
    /// URL to embed in the cast
    pub(crate) url: String,
}

// ============================================================================
// Tool 7: GetChannel
// ============================================================================

pub(crate) struct GetChannel;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetChannelArgs {
    /// Optional Neynar API key. Falls back to NEYNAR_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Channel ID (e.g. 'ethereum', 'farcaster', 'memes')
    pub(crate) id: String,
}

// ============================================================================
// Tool 8: GetTrendingFeed
// ============================================================================

pub(crate) struct GetTrendingFeed;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTrendingFeedArgs {
    /// Optional Neynar API key. Falls back to NEYNAR_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Maximum number of trending casts to return (default 10, max 100)
    pub(crate) limit: Option<u32>,
    /// Time window for trending calculation, e.g. '6h', '12h', '24h', '7d'
    pub(crate) time_window: Option<String>,
}

// ============================================================================
// Integration Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UserByUsernameQuery;

    #[derive(Serialize)]
    struct BulkUsersQuery<'a> {
        fids: &'a str,
    }

    /// Helper: build a client or skip the test when NEYNAR_API_KEY is absent.
    fn client_or_skip() -> Option<NeynarClient> {
        std::env::var("NEYNAR_API_KEY")
            .ok()
            .map(|_| NeynarClient::new(None).expect("failed to build NeynarClient"))
    }

    /// Story: "Post a thread replying to a key influencer"
    #[test]
    fn post_thread_workflow() {
        let Some(client) = client_or_skip() else {
            return;
        };

        let user_resp = client
            .get(
                "/farcaster/user/by_username",
                &UserByUsernameQuery {
                    username: "vitalik.eth",
                },
                "get_user_by_username",
            )
            .expect("get_user_by_username should succeed");
        let user = &user_resp["user"];
        assert!(user.get("fid").is_some(), "user should have an fid");

        let bulk_resp = client
            .get(
                "/farcaster/user/bulk",
                &BulkUsersQuery { fids: "3,5650,2" },
                "bulk_users",
            )
            .expect("bulk_users should succeed");
        let users = bulk_resp["users"]
            .as_array()
            .expect("bulk response should contain users array");
        assert!(
            users.len() >= 2,
            "should get at least 2 users in bulk response"
        );

        let _dwr_resp = client
            .get(
                "/farcaster/user/by_username",
                &UserByUsernameQuery { username: "dwr" },
                "get_user_by_username",
            )
            .expect("get_user_by_username for dwr should succeed");
    }

    /// Story: "Research Farcaster users to find collaboration targets and post"
    #[test]
    fn research_user_and_channel_workflow() {
        let Some(client) = client_or_skip() else {
            return;
        };

        let user_resp = client
            .get(
                "/farcaster/user/by_username",
                &UserByUsernameQuery { username: "dwr" },
                "get_user_by_username",
            )
            .expect("get_user_by_username should succeed");
        let user = &user_resp["user"];
        let fid = user["fid"].as_u64().expect("fid should be a number");
        assert!(fid > 0, "fid should be a positive integer");

        let user2_resp = client
            .get(
                "/farcaster/user/by_username",
                &UserByUsernameQuery {
                    username: "vitalik.eth",
                },
                "get_user_by_username",
            )
            .expect("get_user_by_username for vitalik.eth should succeed");
        let user2_fid = user2_resp["user"]["fid"]
            .as_u64()
            .expect("fid should be a number");

        let fids_param = format!("{fid},{user2_fid},99");
        let bulk_resp = client
            .get(
                "/farcaster/user/bulk",
                &BulkUsersQuery {
                    fids: fids_param.as_str(),
                },
                "bulk_users",
            )
            .expect("bulk_users should succeed");
        let bulk_users = bulk_resp["users"]
            .as_array()
            .expect("bulk should return users array");
        assert!(bulk_users.len() >= 2, "should get at least 2 users");
    }
}
