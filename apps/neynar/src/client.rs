use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
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
    pub(crate) fn new() -> Result<Self, String> {
        let api_key = std::env::var("NEYNAR_API_KEY")
            .map_err(|_| "[neynar] NEYNAR_API_KEY environment variable not set".to_string())?;
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[neynar] failed to build HTTP client: {e}"))?;
        Ok(Self { http, api_key })
    }

    pub(crate) fn get(
        &self,
        path: &str,
        query: &[(&str, &str)],
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

    pub(crate) fn post_json(&self, path: &str, body: &Value, op: &str) -> Result<Value, String> {
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
    /// Farcaster username to look up
    pub(crate) username: String,
}

// ============================================================================
// Tool 2: SearchUsers
// ============================================================================

pub(crate) struct SearchUsers;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchUsersArgs {
    /// Search query string to find users
    pub(crate) q: String,
}

// ============================================================================
// Tool 3: GetFeed
// ============================================================================

pub(crate) struct GetFeed;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetFeedArgs {
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
    /// Channel ID (e.g. 'ethereum', 'farcaster', 'memes')
    pub(crate) id: String,
}

// ============================================================================
// Tool 8: GetTrendingFeed
// ============================================================================

pub(crate) struct GetTrendingFeed;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTrendingFeedArgs {
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

    /// Helper: build a client or skip the test when NEYNAR_API_KEY is absent.
    fn client_or_skip() -> Option<NeynarClient> {
        match std::env::var("NEYNAR_API_KEY") {
            Ok(_) => Some(NeynarClient::new().expect("failed to build NeynarClient")),
            Err(_) => {
                println!("NEYNAR_API_KEY not set — skipping test");
                None
            }
        }
    }

    /// Story: "Post a thread replying to a key influencer"
    /// Look up target user → bulk-fetch their profile + related users → gather
    /// context to compose a reply → skip publish (needs signer_uuid).
    #[test]
    fn post_thread_workflow() {
        let client = match client_or_skip() {
            Some(c) => c,
            None => return,
        };

        // Step 1: Look up the influencer we want to reply to
        println!("post_thread_workflow step 1: looking up user 'vitalik.eth' ...");
        let user_resp = client
            .get(
                "/farcaster/user/by_username",
                &[("username", "vitalik.eth")],
                "get_user_by_username",
            )
            .expect("get_user_by_username should succeed");

        let user = &user_resp["user"];
        assert!(user.get("fid").is_some(), "user should have an fid");
        let vitalik_fid = user["fid"].as_u64().expect("fid should be a number");
        let display_name = user["display_name"].as_str().unwrap_or("?");
        let follower_count = &user["follower_count"];
        let pfp = user["pfp_url"].as_str().unwrap_or("none");
        let bio: String = user["profile"]["bio"]["text"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(100)
            .collect();
        println!(
            "  -> fid={vitalik_fid}, display_name='{display_name}', followers={follower_count}, pfp={pfp}"
        );
        println!("  -> bio: \"{bio}\"");

        // Step 2: Bulk-fetch several influencers to find the best thread participants
        println!("post_thread_workflow step 2: bulk-fetching users fid=3,5650,2 ...");
        let bulk_resp = client
            .get(
                "/farcaster/user/bulk",
                &[("fids", "3,5650,2")],
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

        println!("  -> received {} user(s) in bulk:", users.len());
        for (i, u) in users.iter().enumerate() {
            let ufid = &u["fid"];
            let uname = u["username"].as_str().unwrap_or("?");
            let dname = u["display_name"].as_str().unwrap_or("?");
            let followers = &u["follower_count"];
            let verified = u["verified_addresses"]["eth_addresses"]
                .as_array()
                .map_or(0, |a| a.len());
            println!(
                "  user[{i}]: fid={ufid}, username='{uname}', display_name='{dname}', followers={followers}, eth_addrs={verified}"
            );
        }

        // Step 3: Look up a second user to mention in the thread
        println!("post_thread_workflow step 3: looking up user 'dwr' (Farcaster co-founder) ...");
        let dwr_resp = client
            .get(
                "/farcaster/user/by_username",
                &[("username", "dwr")],
                "get_user_by_username",
            )
            .expect("get_user_by_username for dwr should succeed");

        let dwr = &dwr_resp["user"];
        let dwr_fid = dwr["fid"].as_u64().expect("dwr fid");
        let dwr_name = dwr["display_name"].as_str().unwrap_or("?");
        let dwr_followers = &dwr["follower_count"];
        println!("  -> fid={dwr_fid}, display_name='{dwr_name}', followers={dwr_followers}");

        // Step 4: Skip publish_cast — needs a real signer_uuid
        println!("post_thread_workflow step 4: skipping publish_cast (requires signer_uuid)");
        println!(
            "post_thread_workflow (summary): target=vitalik.eth (fid={}), mention=dwr (fid={}), {} bulk users fetched — ready to compose thread",
            vitalik_fid,
            dwr_fid,
            users.len()
        );
    }

    /// Story: "Research Farcaster users to find collaboration targets and post"
    /// Look up users by username → bulk-fetch multiple profiles → compare
    /// follower counts and verified addresses → skip publish.
    #[test]
    fn research_user_and_channel_workflow() {
        let client = match client_or_skip() {
            Some(c) => c,
            None => return,
        };

        // Step 1: Look up a well-known user by username
        println!("research_user_and_channel_workflow step 1: looking up user 'dwr' ...");
        let user_resp = client
            .get(
                "/farcaster/user/by_username",
                &[("username", "dwr")],
                "get_user_by_username",
            )
            .expect("get_user_by_username should succeed");

        let user = &user_resp["user"];
        assert!(user.get("fid").is_some(), "user should have an fid");
        assert!(
            user.get("username").is_some(),
            "user should have a username"
        );
        assert!(
            user.get("display_name").is_some(),
            "user should have a display_name"
        );

        let fid = user["fid"].as_u64().expect("fid should be a number");
        assert!(fid > 0, "fid should be a positive integer");

        let display_name = user["display_name"].as_str().unwrap_or("?");
        let follower_count = &user["follower_count"];
        let following_count = &user["following_count"];
        println!(
            "  -> user profile: fid={fid}, username='dwr', display_name='{display_name}', followers={follower_count}, following={following_count}"
        );

        // Step 2: Look up another well-known user
        println!("research_user_and_channel_workflow step 2: looking up user 'vitalik.eth' ...");
        let user2_resp = client
            .get(
                "/farcaster/user/by_username",
                &[("username", "vitalik.eth")],
                "get_user_by_username",
            )
            .expect("get_user_by_username for vitalik.eth should succeed");

        let user2 = &user2_resp["user"];
        assert!(user2.get("fid").is_some(), "user should have an fid");
        let user2_fid = user2["fid"].as_u64().expect("fid should be a number");
        let user2_dname = user2["display_name"].as_str().unwrap_or("?");
        let user2_followers = &user2["follower_count"];
        println!(
            "  -> user profile: fid={user2_fid}, username='vitalik.eth', display_name='{user2_dname}', followers={user2_followers}"
        );

        // Step 3: Bulk-fetch both users + extras to compare profiles
        println!(
            "research_user_and_channel_workflow step 3: bulk-fetching fid={fid},{user2_fid} + extras ..."
        );
        let fids_param = format!("{fid},{user2_fid},99");
        let bulk_resp = client
            .get(
                "/farcaster/user/bulk",
                &[("fids", &fids_param)],
                "bulk_users",
            )
            .expect("bulk_users should succeed");

        let bulk_users = bulk_resp["users"]
            .as_array()
            .expect("bulk should return users array");
        assert!(bulk_users.len() >= 2, "should get at least 2 users");

        println!("  -> bulk-fetched {} user(s):", bulk_users.len());
        for (i, bu) in bulk_users.iter().enumerate() {
            let bfid = &bu["fid"];
            let buname = bu["username"].as_str().unwrap_or("?");
            let bdname = bu["display_name"].as_str().unwrap_or("?");
            let bfollowers = &bu["follower_count"];
            let eth_addrs = bu["verified_addresses"]["eth_addresses"]
                .as_array()
                .map_or(0, |a| a.len());
            println!(
                "  bulk[{i}]: fid={bfid}, username='{buname}', display_name='{bdname}', followers={bfollowers}, eth_addrs={eth_addrs}"
            );
        }

        // Step 4: Verify we have the data to decide who to collaborate with and post
        let has_verified = bulk_users.iter().any(|u| {
            u["verified_addresses"]["eth_addresses"]
                .as_array()
                .map_or(false, |a| !a.is_empty())
        });
        println!("  -> at least one user has verified ETH addresses: {has_verified}");

        println!(
            "research_user_and_channel_workflow step 5: skipping publish_cast (requires signer_uuid)"
        );
        println!(
            "research_user_and_channel_workflow (summary): dwr fid={}, vitalik fid={}, {} bulk profiles — ready to post collaboration cast",
            fid,
            user2_fid,
            bulk_users.len()
        );
    }
}
