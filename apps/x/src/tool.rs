use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{json, Value};

impl DynAomiTool for GetXUser {
    type App = XApp;
    type Args = GetXUserArgs;
    const NAME: &'static str = "get_x_user";
    const DESCRIPTION: &'static str = "Get an X (Twitter) user's profile information by username. Returns follower count, bio, verification status, and more.";

    fn run(_app: &XApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = XClient::new()?;
        let username = args.username.trim_start_matches('@');
        let user: User = client.get("/twitter/user/info", &[("userName", username)])?;
        Ok(json!({
            "id": user.id,
            "username": user.user_name,
            "name": user.name,
            "bio": user.description,
            "location": user.location,
            "url": user.url,
            "profile_image": user.profile_image_url,
            "banner_image": user.profile_banner_url,
            "followers": user.followers_count,
            "following": user.following_count,
            "posts_count": user.statuses_count,
            "likes_count": user.favourites_count,
            "listed_count": user.listed_count,
            "created_at": user.created_at,
            "verified": user.verified,
            "blue_verified": user.is_blue_verified,
        }))
    }
}

// ============================================================================
// Tool 2: GetXUserPosts
// ============================================================================

pub(crate) struct GetXUserPosts;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetXUserPostsArgs {
    /// X username without the @ symbol
    username: String,
    /// Pagination cursor for fetching more results
    cursor: Option<String>,
}

impl DynAomiTool for GetXUserPosts {
    type App = XApp;
    type Args = GetXUserPostsArgs;
    const NAME: &'static str = "get_x_user_posts";
    const DESCRIPTION: &'static str = "Get recent posts from an X (Twitter) user. Returns post text, engagement metrics, and metadata.";

    fn run(_app: &XApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = XClient::new()?;
        let username = args.username.trim_start_matches('@');
        let mut query: Vec<(&str, &str)> = vec![("userName", username)];
        let cursor_val = args.cursor.unwrap_or_default();
        if !cursor_val.is_empty() {
            query.push(("cursor", &cursor_val));
        }
        let data: PostsData = client.get("/twitter/user/last_tweets", &query)?;
        let posts = data.tweets.unwrap_or_default();
        let formatted: Vec<Value> = posts.iter().map(format_post).collect();
        Ok(json!({
            "posts_count": formatted.len(),
            "posts": formatted,
            "cursor": data.next_cursor,
            "has_more": data.has_next_page.unwrap_or(false),
        }))
    }
}

// ============================================================================
// Tool 3: SearchX
// ============================================================================

pub(crate) struct SearchX;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchXArgs {
    /// Search query. Supports operators: from:user, #hashtag, @mention, lang:en, since:2026-01-01, until:2026-02-01, min_faves:100
    query: String,
    /// Sort order: 'Latest' for recent posts, 'Top' for popular posts (default: Latest)
    query_type: Option<String>,
    /// Pagination cursor for fetching more results
    cursor: Option<String>,
}

impl DynAomiTool for SearchX {
    type App = XApp;
    type Args = SearchXArgs;
    const NAME: &'static str = "search_x";
    const DESCRIPTION: &'static str = "Search for posts on X (Twitter) using advanced query operators. Supports filtering by user, hashtag, date range, and engagement metrics.";

    fn run(_app: &XApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = XClient::new()?;
        let query_type = args.query_type.as_deref().unwrap_or("Latest");
        let mut params: Vec<(&str, &str)> = vec![("query", &args.query), ("queryType", query_type)];
        let cursor_val = args.cursor.unwrap_or_default();
        if !cursor_val.is_empty() {
            params.push(("cursor", &cursor_val));
        }
        let data: PostsData = client.get("/twitter/tweet/advanced_search", &params)?;
        let posts = data.tweets.unwrap_or_default();
        let formatted: Vec<Value> = posts.iter().map(format_post).collect();
        Ok(json!({
            "query": args.query,
            "query_type": query_type,
            "results_count": formatted.len(),
            "posts": formatted,
            "cursor": data.next_cursor,
            "has_more": data.has_next_page.unwrap_or(false),
        }))
    }
}

// ============================================================================
// Tool 4: GetXTrends
// ============================================================================

pub(crate) struct GetXTrends;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetXTrendsArgs {}

impl DynAomiTool for GetXTrends {
    type App = XApp;
    type Args = GetXTrendsArgs;
    const NAME: &'static str = "get_x_trends";
    const DESCRIPTION: &'static str =
        "Get current trending topics on X (Twitter). Returns trend names and post counts.";

    fn run(_app: &XApp, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = XClient::new()?;
        let data: TrendsData = client.get("/twitter/trends", &[])?;
        let trends = data.trends.unwrap_or_default();
        let formatted: Vec<Value> = trends
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "url": t.url,
                    "post_count": t.tweet_count,
                    "description": t.description,
                    "category": t.domain_context,
                })
            })
            .collect();
        Ok(json!({
            "trends_count": formatted.len(),
            "trends": formatted,
        }))
    }
}

// ============================================================================
// Tool 5: GetXPost
// ============================================================================

pub(crate) struct GetXPost;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetXPostArgs {
    /// The ID of the post to retrieve
    post_id: String,
}

impl DynAomiTool for GetXPost {
    type App = XApp;
    type Args = GetXPostArgs;
    const NAME: &'static str = "get_x_post";
    const DESCRIPTION: &'static str = "Get details of a specific X (Twitter) post by its ID. Returns full post content, engagement metrics, and author info.";

    fn run(_app: &XApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = XClient::new()?;
        let post: Post =
            client.get("/twitter/tweet/info", &[("tweetId", args.post_id.as_str())])?;
        Ok(format_post(&post))
    }
}
