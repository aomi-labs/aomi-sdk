use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct SocialApp;

pub(crate) use crate::tool::*;

// ############################################################################
//                              X CLIENT
// ############################################################################

pub(crate) const X_API_BASE: &str = "https://api.twitterapi.io";

#[derive(Clone)]
pub(crate) struct XClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_key: String,
}

impl XClient {
    pub(crate) fn new() -> Result<Self, String> {
        let api_key = std::env::var("X_API_KEY")
            .map_err(|_| "X_API_KEY environment variable not set".to_string())?;
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        Ok(Self { http, api_key })
    }

    pub(crate) fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T, String> {
        let url = format!("{X_API_BASE}{path}");
        let resp = self
            .http
            .get(&url)
            .header("X-API-Key", &self.api_key)
            .query(query)
            .send()
            .map_err(|e| format!("X API request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("X API error {status}: {text}"));
        }

        let api_resp: XApiResponse<T> = resp
            .json()
            .map_err(|e| format!("X API decode failed: {e}"))?;

        if !api_resp.is_success() {
            return Err(format!("X API logical error: {}", api_resp.error_message()));
        }

        api_resp
            .data
            .ok_or_else(|| "X API returned empty data".to_string())
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct XApiResponse<T> {
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) msg: Option<String>,
    #[serde(default)]
    pub(crate) message: Option<String>,
    #[serde(default)]
    pub(crate) code: Option<i64>,
    #[serde(default)]
    pub(crate) success: Option<bool>,
    pub(crate) data: Option<T>,
}

impl<T> XApiResponse<T> {
    pub(crate) fn is_success(&self) -> bool {
        if let Some(success) = self.success {
            return success;
        }
        if let Some(ref status) = self.status {
            if status.eq_ignore_ascii_case("success") || status.eq_ignore_ascii_case("ok") {
                return true;
            }
        }
        if let Some(code) = self.code {
            return code == 0 || code == 200;
        }
        false
    }

    pub(crate) fn error_message(&self) -> String {
        self.msg
            .clone()
            .or_else(|| self.message.clone())
            .unwrap_or_else(|| "Unknown API error".to_string())
    }
}

// ============================================================================
// X Data Models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct XUser {
    #[serde(default, deserialize_with = "de_opt_string")]
    pub(crate) id: Option<String>,
    pub(crate) user_name: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) location: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) profile_image_url: Option<String>,
    pub(crate) profile_banner_url: Option<String>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub(crate) followers_count: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub(crate) following_count: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub(crate) favourites_count: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub(crate) statuses_count: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub(crate) listed_count: Option<u64>,
    pub(crate) created_at: Option<String>,
    pub(crate) verified: Option<bool>,
    pub(crate) is_blue_verified: Option<bool>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct XPost {
    #[serde(default, deserialize_with = "de_opt_string")]
    pub(crate) id: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) full_text: Option<String>,
    pub(crate) created_at: Option<String>,
    pub(crate) author: Option<XPostAuthor>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub(crate) retweet_count: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub(crate) favorite_count: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub(crate) reply_count: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub(crate) quote_count: Option<u64>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub(crate) view_count: Option<u64>,
    pub(crate) lang: Option<String>,
    pub(crate) is_retweet: Option<bool>,
    pub(crate) is_quote: Option<bool>,
    pub(crate) in_reply_to_status_id: Option<String>,
    pub(crate) conversation_id: Option<String>,
    pub(crate) hashtags: Option<Vec<String>>,
    pub(crate) mentions: Option<Vec<XMention>>,
    pub(crate) urls: Option<Vec<XUrlEntity>>,
    pub(crate) media: Option<Vec<XMedia>>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct XPostAuthor {
    #[serde(default, deserialize_with = "de_opt_string")]
    pub(crate) id: Option<String>,
    pub(crate) user_name: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) profile_image_url: Option<String>,
    pub(crate) is_blue_verified: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct XMention {
    pub(crate) user_name: Option<String>,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct XUrlEntity {
    pub(crate) expanded_url: Option<String>,
    pub(crate) display_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct XMedia {
    pub(crate) media_url_https: Option<String>,
    #[serde(rename = "type")]
    pub(crate) media_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct XTrend {
    pub(crate) name: Option<String>,
    pub(crate) url: Option<String>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub(crate) tweet_count: Option<u64>,
    pub(crate) description: Option<String>,
    pub(crate) domain_context: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct XPostsData {
    pub(crate) tweets: Option<Vec<XPost>>,
    pub(crate) next_cursor: Option<String>,
    pub(crate) has_next_page: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct XTrendsData {
    pub(crate) trends: Option<Vec<XTrend>>,
}

pub(crate) fn format_x_post(p: &XPost) -> Value {
    json!({
        "id": p.id,
        "text": p.full_text.as_ref().or(p.text.as_ref()),
        "created_at": p.created_at,
        "author": p.author.as_ref().map(|a| json!({
            "id": a.id,
            "username": a.user_name,
            "name": a.name,
            "profile_image": a.profile_image_url,
            "blue_verified": a.is_blue_verified,
        })),
        "reposts": p.retweet_count,
        "likes": p.favorite_count,
        "replies": p.reply_count,
        "quotes": p.quote_count,
        "views": p.view_count,
        "language": p.lang,
        "is_repost": p.is_retweet,
        "is_quote": p.is_quote,
        "reply_to": p.in_reply_to_status_id,
        "conversation_id": p.conversation_id,
        "hashtags": p.hashtags,
        "mentions": p.mentions.as_ref().map(|m|
            m.iter().map(|mention| json!({
                "username": mention.user_name,
                "name": mention.name,
            })).collect::<Vec<_>>()
        ),
        "urls": p.urls.as_ref().map(|u|
            u.iter().map(|url| json!({
                "url": url.expanded_url,
                "display": url.display_url,
            })).collect::<Vec<_>>()
        ),
        "media": p.media.as_ref().map(|m|
            m.iter().map(|media| json!({
                "url": media.media_url_https,
                "type": media.media_type,
            })).collect::<Vec<_>>()
        ),
    })
}

// Custom deserializers for flexible X API responses
pub(crate) fn de_opt_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(Value::Bool(b)) => Some(b.to_string()),
        Some(other) => Some(other.to_string()),
    })
}

pub(crate) fn de_opt_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(match value {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => n.as_u64().or_else(|| {
            n.as_i64()
                .and_then(|v| if v >= 0 { Some(v as u64) } else { None })
        }),
        Some(Value::String(s)) => s.parse::<u64>().ok(),
        Some(Value::Bool(b)) => Some(if b { 1 } else { 0 }),
        _ => None,
    })
}

// ############################################################################
//                           NEYNAR CLIENT (Farcaster)
// ############################################################################

pub(crate) const NEYNAR_API_BASE: &str = "https://api.neynar.com/v2";

#[derive(Clone)]
pub(crate) struct NeynarClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_key: String,
}

