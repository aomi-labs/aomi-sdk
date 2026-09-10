//! Curated tool layer for X (Twitter) on the official X API v2.
//!
//! Five read-only tools, same names as the previous twitterapi.io-backed
//! version so existing prompts keep working, now mapped onto v2:
//!
//!   * `get_x_user`        — GET /users/by/username/:handle
//!   * `get_x_user_posts`  — GET /users/:id/tweets (handle resolved first)
//!   * `search_x`          — GET /tweets/search/recent
//!   * `get_x_trends`      — GET /trends/by/woeid/:woeid
//!   * `get_x_post`        — GET /tweets/:id
//!
//! Pagination uses v2's `next_token`, surfaced as `next_cursor`.

use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};

fn q(key: &'static str, value: impl ToString) -> (&'static str, String) {
    (key, value.to_string())
}

fn clean_handle(raw: &str) -> Result<String, String> {
    let handle = raw.trim().trim_start_matches('@').to_string();
    if handle.is_empty()
        || !handle
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(format!("[x] `{raw}` is not a valid X handle"));
    }
    Ok(handle)
}

fn cursor_param(cursor: &Option<String>) -> Option<String> {
    cursor
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_string)
}

// ============================================================================
// get_x_user
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetXUserArgs {
    /// X handle without the @ (e.g. `elonmusk`); a leading @ is tolerated
    pub(crate) username: String,
}

pub(crate) struct GetXUser;

impl DynAomiTool for GetXUser {
    type App = XApp;
    type Args = GetXUserArgs;
    const NAME: &'static str = "get_x_user";
    const DESCRIPTION: &'static str = "Look up an X account by handle: bio, join date, verification, follower/following/post counts. Use for any 'who is @handle' or 'how many followers' question.";

    fn run(_app: &XApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let handle = clean_handle(&args.username)?;
        let client = XClient::from_ctx(&ctx)?;
        let (_, data) = client.user_id_for(&handle)?;
        Ok(json!({ "source": "x", "user": normalize_user(&Value::Object(data)) }))
    }
}

// ============================================================================
// get_x_user_posts
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetXUserPostsArgs {
    /// X handle without the @
    pub(crate) username: String,
    /// Posts per page (default 20, min 5, max 100)
    pub(crate) limit: Option<u32>,
    /// `true` to drop replies and reposts and keep only original posts and quotes (default false)
    pub(crate) originals_only: Option<bool>,
    /// `next_cursor` from a previous page
    pub(crate) cursor: Option<String>,
}

pub(crate) struct GetXUserPosts;

impl DynAomiTool for GetXUserPosts {
    type App = XApp;
    type Args = GetXUserPostsArgs;
    const NAME: &'static str = "get_x_user_posts";
    const DESCRIPTION: &'static str = "Recent posts from one account, newest first, with engagement metrics and a `next_cursor` for more. Use for 'what has @handle been posting'. Costs two API calls on the first page (handle lookup + timeline).";

    fn run(_app: &XApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let handle = clean_handle(&args.username)?;
        let client = XClient::from_ctx(&ctx)?;
        let (id, user) = client.user_id_for(&handle)?;
        let mut query = vec![
            q("max_results", clamp_page(args.limit, 20)),
            q("tweet.fields", TWEET_FIELDS),
            q("expansions", "author_id"),
            q("user.fields", "username,name"),
        ];
        if args.originals_only.unwrap_or(false) {
            query.push(q("exclude", "replies,retweets"));
        }
        if let Some(cursor) = cursor_param(&args.cursor) {
            query.push(q("pagination_token", cursor));
        }
        let value = client.get(&format!("/users/{id}/tweets"), &query)?;
        let mut page = normalize_page(&value);
        page["source"] = json!("x");
        page["user"] = json!({
            "id": id,
            "username": user.get("username"),
            "name": user.get("name"),
        });
        Ok(page)
    }
}

// ============================================================================
// search_x
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchXArgs {
    /// X search query. Operators: `from:user`, `to:user`, `@user`, `#tag`, `lang:en`, `has:media`, `has:links`, `-is:retweet`, `-is:reply`, `"exact phrase"`, `-word`. Max 512 characters.
    pub(crate) query: String,
    /// `Latest` (newest first, default) or `Top` (most relevant)
    pub(crate) query_type: Option<String>,
    /// Only posts after this time, ISO-8601 (e.g. `2026-09-01T00:00:00Z`). Recent search covers the last 7 days only.
    pub(crate) since: Option<String>,
    /// Only posts before this time, ISO-8601
    pub(crate) until: Option<String>,
    /// Posts per page (default 20, min 10, max 100)
    pub(crate) limit: Option<u32>,
    /// `next_cursor` from a previous page
    pub(crate) cursor: Option<String>,
}

