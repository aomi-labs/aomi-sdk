use crate::client::*;
use aomi_sdk::*;
use serde_json::{Value, json};

// ============================================================================
// Tool 1: GetUserByUsername
// ============================================================================

impl DynAomiTool for GetUserByUsername {
    type App = NeynarApp;
    type Args = GetUserByUsernameArgs;
    const NAME: &'static str = "get_user_by_username";
    const DESCRIPTION: &'static str = "Look up a Farcaster user profile by username. Returns display name, bio, follower count, FID, and more.";

    fn run(_app: &NeynarApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = NeynarClient::new()?;
        let username = args.username.trim_start_matches('@');
        client.get(
            "/farcaster/user/by_username",
            &[("username", username)],
            "get_user_by_username",
        )
    }
}

// ============================================================================
// Tool 2: SearchUsers
// ============================================================================

impl DynAomiTool for SearchUsers {
    type App = NeynarApp;
    type Args = SearchUsersArgs;
    const NAME: &'static str = "search_users";
    const DESCRIPTION: &'static str =
        "Search for Farcaster users by name or keyword. Returns a list of matching user profiles.";

    fn run(_app: &NeynarApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = NeynarClient::new()?;
        client.get(
            "/farcaster/user/search",
            &[("q", args.q.as_str())],
            "search_users",
        )
    }
}

// ============================================================================
// Tool 3: GetFeed
// ============================================================================

impl DynAomiTool for GetFeed {
    type App = NeynarApp;
    type Args = GetFeedArgs;
    const NAME: &'static str = "get_feed";
    const DESCRIPTION: &'static str =
        "Get casts from a Farcaster feed. Supports filtering by feed type, FID, and result limit.";

    fn run(_app: &NeynarApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = NeynarClient::new()?;
        let fid_str = args.fid.map(|f| f.to_string());
        let limit_str = args.limit.unwrap_or(25).to_string();
        let mut params: Vec<(&str, &str)> = vec![
            ("feed_type", args.feed_type.as_str()),
            ("limit", limit_str.as_str()),
        ];
        if let Some(ref fid) = fid_str {
            params.push(("fid", fid.as_str()));
        }
        client.get("/farcaster/feed", &params, "get_feed")
    }
}

// ============================================================================
// Tool 4: GetCast
// ============================================================================

impl DynAomiTool for GetCast {
    type App = NeynarApp;
    type Args = GetCastArgs;
    const NAME: &'static str = "get_cast";
    const DESCRIPTION: &'static str = "Get a single Farcaster cast by its hash or Warpcast URL. Returns cast content, author, reactions, and replies.";

    fn run(_app: &NeynarApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = NeynarClient::new()?;
        client.get(
            "/farcaster/cast",
            &[
                ("identifier", args.identifier.as_str()),
                ("type", args.id_type.as_str()),
            ],
            "get_cast",
        )
    }
}

// ============================================================================
// Tool 5: SearchCasts
// ============================================================================

impl DynAomiTool for SearchCasts {
    type App = NeynarApp;
    type Args = SearchCastsArgs;
    const NAME: &'static str = "search_casts";
    const DESCRIPTION: &'static str = "Search for Farcaster casts by keyword. Returns matching casts with content, author info, and engagement metrics.";

    fn run(_app: &NeynarApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = NeynarClient::new()?;
        let limit_str = args.limit.unwrap_or(25).to_string();
        client.get(
            "/farcaster/cast/search",
            &[("q", args.q.as_str()), ("limit", limit_str.as_str())],
            "search_casts",
        )
    }
}

// ============================================================================
// Tool 6: PublishCast
// ============================================================================

impl DynAomiTool for PublishCast {
    type App = NeynarApp;
    type Args = PublishCastArgs;
    const NAME: &'static str = "publish_cast";
    const DESCRIPTION: &'static str = "Publish a new cast to Farcaster. Requires a signer_uuid authorized to act on behalf of the user.";

    fn run(_app: &NeynarApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = NeynarClient::new()?;
        let mut body = json!({
            "signer_uuid": args.signer_uuid,
            "text": args.text,
        });
        if let Some(embeds) = args.embeds {
            let embed_vals: Vec<Value> = embeds
                .into_iter()
                .map(|e| json!({ "url": e.url }))
                .collect();
            body.as_object_mut()
                .unwrap()
                .insert("embeds".to_string(), Value::Array(embed_vals));
        }
        client.post_json("/farcaster/cast", &body, "publish_cast")
    }
}

// ============================================================================
// Tool 7: GetChannel
// ============================================================================

impl DynAomiTool for GetChannel {
    type App = NeynarApp;
    type Args = GetChannelArgs;
    const NAME: &'static str = "get_channel";
    const DESCRIPTION: &'static str = "Get information about a Farcaster channel by its ID. Returns channel name, description, follower count, and image.";

    fn run(_app: &NeynarApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = NeynarClient::new()?;
        client.get(
            "/farcaster/channel",
            &[("id", args.id.as_str())],
            "get_channel",
        )
    }
}

// ============================================================================
// Tool 8: GetTrendingFeed
// ============================================================================

impl DynAomiTool for GetTrendingFeed {
    type App = NeynarApp;
    type Args = GetTrendingFeedArgs;
    const NAME: &'static str = "get_trending_feed";
    const DESCRIPTION: &'static str =
        "Get trending casts on Farcaster. Returns popular casts within a configurable time window.";

    fn run(_app: &NeynarApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = NeynarClient::new()?;
        let limit_str = args.limit.unwrap_or(10).to_string();
        let time_window = args.time_window.unwrap_or_else(|| "24h".to_string());
        client.get(
            "/farcaster/trending/feed",
            &[
                ("limit", limit_str.as_str()),
                ("time_window", time_window.as_str()),
            ],
            "get_trending_feed",
        )
    }
}
