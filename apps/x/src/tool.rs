use crate::client::*;
use crate::types::{
    AdvancedSearchQuery, TrendsQuery, TweetInfoQuery, UserInfoQuery, UserLastTweetsQuery,
    XPostsView, XSearchResultsView, XTrendsView,
};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[x] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("x".to_string()));
            Value::Object(map)
        }
        other => serde_json::json!({ "source": "x", "data": other }),
    })
}

impl DynAomiTool for GetXUser {
    type App = XApp;
    type Args = GetXUserArgs;
    const NAME: &'static str = "get_x_user";
    const DESCRIPTION: &'static str = "Get an X (Twitter) user's profile information by username. Returns follower count, bio, verification status, and more.";

    fn run(_app: &XApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = XClient::new(args.api_key.as_deref())?;
        let username = args.username.trim_start_matches('@');
        let user: User = client.get(
            "/twitter/user/info",
            &UserInfoQuery {
                user_name: username,
            },
        )?;
        ok(format_user(&user))
    }
}

// ============================================================================
// Tool 2: GetXUserPosts
// ============================================================================

pub(crate) struct GetXUserPosts;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetXUserPostsArgs {
    /// Optional X API key. Falls back to X_API_KEY when omitted.
    #[serde(default)]
    api_key: Option<String>,
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
        let client = XClient::new(args.api_key.as_deref())?;
        let username = args.username.trim_start_matches('@');
        let data: PostsData = client.get(
            "/twitter/user/last_tweets",
            &UserLastTweetsQuery {
                user_name: username,
                cursor: args.cursor.as_deref().filter(|cursor| !cursor.is_empty()),
            },
        )?;
        let posts = data.tweets.unwrap_or_default();
        let formatted = posts.iter().map(format_post).collect::<Vec<_>>();
        ok(XPostsView {
            posts_count: formatted.len(),
            posts: formatted,
            cursor: data.next_cursor,
            has_more: data.has_next_page.unwrap_or(false),
        })
    }
}

// ============================================================================
// Tool 3: SearchX
// ============================================================================

pub(crate) struct SearchX;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchXArgs {
    /// Optional X API key. Falls back to X_API_KEY when omitted.
    #[serde(default)]
    api_key: Option<String>,
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
        let client = XClient::new(args.api_key.as_deref())?;
        let query_type = args.query_type.as_deref().unwrap_or("Latest");
        let data: PostsData = client.get(
            "/twitter/tweet/advanced_search",
            &AdvancedSearchQuery {
                query: args.query.as_str(),
                query_type,
                cursor: args.cursor.as_deref().filter(|cursor| !cursor.is_empty()),
            },
        )?;
        let posts = data.tweets.unwrap_or_default();
        let formatted = posts.iter().map(format_post).collect::<Vec<_>>();
        ok(XSearchResultsView {
            query: args.query,
            query_type: query_type.to_string(),
            results_count: formatted.len(),
            posts: formatted,
            cursor: data.next_cursor,
            has_more: data.has_next_page.unwrap_or(false),
        })
    }
}

// ============================================================================
// Tool 4: GetXTrends
// ============================================================================

pub(crate) struct GetXTrends;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetXTrendsArgs {
    /// Optional X API key. Falls back to X_API_KEY when omitted.
    #[serde(default)]
    api_key: Option<String>,
    /// Optional Yahoo WOEID location ID. The twitterapi.io trends docs document this parameter.
    #[serde(default)]
    woeid: Option<u64>,
    /// Optional number of trends to return.
    #[serde(default)]
    count: Option<u64>,
}

impl DynAomiTool for GetXTrends {
    type App = XApp;
    type Args = GetXTrendsArgs;
    const NAME: &'static str = "get_x_trends";
    const DESCRIPTION: &'static str =
        "Get current trending topics on X (Twitter). Returns trend names and post counts.";

    fn run(_app: &XApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = XClient::new(args.api_key.as_deref())?;
        let data: TrendsData = client.get(
            "/twitter/trends",
            &TrendsQuery {
                woeid: args.woeid,
                count: args.count,
            },
        )?;
        let trends = data.trends.unwrap_or_default();
        let formatted = trends.iter().map(format_trend).collect::<Vec<_>>();
        ok(XTrendsView {
            trends_count: formatted.len(),
            trends: formatted,
        })
    }
}

// ============================================================================
// Tool 5: GetXPost
// ============================================================================

pub(crate) struct GetXPost;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetXPostArgs {
    /// Optional X API key. Falls back to X_API_KEY when omitted.
    #[serde(default)]
    api_key: Option<String>,
    /// The ID of the post to retrieve
    post_id: String,
}

impl DynAomiTool for GetXPost {
    type App = XApp;
    type Args = GetXPostArgs;
    const NAME: &'static str = "get_x_post";
    const DESCRIPTION: &'static str = "Get details of a specific X (Twitter) post by its ID. Returns full post content, engagement metrics, and author info.";

    fn run(_app: &XApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = XClient::new(args.api_key.as_deref())?;
        let post: Post = client.get(
            "/twitter/tweet/info",
            &TweetInfoQuery {
                tweet_id: args.post_id.as_str(),
            },
        )?;
        ok(format_post(&post))
    }
}