pub(crate) struct SearchX;

impl DynAomiTool for SearchX {
    type App = XApp;
    type Args = SearchXArgs;
    const NAME: &'static str = "search_x";
    const DESCRIPTION: &'static str = "Search posts from the last 7 days by keyword, hashtag, account, or language, newest-first or most-relevant. Use for 'find posts about', 'what is @handle saying about', and hashtag monitoring. Returns normalized posts with author handles and a `next_cursor`.";

    fn run(_app: &XApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let query_text = args.query.trim();
        if query_text.is_empty() {
            return Err("[x] query is required".to_string());
        }
        if query_text.chars().count() > 512 {
            return Err("[x] query exceeds the 512-character limit of recent search".to_string());
        }
        let sort_order = match args
            .query_type
            .as_deref()
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref()
        {
            None | Some("latest") | Some("recency") | Some("recent") => "recency",
            Some("top") | Some("relevancy") | Some("popular") => "relevancy",
            Some(other) => {
                return Err(format!(
                    "[x] unknown query_type `{other}`; use Latest or Top"
                ));
            }
        };
        // Recent search requires at least 10 results per page.
        let limit = clamp_page(args.limit, 20).max(10);
        let mut query = vec![
            q("query", query_text),
            q("sort_order", sort_order),
            q("max_results", limit),
            q("tweet.fields", TWEET_FIELDS),
            q("expansions", "author_id"),
            q("user.fields", "username,name,verified"),
        ];
        if let Some(since) = args
            .since
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            query.push(q("start_time", since));
        }
        if let Some(until) = args
            .until
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            query.push(q("end_time", until));
        }
        if let Some(cursor) = cursor_param(&args.cursor) {
            query.push(q("next_token", cursor));
        }
        let client = XClient::from_ctx(&ctx)?;
        let value = client.get("/tweets/search/recent", &query)?;
        let mut page = normalize_page(&value);
        page["source"] = json!("x");
        page["query"] = json!(query_text);
        page["sort_order"] = json!(sort_order);
        Ok(page)
    }
}

// ============================================================================
// get_x_trends
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetXTrendsArgs {
    /// Yahoo WOEID location id: 1 = worldwide (default), 23424977 = United States, 23424975 = United Kingdom, 23424856 = Japan
    pub(crate) woeid: Option<u64>,
    /// Number of trends (default 20, max 50)
    pub(crate) count: Option<u32>,
}

pub(crate) struct GetXTrends;

impl DynAomiTool for GetXTrends {
    type App = XApp;
    type Args = GetXTrendsArgs;
    const NAME: &'static str = "get_x_trends";
    const DESCRIPTION: &'static str = "Currently trending topics on X, worldwide by default or for a WOEID location. Returns trend names with post counts when X provides them.";

