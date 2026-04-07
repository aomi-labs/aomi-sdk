use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};

impl DynAomiTool for GetXUser {
    type App = SocialApp;
    type Args = GetXUserArgs;
    const NAME: &'static str = "get_x_user";
    const DESCRIPTION: &'static str = "Get an X (Twitter) user's profile information by username. Returns follower count, bio, verification status, and more.";

    fn run(_app: &SocialApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = XClient::new(args.api_key.as_deref())?;
        let username = args.username.trim_start_matches('@');
        let user: XUser = client.get("/twitter/user/info", &[("userName", username)])?;
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
    /// Optional X API key. Falls back to X_API_KEY when omitted.
    api_key: Option<String>,
    /// X username without the @ symbol
    username: String,
    /// Pagination cursor for fetching more results
    cursor: Option<String>,
}

impl DynAomiTool for GetXUserPosts {
    type App = SocialApp;
    type Args = GetXUserPostsArgs;
    const NAME: &'static str = "get_x_user_posts";
    const DESCRIPTION: &'static str = "Get recent posts from an X (Twitter) user. Returns post text, engagement metrics, and metadata.";

    fn run(_app: &SocialApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = XClient::new(args.api_key.as_deref())?;
        let username = args.username.trim_start_matches('@');
        let mut query: Vec<(&str, &str)> = vec![("userName", username)];
        let cursor_val = args.cursor.unwrap_or_default();
        if !cursor_val.is_empty() {
            query.push(("cursor", &cursor_val));
        }
        let data: XPostsData = client.get("/twitter/user/last_tweets", &query)?;
        let posts = data.tweets.unwrap_or_default();
        let formatted: Vec<Value> = posts.iter().map(format_x_post).collect();
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
    /// Optional X API key. Falls back to X_API_KEY when omitted.
    api_key: Option<String>,
    /// Search query. Supports operators: from:user, #hashtag, @mention, lang:en, since:2026-01-01, until:2026-02-01, min_faves:100
    query: String,
    /// Sort order: 'Latest' for recent posts, 'Top' for popular posts (default: Latest)
    query_type: Option<String>,
    /// Pagination cursor for fetching more results
    cursor: Option<String>,
}

impl DynAomiTool for SearchX {
    type App = SocialApp;
    type Args = SearchXArgs;
    const NAME: &'static str = "search_x";
    const DESCRIPTION: &'static str = "Search for posts on X (Twitter) using advanced query operators. Supports filtering by user, hashtag, date range, and engagement metrics.";