impl NeynarClient {
    pub(crate) fn new() -> Result<Self, String> {
        let api_key = std::env::var("NEYNAR_API_KEY")
            .map_err(|_| "NEYNAR_API_KEY environment variable not set".to_string())?;
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        Ok(Self { http, api_key })
    }

    pub(crate) fn search_casts(
        &self,
        query: &str,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> Result<NeynarCastsResponse, String> {
        let url = format!("{}/farcaster/cast/search", NEYNAR_API_BASE);
        let limit_val = limit.unwrap_or(25).to_string();

        let mut params: Vec<(&str, &str)> = vec![("q", query), ("limit", &limit_val)];
        if let Some(c) = cursor {
            params.push(("cursor", c));
        }

        let resp = self
            .http
            .get(&url)
            .header("x-api-key", &self.api_key)
            .query(&params)
            .send()
            .map_err(|e| format!("Neynar search failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("Neynar search error {status}: {text}"));
        }

        let api_resp: NeynarSearchCastsApiResponse = resp
            .json()
            .map_err(|e| format!("Neynar decode failed: {e}"))?;
        Ok(NeynarCastsResponse {
            casts: api_resp.result.casts,
            cursor: api_resp.result.next.map(|n| n.cursor),
        })
    }

    pub(crate) fn get_user_by_username(&self, username: &str) -> Result<FarcasterUser, String> {
        let url = format!("{}/farcaster/user/by_username", NEYNAR_API_BASE);

        let resp = self
            .http
            .get(&url)
            .header("x-api-key", &self.api_key)
            .query(&[("username", username)])
            .send()
            .map_err(|e| format!("Neynar get user failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!(
                "Neynar get user '{username}' error {status}: {text}"
            ));
        }

        let api_resp: NeynarUserApiResponse = resp
            .json()
            .map_err(|e| format!("Neynar decode failed: {e}"))?;
        Ok(api_resp.user)
    }

    pub(crate) fn get_user_by_fid(&self, fid: u64) -> Result<FarcasterUser, String> {
        let url = format!("{}/farcaster/user/bulk", NEYNAR_API_BASE);

        let resp = self
            .http
            .get(&url)
            .header("x-api-key", &self.api_key)
            .query(&[("fids", fid.to_string())])
            .send()
            .map_err(|e| format!("Neynar get user by fid failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("Neynar get user fid={fid} error {status}: {text}"));
        }

        let api_resp: NeynarBulkUsersApiResponse = resp
            .json()
            .map_err(|e| format!("Neynar decode failed: {e}"))?;
        api_resp
            .users
            .into_iter()
            .next()
            .ok_or_else(|| "User not found".to_string())
    }

    pub(crate) fn get_channel(&self, channel_id: &str) -> Result<FarcasterChannel, String> {
        let url = format!("{}/farcaster/channel", NEYNAR_API_BASE);

        let resp = self
            .http
            .get(&url)
            .header("x-api-key", &self.api_key)
            .query(&[("id", channel_id)])
            .send()
            .map_err(|e| format!("Neynar get channel failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!(
                "Neynar get channel '{channel_id}' error {status}: {text}"
            ));
        }

        let api_resp: NeynarChannelApiResponse = resp
            .json()
            .map_err(|e| format!("Neynar decode failed: {e}"))?;
        Ok(api_resp.channel)
    }

    pub(crate) fn get_trending_channels(
        &self,
        limit: Option<u32>,
    ) -> Result<Vec<FarcasterChannel>, String> {
        let url = format!("{}/farcaster/channel/trending", NEYNAR_API_BASE);
        let limit_val = limit.unwrap_or(10).to_string();

        let resp = self
            .http
            .get(&url)
            .header("x-api-key", &self.api_key)
            .query(&[("limit", &limit_val)])
            .send()
            .map_err(|e| format!("Neynar trending channels failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("Neynar trending channels error {status}: {text}"));
        }

        let api_resp: NeynarTrendingChannelsApiResponse = resp
            .json()
            .map_err(|e| format!("Neynar decode failed: {e}"))?;
        Ok(api_resp.channels)
    }

    pub(crate) fn get_channel_feed(
        &self,
        channel_id: &str,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> Result<NeynarCastsResponse, String> {
        let url = format!("{}/farcaster/feed/channels", NEYNAR_API_BASE);
        let limit_val = limit.unwrap_or(25).to_string();

        let mut params: Vec<(&str, &str)> =
            vec![("channel_ids", channel_id), ("limit", &limit_val)];
        if let Some(c) = cursor {
            params.push(("cursor", c));
        }

        let resp = self
            .http
            .get(&url)
            .header("x-api-key", &self.api_key)
            .query(&params)
            .send()
            .map_err(|e| format!("Neynar channel feed failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("Neynar channel feed error {status}: {text}"));
        }

        let api_resp: NeynarChannelFeedApiResponse = resp
            .json()
            .map_err(|e| format!("Neynar decode failed: {e}"))?;
        Ok(NeynarCastsResponse {
            casts: api_resp.casts,
            cursor: api_resp.next.map(|n| n.cursor),
        })
    }
}

// ============================================================================
// Neynar API Response types
// ============================================================================

#[derive(Debug, Deserialize)]
pub(crate) struct NeynarSearchCastsApiResponse {
    pub(crate) result: NeynarSearchCastsResult,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NeynarSearchCastsResult {
    pub(crate) casts: Vec<FarcasterCast>,
    pub(crate) next: Option<NeynarNextCursor>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NeynarNextCursor {
    pub(crate) cursor: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NeynarUserApiResponse {
    pub(crate) user: FarcasterUser,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NeynarBulkUsersApiResponse {
    pub(crate) users: Vec<FarcasterUser>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NeynarChannelApiResponse {
    pub(crate) channel: FarcasterChannel,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NeynarTrendingChannelsApiResponse {
    pub(crate) channels: Vec<FarcasterChannel>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NeynarChannelFeedApiResponse {
    pub(crate) casts: Vec<FarcasterCast>,
    pub(crate) next: Option<NeynarNextCursor>,
}

// ============================================================================
// Farcaster Data Models
// ============================================================================

pub(crate) struct NeynarCastsResponse {
    pub(crate) casts: Vec<FarcasterCast>,
    pub(crate) cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FarcasterUser {
    pub(crate) fid: u64,
    pub(crate) username: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) pfp_url: Option<String>,
    #[serde(default)]
    pub(crate) profile: Option<FarcasterUserProfile>,
    pub(crate) follower_count: Option<u64>,
    pub(crate) following_count: Option<u64>,
    pub(crate) verifications: Option<Vec<String>>,
    pub(crate) verified_addresses: Option<FarcasterVerifiedAddresses>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FarcasterUserProfile {
    pub(crate) bio: Option<FarcasterBio>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FarcasterBio {
    pub(crate) text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FarcasterVerifiedAddresses {
    pub(crate) eth_addresses: Option<Vec<String>>,
    pub(crate) sol_addresses: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FarcasterCast {
    pub(crate) hash: String,
    pub(crate) text: Option<String>,
    pub(crate) timestamp: Option<String>,
    pub(crate) author: Option<FarcasterCastAuthor>,
    pub(crate) reactions: Option<FarcasterCastReactions>,
    pub(crate) replies: Option<FarcasterCastReplies>,
    pub(crate) channel: Option<FarcasterCastChannel>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FarcasterCastAuthor {
    pub(crate) fid: u64,
    pub(crate) username: Option<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) pfp_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FarcasterCastReactions {
    pub(crate) likes_count: Option<u64>,
    pub(crate) recasts_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FarcasterCastReplies {
    pub(crate) count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FarcasterCastChannel {
    pub(crate) id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FarcasterChannel {
    pub(crate) id: String,
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) image_url: Option<String>,
    pub(crate) follower_count: Option<u64>,
    pub(crate) lead: Option<FarcasterCastAuthor>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}

pub(crate) fn format_cast(c: &FarcasterCast) -> Value {
    json!({
        "hash": c.hash,
        "text": c.text,
        "timestamp": c.timestamp,
        "author": c.author.as_ref().map(|a| json!({
            "fid": a.fid,
            "username": a.username,
            "display_name": a.display_name,
            "profile_image": a.pfp_url,
        })),
        "likes": c.reactions.as_ref().and_then(|r| r.likes_count),
        "recasts": c.reactions.as_ref().and_then(|r| r.recasts_count),
        "replies": c.replies.as_ref().and_then(|r| r.count),
        "channel": c.channel.as_ref().map(|ch| json!({
            "id": ch.id,
            "name": ch.name,
        })),
    })
}

// ############################################################################
//                         LUNARCRUSH CLIENT
// ############################################################################

pub(crate) const LUNARCRUSH_API_BASE: &str = "https://lunarcrush.com/api4";

#[derive(Clone)]
pub(crate) struct LunarCrushClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_key: String,
}

impl LunarCrushClient {
    pub(crate) fn new() -> Result<Self, String> {
        let api_key = std::env::var("LUNARCRUSH_API_KEY")
            .map_err(|_| "LUNARCRUSH_API_KEY environment variable not set".to_string())?;
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        Ok(Self { http, api_key })
    }

    pub(crate) fn get_trending_topics(&self) -> Result<Vec<LunarCrushTrendingTopic>, String> {
        let url = format!("{}/public/topics/list/v1", LUNARCRUSH_API_BASE);

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .map_err(|e| format!("LunarCrush trending topics failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("LunarCrush trending topics error {status}: {text}"));
        }

        let api_resp: LunarCrushTopicsListResponse = resp
            .json()
            .map_err(|e| format!("LunarCrush decode failed: {e}"))?;
        Ok(api_resp.data)
    }

    pub(crate) fn get_topic_sentiment(
        &self,
        topic: &str,
    ) -> Result<LunarCrushTopicSentiment, String> {
        let topic = topic.to_lowercase().replace(['$', '#'], "");
        let url = format!("{}/public/topic/{}/v1", LUNARCRUSH_API_BASE, topic);

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .map_err(|e| format!("LunarCrush topic sentiment failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!(
                "LunarCrush topic sentiment for '{topic}' error {status}: {text}"
            ));
        }

        let api_resp: LunarCrushTopicSentimentResponse = resp
            .json()
            .map_err(|e| format!("LunarCrush decode failed: {e}"))?;
        Ok(api_resp.data)
    }

    pub(crate) fn get_topic_summary(&self, topic: &str) -> Result<LunarCrushTopicSummary, String> {
        let topic = topic.to_lowercase().replace(['$', '#'], "");
        let url = format!("{}/public/topic/{}/whatsup/v1", LUNARCRUSH_API_BASE, topic);

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .map_err(|e| format!("LunarCrush topic summary failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!(
                "LunarCrush topic summary for '{topic}' error {status}: {text}"
            ));
        }

        let api_resp: LunarCrushTopicSummaryResponse = resp
            .json()
            .map_err(|e| format!("LunarCrush decode failed: {e}"))?;
        Ok(LunarCrushTopicSummary {
            topic: api_resp.config.topic,
            summary: api_resp.summary,
            generated_at: api_resp.config.generated,
        })
    }
}

// ============================================================================
// LunarCrush API Response types
// ============================================================================

#[derive(Debug, Deserialize)]
pub(crate) struct LunarCrushTopicsListResponse {
    pub(crate) data: Vec<LunarCrushTrendingTopic>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LunarCrushTopicSentimentResponse {
    #[allow(dead_code)]
    pub(crate) config: LunarCrushTopicConfig,
    pub(crate) data: LunarCrushTopicSentiment,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LunarCrushTopicConfig {
    #[allow(dead_code)]
    pub(crate) topic: String,
    #[allow(dead_code)]
    pub(crate) generated: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LunarCrushTopicSummaryResponse {
    pub(crate) config: LunarCrushTopicSummaryConfig,
    pub(crate) summary: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LunarCrushTopicSummaryConfig {
    pub(crate) topic: String,
    pub(crate) generated: u64,
}

// ============================================================================
// LunarCrush Data Models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LunarCrushTrendingTopic {
    pub(crate) topic: String,
    pub(crate) title: Option<String>,
    pub(crate) topic_rank: Option<u32>,
    pub(crate) topic_rank_1h_previous: Option<u32>,
    pub(crate) topic_rank_24h_previous: Option<u32>,
    pub(crate) num_contributors: Option<u64>,
    pub(crate) num_posts: Option<u64>,
    pub(crate) interactions_24h: Option<u64>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LunarCrushTopicSentiment {
    pub(crate) topic: String,
    pub(crate) title: Option<String>,
    pub(crate) topic_rank: Option<u32>,
    pub(crate) related_topics: Option<Vec<String>>,
    pub(crate) types_count: Option<HashMap<String, u64>>,
    pub(crate) types_interactions: Option<HashMap<String, u64>>,
    pub(crate) types_sentiment: Option<HashMap<String, u32>>,
    pub(crate) types_sentiment_detail: Option<HashMap<String, LunarCrushSentimentDetail>>,
    pub(crate) interactions_24h: Option<u64>,
    pub(crate) num_contributors: Option<u64>,
    pub(crate) num_posts: Option<u64>,
    pub(crate) categories: Option<Vec<String>>,
    pub(crate) trend: Option<String>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LunarCrushSentimentDetail {
    pub(crate) positive: Option<u64>,
    pub(crate) neutral: Option<u64>,
    pub(crate) negative: Option<u64>,
}

pub(crate) struct LunarCrushTopicSummary {
    pub(crate) topic: String,
    pub(crate) summary: String,
    pub(crate) generated_at: u64,
}

// ############################################################################
//                         X TOOLS (5)
// ############################################################################

// ============================================================================
// Tool 1: GetXUser
// ============================================================================

pub(crate) struct GetXUser;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetXUserArgs {
    /// X username without the @ symbol (e.g., 'elonmusk')
    pub(crate) username: String,
}
