use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct XApp;

pub(crate) use crate::tool::*;

// ============================================================================
// Client
// ============================================================================

pub(crate) const API_BASE: &str = "https://api.twitterapi.io";

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
        let url = format!("{API_BASE}{path}");
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

        let api_resp: ApiResponse<T> = resp
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
pub(crate) struct ApiResponse<T> {
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

impl<T> ApiResponse<T> {
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
// Data models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct User {
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
pub(crate) struct Post {
    #[serde(default, deserialize_with = "de_opt_string")]
    pub(crate) id: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) full_text: Option<String>,
    pub(crate) created_at: Option<String>,
    pub(crate) author: Option<PostAuthor>,
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
    pub(crate) mentions: Option<Vec<Mention>>,
    pub(crate) urls: Option<Vec<UrlEntity>>,
    pub(crate) media: Option<Vec<Media>>,
    #[serde(flatten)]
    pub(crate) _extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PostAuthor {
    #[serde(default, deserialize_with = "de_opt_string")]
    pub(crate) id: Option<String>,
    pub(crate) user_name: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) profile_image_url: Option<String>,
    pub(crate) is_blue_verified: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Mention {
    pub(crate) user_name: Option<String>,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UrlEntity {
    pub(crate) expanded_url: Option<String>,
    pub(crate) display_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Media {
    pub(crate) media_url_https: Option<String>,
    #[serde(rename = "type")]
    pub(crate) media_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Trend {
    pub(crate) name: Option<String>,
    pub(crate) url: Option<String>,
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub(crate) tweet_count: Option<u64>,
    pub(crate) description: Option<String>,
    pub(crate) domain_context: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PostsData {
    pub(crate) tweets: Option<Vec<Post>>,
    pub(crate) next_cursor: Option<String>,
    pub(crate) has_next_page: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TrendsData {
    pub(crate) trends: Option<Vec<Trend>>,
}

pub(crate) fn format_post(p: &Post) -> Value {
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

// Custom deserializers for flexible API responses
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

// ============================================================================
// Tool 1: GetXUser
// ============================================================================

pub(crate) struct GetXUser;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetXUserArgs {
    /// X username without the @ symbol (e.g., 'elonmusk')
    pub(crate) username: String,
}