    fn run(_app: &SocialApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = XClient::new(args.api_key.as_deref())?;
        let query_type = args.query_type.as_deref().unwrap_or("Latest");
        let mut params: Vec<(&str, &str)> = vec![("query", &args.query), ("queryType", query_type)];
        let cursor_val = args.cursor.unwrap_or_default();
        if !cursor_val.is_empty() {
            params.push(("cursor", &cursor_val));
        }
        let data: XPostsData = client.get("/twitter/tweet/advanced_search", &params)?;
        let posts = data.tweets.unwrap_or_default();
        let formatted: Vec<Value> = posts.iter().map(format_x_post).collect();
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
pub(crate) struct GetXTrendsArgs {
    /// Optional X API key. Falls back to X_API_KEY when omitted.
    api_key: Option<String>,
}

impl DynAomiTool for GetXTrends {
    type App = SocialApp;
    type Args = GetXTrendsArgs;
    const NAME: &'static str = "get_x_trends";
    const DESCRIPTION: &'static str =
        "Get current trending topics on X (Twitter). Returns trend names and post counts.";

    fn run(_app: &SocialApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = XClient::new(args.api_key.as_deref())?;
        let data: XTrendsData = client.get("/twitter/trends", &[])?;
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
    /// Optional X API key. Falls back to X_API_KEY when omitted.
    api_key: Option<String>,
    /// The ID of the post to retrieve
    post_id: String,
}

impl DynAomiTool for GetXPost {
    type App = SocialApp;
    type Args = GetXPostArgs;
    const NAME: &'static str = "get_x_post";
    const DESCRIPTION: &'static str = "Get details of a specific X (Twitter) post by its ID. Returns full post content, engagement metrics, and author info.";

    fn run(_app: &SocialApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = XClient::new(args.api_key.as_deref())?;
        let post: XPost =
            client.get("/twitter/tweet/info", &[("tweetId", args.post_id.as_str())])?;
        Ok(format_x_post(&post))
    }
}

// ############################################################################
//                      FARCASTER TOOLS (4)
// ############################################################################

// ============================================================================
// Tool 6: SearchFarcaster
// ============================================================================

pub(crate) struct SearchFarcaster;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchFarcasterArgs {
    /// Optional Neynar API key. Falls back to NEYNAR_API_KEY when omitted.
    api_key: Option<String>,
    /// Search query for casts. Supports text search, @mentions, and channel names.
    query: String,
    /// Pagination cursor for fetching more results
    cursor: Option<String>,
    /// Number of results to return (default: 25, max: 100)
    limit: Option<u32>,
}

impl DynAomiTool for SearchFarcaster {
    type App = SocialApp;
    type Args = SearchFarcasterArgs;
    const NAME: &'static str = "search_farcaster";
    const DESCRIPTION: &'static str = "Search for casts (posts) on Farcaster. Returns matching posts with author info, engagement metrics, and channel context.";

    fn run(_app: &SocialApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = NeynarClient::new(args.api_key.as_deref())?;
        let response = client.search_casts(&args.query, args.cursor.as_deref(), args.limit)?;

        let formatted_casts: Vec<Value> = response.casts.iter().map(format_cast).collect();

        Ok(json!({
            "query": args.query,
            "results_count": formatted_casts.len(),
            "casts": formatted_casts,
            "cursor": response.cursor,
        }))
    }
}

// ============================================================================
// Tool 7: GetFarcasterUser
// ============================================================================

pub(crate) struct GetFarcasterUser;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetFarcasterUserArgs {
    /// Optional Neynar API key. Falls back to NEYNAR_API_KEY when omitted.
    api_key: Option<String>,
    /// Username (e.g., 'vitalik.eth', 'dwr.eth') or FID (numeric ID like '3')
    identifier: String,
}

impl DynAomiTool for GetFarcasterUser {
    type App = SocialApp;
    type Args = GetFarcasterUserArgs;
    const NAME: &'static str = "get_farcaster_user";
    const DESCRIPTION: &'static str = "Get a Farcaster user's profile by username or FID. Returns follower count, bio, verified addresses (ETH/SOL), and more.";

    fn run(_app: &SocialApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = NeynarClient::new(args.api_key.as_deref())?;
        let identifier = args.identifier.trim_start_matches('@');

        let user = if let Ok(fid) = identifier.parse::<u64>() {
            client.get_user_by_fid(fid)?
        } else {
            client.get_user_by_username(identifier)?
        };

        let bio = user
            .profile
            .as_ref()
            .and_then(|p| p.bio.as_ref())
            .and_then(|b| b.text.clone());

        Ok(json!({
            "fid": user.fid,
            "username": user.username,
            "display_name": user.display_name,
            "bio": bio,
            "profile_image": user.pfp_url,
            "followers": user.follower_count,
            "following": user.following_count,
            "verified_addresses": {
                "ethereum": user.verified_addresses.as_ref().and_then(|v| v.eth_addresses.clone()),
                "solana": user.verified_addresses.as_ref().and_then(|v| v.sol_addresses.clone()),
            },
            "verifications": user.verifications,
        }))
    }
}

// ============================================================================
// Tool 8: GetFarcasterChannel
// ============================================================================

pub(crate) struct GetFarcasterChannel;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetFarcasterChannelArgs {
    /// Optional Neynar API key. Falls back to NEYNAR_API_KEY when omitted.
    api_key: Option<String>,
    /// Channel ID (e.g., 'base', 'degen', 'crypto', 'memes')
    channel_id: String,
    /// Include recent casts from the channel (default: true)
    include_feed: Option<bool>,
    /// Number of recent casts to include (default: 10)
    feed_limit: Option<u32>,
}

impl DynAomiTool for GetFarcasterChannel {
    type App = SocialApp;
    type Args = GetFarcasterChannelArgs;
    const NAME: &'static str = "get_farcaster_channel";
    const DESCRIPTION: &'static str = "Get information about a Farcaster channel including description, follower count, and optionally recent casts. Popular channels include /base, /degen, /crypto, /memes.";

    fn run(_app: &SocialApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = NeynarClient::new(args.api_key.as_deref())?;
        let channel = client.get_channel(&args.channel_id)?;

        let mut result = json!({
            "id": channel.id,
            "name": channel.name,
            "description": channel.description,
            "image": channel.image_url,
            "followers": channel.follower_count,
            "lead": channel.lead.map(|l| json!({
                "fid": l.fid,
                "username": l.username,
                "display_name": l.display_name,
            })),
        });

        // Include feed if requested (default: true)
        let include_feed = args.include_feed.unwrap_or(true);
        if include_feed {
            let feed_limit = args.feed_limit.unwrap_or(10);
            let feed = client.get_channel_feed(&args.channel_id, None, Some(feed_limit))?;
            let formatted_casts: Vec<Value> = feed.casts.iter().map(format_cast).collect();
            result["recent_casts"] = json!(formatted_casts);
        }

        Ok(result)
    }
}

// ============================================================================
// Tool 9: GetFarcasterTrending
// ============================================================================

pub(crate) struct GetFarcasterTrending;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetFarcasterTrendingArgs {
    /// Optional Neynar API key. Falls back to NEYNAR_API_KEY when omitted.
    api_key: Option<String>,
    /// Number of trending channels to return (default: 10)
    limit: Option<u32>,
}

impl DynAomiTool for GetFarcasterTrending {
    type App = SocialApp;
    type Args = GetFarcasterTrendingArgs;
    const NAME: &'static str = "get_farcaster_trending";
    const DESCRIPTION: &'static str = "Get trending Farcaster channels. Shows what topics and communities are gaining attention in the Web3 social space.";

    fn run(_app: &SocialApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = NeynarClient::new(args.api_key.as_deref())?;
        let channels = client.get_trending_channels(args.limit)?;

        let formatted_channels: Vec<Value> = channels
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "name": c.name,
                    "description": c.description,
                    "image": c.image_url,
                    "followers": c.follower_count,
                })
            })
            .collect();