    fn run(_app: &XApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let woeid = args.woeid.unwrap_or(1);
        let count = args.count.unwrap_or(20).clamp(1, 50);
        let client = XClient::from_ctx(&ctx)?;
        let value = client.get(
            &format!("/trends/by/woeid/{woeid}"),
            &[q("max_trends", count)],
        )?;
        let trends: Vec<Value> = value
            .get("data")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        json!({
                            "rank": i + 1,
                            "name": t.get("trend_name"),
                            "post_count": t.get("tweet_count"),
                            "category": t.get("category"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(json!({
            "source": "x",
            "woeid": woeid,
            "count": trends.len(),
            "trends": trends,
        }))
    }
}

// ============================================================================
// get_x_post
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetXPostArgs {
    /// Numeric post id, or a full `https://x.com/<user>/status/<id>` URL
    pub(crate) post_id: String,
}

pub(crate) struct GetXPost;

impl DynAomiTool for GetXPost {
    type App = XApp;
    type Args = GetXPostArgs;
    const NAME: &'static str = "get_x_post";
    const DESCRIPTION: &'static str = "Fetch one post by id or URL: full text, author, timestamp, and engagement (likes, reposts, replies, quotes, bookmarks, impressions).";

    fn run(_app: &XApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let id = extract_post_id(&args.post_id)?;
        let client = XClient::from_ctx(&ctx)?;
        let value = client.get(
            &format!("/tweets/{id}"),
            &[
                q("tweet.fields", TWEET_FIELDS),
                q("expansions", "author_id"),
                q("user.fields", "username,name,verified"),
            ],
        )?;
        let users = users_by_id(&value);
        let tweet = value
            .get("data")
            .ok_or_else(|| format!("[x] post {id} not found"))?;
        let author = tweet
            .get("author_id")
            .and_then(Value::as_str)
            .and_then(|a| users.get(a));
        Ok(json!({ "source": "x", "post": normalize_post(tweet, author) }))
    }
}

/// Accept a bare id or any x.com / twitter.com status URL.
pub(crate) fn extract_post_id(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    let candidate = raw
        .rsplit("/status/")
        .next()
        .unwrap_or(raw)
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap_or("");
    if candidate.is_empty() {
        return Err(format!("[x] `{raw}` is not a post id or status URL"));
    }
    Ok(candidate.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aomi_sdk::testing::{TestCtxBuilder, run_tool};

    #[test]
    fn handles_and_post_ids_are_cleaned() {
        assert_eq!(clean_handle(" @Aomi_labs ").unwrap(), "Aomi_labs");
        assert!(clean_handle("not a handle").is_err());
        assert_eq!(extract_post_id("20").unwrap(), "20");
        assert_eq!(
            extract_post_id("https://x.com/jack/status/20?s=46&t=abc").unwrap(),
            "20"
        );
        assert_eq!(
            extract_post_id("https://twitter.com/jack/status/20/photo/1").unwrap(),
            "20"
        );
        assert!(extract_post_id("https://x.com/jack").is_err());
    }

    #[test]
    fn missing_token_fails_before_any_request() {
        let ctx = TestCtxBuilder::new(GetXUser::NAME).build();
        let err = run_tool::<GetXUser>(&XApp, json!({ "username": "aomi_labs" }), ctx).unwrap_err();
        assert!(err.contains("X_API_KEY"), "{err}");
        assert!(err.contains("Bearer Token"), "{err}");
    }

    #[test]
    fn search_validates_query_type_before_network() {
        let ctx = TestCtxBuilder::new(SearchX::NAME)
            .secret(API_KEY_NAME, "t")
            .build();
        let err = run_tool::<SearchX>(
            &XApp,
            json!({ "query": "aomi", "query_type": "Newest" }),
            ctx,
        )
        .unwrap_err();
        assert!(err.contains("Latest or Top"), "{err}");
        let ctx = TestCtxBuilder::new(SearchX::NAME)
            .secret(API_KEY_NAME, "t")
            .build();
        let err = run_tool::<SearchX>(&XApp, json!({ "query": "   " }), ctx).unwrap_err();
        assert!(err.contains("required"), "{err}");
    }

    /// Live ladder on the official API: profile → timeline → search → trends
    /// → single post. Five to six billable calls. Needs `X_API_KEY` set to an
    /// app bearer token.
    #[test]
    #[ignore = "network: hits api.x.com and needs X_API_KEY"]
    fn live_read_ladder() {
        let token = std::env::var(API_KEY_NAME).expect("X_API_KEY must be set");
        let ctx = |name: &str| {
            TestCtxBuilder::new(name)
                .secret(API_KEY_NAME, token.clone())
                .build()
        };

        let user = run_tool::<GetXUser>(&XApp, json!({ "username": "@aomi_labs" }), ctx("u"))
            .expect("user")
            .into_value();
        assert_eq!(user["user"]["username"], "aomi_labs", "{user}");

        let posts = run_tool::<GetXUserPosts>(
            &XApp,
            json!({ "username": "aomi_labs", "limit": 5 }),
            ctx("p"),
        )
        .expect("posts")
        .into_value();
        assert!(posts["count"].as_u64().unwrap() >= 1, "{posts}");
        assert_eq!(posts["posts"][0]["author_username"], "aomi_labs");

        let search = run_tool::<SearchX>(
            &XApp,
            json!({ "query": "from:aomi_labs -is:retweet", "limit": 10 }),
            ctx("s"),
        )
        .expect("search")
        .into_value();
        assert!(search["posts"].is_array(), "{search}");

        let trends = run_tool::<GetXTrends>(&XApp, json!({ "count": 5 }), ctx("t"))
            .expect("trends")
            .into_value();
        assert!(trends["count"].as_u64().unwrap() >= 1, "{trends}");

        let post = run_tool::<GetXPost>(
            &XApp,
            json!({ "post_id": "https://x.com/jack/status/20" }),
            ctx("x"),
        )
        .expect("post")
        .into_value();
        assert_eq!(post["post"]["author_username"], "jack", "{post}");
    }
}