        Ok(json!({
            "trending_count": formatted_channels.len(),
            "channels": formatted_channels,
        }))
    }
}

// ############################################################################
//                      LUNARCRUSH TOOLS (3)
// ############################################################################

// ============================================================================
// Tool 10: GetCryptoSentiment
// ============================================================================

pub(crate) struct GetCryptoSentiment;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCryptoSentimentArgs {
    /// Optional LunarCrush API key. Falls back to LUNARCRUSH_API_KEY when omitted.
    api_key: Option<String>,
    /// Crypto topic to get sentiment for (e.g., 'bitcoin', 'ethereum', 'solana')
    topic: String,
}

impl DynAomiTool for GetCryptoSentiment {
    type App = SocialApp;
    type Args = GetCryptoSentimentArgs;
    const NAME: &'static str = "get_crypto_sentiment";
    const DESCRIPTION: &'static str = "Get aggregated sentiment data for a crypto topic from X, Reddit, YouTube, TikTok, and news. Returns sentiment scores, social volume, contributor counts, and platform breakdown.";

    fn run(_app: &SocialApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LunarCrushClient::new(args.api_key.as_deref())?;
        let sentiment = client.get_topic_sentiment(&args.topic)?;

        // Calculate overall sentiment from platform breakdown
        let overall_sentiment = sentiment.types_sentiment.as_ref().map(|ts| {
            let values: Vec<u32> = ts.values().copied().collect();
            if values.is_empty() {
                50
            } else {
                values.iter().sum::<u32>() / values.len() as u32
            }
        });

        Ok(json!({
            "topic": sentiment.topic,
            "title": sentiment.title,
            "rank": sentiment.topic_rank,
            "trend": sentiment.trend,
            "overall_sentiment": overall_sentiment,
            "sentiment_breakdown": {
                "platforms": sentiment.types_sentiment,
                "details": sentiment.types_sentiment_detail.map(|d| {
                    d.iter().map(|(k, v)| {
                        (k.clone(), json!({
                            "positive": v.positive,
                            "neutral": v.neutral,
                            "negative": v.negative,
                        }))
                    }).collect::<serde_json::Map<String, Value>>()
                }),
            },
            "social_metrics": {
                "interactions_24h": sentiment.interactions_24h,
                "contributors": sentiment.num_contributors,
                "posts": sentiment.num_posts,
            },
            "platform_activity": {
                "post_counts": sentiment.types_count,
                "interactions": sentiment.types_interactions,
            },
            "related_topics": sentiment.related_topics,
            "categories": sentiment.categories,
        }))
    }
}

// ============================================================================
// Tool 11: GetTrendingTopics
// ============================================================================

pub(crate) struct GetTrendingTopics;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTrendingTopicsArgs {
    /// Optional LunarCrush API key. Falls back to LUNARCRUSH_API_KEY when omitted.
    api_key: Option<String>,
    /// Maximum number of trending topics to return (default: 20)
    limit: Option<u32>,
}

impl DynAomiTool for GetTrendingTopics {
    type App = SocialApp;
    type Args = GetTrendingTopicsArgs;
    const NAME: &'static str = "get_trending_topics";
    const DESCRIPTION: &'static str = "Get trending social topics across X, Reddit, YouTube, TikTok, and news. Shows what's gaining attention with rank changes and engagement metrics.";

    fn run(_app: &SocialApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LunarCrushClient::new(args.api_key.as_deref())?;
        let mut topics = client.get_trending_topics()?;

        // Limit results
        let limit = args.limit.unwrap_or(20) as usize;
        topics.truncate(limit);

        let formatted_topics: Vec<Value> = topics
            .iter()
            .map(|t| {
                let rank_change_1h = match (t.topic_rank, t.topic_rank_1h_previous) {
                    (Some(current), Some(prev)) => Some(prev as i32 - current as i32),
                    _ => None,
                };
                let rank_change_24h = match (t.topic_rank, t.topic_rank_24h_previous) {
                    (Some(current), Some(prev)) => Some(prev as i32 - current as i32),
                    _ => None,
                };

                json!({
                    "topic": t.topic,
                    "title": t.title,
                    "rank": t.topic_rank,
                    "rank_change_1h": rank_change_1h,
                    "rank_change_24h": rank_change_24h,
                    "contributors": t.num_contributors,
                    "posts": t.num_posts,
                    "interactions_24h": t.interactions_24h,
                })
            })
            .collect();

        Ok(json!({
            "trending_count": formatted_topics.len(),
            "topics": formatted_topics,
        }))
    }
}

// ============================================================================
// Tool 12: GetTopicSummary
// ============================================================================

pub(crate) struct GetTopicSummary;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTopicSummaryArgs {
    /// Optional LunarCrush API key. Falls back to LUNARCRUSH_API_KEY when omitted.
    api_key: Option<String>,
    /// Crypto topic to get a summary for (e.g., 'bitcoin', 'ethereum', 'solana')
    topic: String,
}

impl DynAomiTool for GetTopicSummary {
    type App = SocialApp;
    type Args = GetTopicSummaryArgs;
    const NAME: &'static str = "get_topic_summary";
    const DESCRIPTION: &'static str = "Get an AI-generated summary of the hottest news and social posts for a crypto topic. Provides a quick overview of what's being discussed.";

    fn run(_app: &SocialApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LunarCrushClient::new(args.api_key.as_deref())?;
        let summary = client.get_topic_summary(&args.topic)?;

        Ok(json!({
            "topic": summary.topic,
            "summary": summary.summary,
            "generated_at": summary.generated_at,
        }))
    }
}
