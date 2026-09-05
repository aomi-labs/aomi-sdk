//! Client layer for the official X API v2 (<https://api.x.com/2>).
//!
//! App-only (OAuth 2.0 bearer) read access: every request carries
//! `Authorization: Bearer <X_API_KEY>`, where the slot holds the app's
//! bearer token from the X developer portal (Keys & Tokens → Bearer Token).
//! The project is billed pay-per-use, so each tool makes the fewest calls
//! that answer the question (a username lookup costs one extra call before a
//! timeline read; nothing else fans out).
//!
//! v2 answers `{ "data": ..., "includes": {...}, "meta": {...} }`. The
//! normalizers below flatten that into stable snake_case posts and users so
//! the model never has to join `includes.users` by hand.

use aomi_sdk::*;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue};
use serde_json::{Map, Value, json};
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct XApp;

pub(crate) const API_BASE: &str = "https://api.x.com/2";
const BASE_URL_ENV: &str = "X_API_URL";
pub(crate) const API_KEY_NAME: &str = "X_API_KEY";

/// Fields requested on every post so engagement and threading are always present.
pub(crate) const TWEET_FIELDS: &str =
    "id,text,created_at,author_id,conversation_id,lang,public_metrics,entities,referenced_tweets";
/// Fields requested on every user object.
pub(crate) const USER_FIELDS: &str = "id,username,name,description,created_at,verified,verified_type,location,url,profile_image_url,public_metrics";

/// Page size ceiling shared by timelines and search (v2 caps recent search at 100).
pub(crate) const MAX_PAGE: u32 = 100;

pub(crate) struct XClient {
    http: reqwest::blocking::Client,
    base_url: String,
}

impl XClient {
    pub(crate) fn from_ctx(ctx: &DynToolCallCtx) -> Result<Self, String> {
        let token = resolve_secret_value(
            ctx,
            None,
            API_KEY_NAME,
            "[x] missing X_API_KEY. Set it to the app's Bearer Token from the X developer portal (Keys & Tokens → App-Only Authentication).",
        )?;
        let mut auth = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|e| format!("[x] invalid X_API_KEY value: {e}"))?;
        auth.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, auth);
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .default_headers(headers)
            .build()
            .map_err(|e| format!("[x] failed to build HTTP client: {e}"))?;
        let base_url = std::env::var(BASE_URL_ENV)
            .ok()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| API_BASE.to_string());
        Ok(Self { http, base_url })
    }

    /// `GET {base}{path}?{query}`, decoded. Non-2xx becomes a short,
    /// actionable error via [`api_error`].
    pub(crate) fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, String> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .get(&url)
            .query(query)
            .send()
            .map_err(|e| format!("[x] request to {path} failed: {e}"))?;
        let status = response.status().as_u16();
        let reset = response
            .headers()
            .get("x-rate-limit-reset")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = response.text().unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(api_error(path, status, &body, reset.as_deref()));
        }
        let value: Value = serde_json::from_str(&body)
            .map_err(|e| format!("[x] {path} returned a non-JSON body: {e}"))?;
        // v2 can return 200 with only `errors` (e.g. a suspended account).
        if value.get("data").is_none()
            && let Some(err) = first_error(&value)
        {
            return Err(format!("[x] {path}: {err}"));
        }
        Ok(value)
    }

    /// Resolve a handle to its numeric id (one call). Timelines are keyed by id.
    pub(crate) fn user_id_for(
        &self,
        username: &str,
    ) -> Result<(String, Map<String, Value>), String> {
        let value = self.get(
            &format!("/users/by/username/{username}"),
            &[("user.fields", USER_FIELDS.to_string())],
        )?;
        let data = value
            .get("data")
            .and_then(Value::as_object)
            .cloned()
            .ok_or_else(|| format!("[x] no X account named @{username}"))?;
        let id = data
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("[x] profile for @{username} has no id"))?
            .to_string();
        Ok((id, data))
    }
}

// ============================================================================
// Errors
// ============================================================================

fn first_error(value: &Value) -> Option<String> {
    let err = value.get("errors")?.as_array()?.first()?;
    let title = err.get("title").and_then(Value::as_str).unwrap_or("error");
    let detail = err
        .get("detail")
        .or_else(|| err.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("");
    Some(if detail.is_empty() {
        title.to_string()
    } else {
        format!("{title}: {detail}")
    })
}

pub(crate) fn api_error(path: &str, status: u16, body: &str, reset_epoch: Option<&str>) -> String {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            first_error(&v).or_else(|| {
                v.get("detail")
                    .or_else(|| v.get("title"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| brief(body));
    let detail = if detail.is_empty() {
        "no detail".to_string()
    } else {
        detail
    };
    match status {
        401 => format!(
            "[x] 401 unauthorized on {path}: {detail}. X_API_KEY must be the app's Bearer Token from the X developer portal."
        ),
        402 | 403 => format!(
            "[x] {status} on {path}: {detail}. The X project may lack credits or access to this endpoint (pay-per-use plan)."
        ),
        404 => format!("[x] {path} not found (404): {detail}"),
        429 => match reset_epoch {
            Some(reset) => format!(
                "[x] rate limited on {path}: {detail}. The window resets at unix time {reset}; wait and retry."
            ),
            None => format!("[x] rate limited on {path}: {detail}. Wait and retry."),
        },
        _ => format!("[x] HTTP {status} on {path}: {detail}"),
    }
}

pub(crate) fn brief(s: &str) -> String {
    const MAX: usize = 240;
    let s = s.trim();
    if s.chars().count() > MAX {
        format!("{}…", s.chars().take(MAX).collect::<String>())
    } else {
        s.to_string()
    }
}

// ============================================================================
// Normalization
// ============================================================================

/// `includes.users` keyed by id, so posts can carry their author's handle.
pub(crate) fn users_by_id(value: &Value) -> Map<String, Value> {
    value
        .get("includes")
        .and_then(|i| i.get("users"))
        .and_then(Value::as_array)
        .map(|users| {
            users
                .iter()
                .filter_map(|u| {
                    u.get("id")
                        .and_then(Value::as_str)
                        .map(|id| (id.to_string(), u.clone()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Flatten one v2 tweet object (plus its resolved author) into a stable shape.
pub(crate) fn normalize_post(tweet: &Value, author: Option<&Value>) -> Value {
    let metrics = tweet.get("public_metrics").cloned().unwrap_or(Value::Null);
    let author_username = author
        .and_then(|a| a.get("username"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let id = tweet.get("id").and_then(Value::as_str).unwrap_or("");
    let url = author_username
        .as_deref()
        .map(|u| format!("https://x.com/{u}/status/{id}"))
        .unwrap_or_else(|| format!("https://x.com/i/status/{id}"));
    let kind = tweet
        .get("referenced_tweets")
        .and_then(Value::as_array)
        .and_then(|r| r.first())
        .and_then(|r| r.get("type"))
        .and_then(Value::as_str)
        .map(|t| match t {
            "retweeted" => "repost",
            "quoted" => "quote",
            "replied_to" => "reply",
            other => other,
        })
        .unwrap_or("post");
    json!({
        "id": id,
        "text": tweet.get("text"),
        "created_at": tweet.get("created_at"),
        "lang": tweet.get("lang"),
        "kind": kind,
        "conversation_id": tweet.get("conversation_id"),
        "author_id": tweet.get("author_id"),
        "author_username": author_username,
        "author_name": author.and_then(|a| a.get("name")),
        "url": url,
        "metrics": {
            "likes": metrics.get("like_count"),
            "reposts": metrics.get("retweet_count"),
            "replies": metrics.get("reply_count"),
            "quotes": metrics.get("quote_count"),
            "bookmarks": metrics.get("bookmark_count"),
            "impressions": metrics.get("impression_count"),
        },
    })
}

/// Flatten a v2 user object.
pub(crate) fn normalize_user(user: &Value) -> Value {
    let m = user.get("public_metrics").cloned().unwrap_or(Value::Null);
    let username = user.get("username").and_then(Value::as_str).unwrap_or("");
    json!({
        "id": user.get("id"),
        "username": username,
        "name": user.get("name"),
        "description": user.get("description"),
        "created_at": user.get("created_at"),
        "verified": user.get("verified"),
        "verified_type": user.get("verified_type"),
        "location": user.get("location"),
        "website": user.get("url"),
        "profile_image_url": user.get("profile_image_url"),
        "url": format!("https://x.com/{username}"),
        "metrics": {
            "followers": m.get("followers_count"),
            "following": m.get("following_count"),
            "posts": m.get("tweet_count"),
            "likes": m.get("like_count"),
            "listed": m.get("listed_count"),
        },
    })
}

/// A page of posts: normalized rows plus the v2 cursor (`meta.next_token`).
pub(crate) fn normalize_page(value: &Value) -> Value {
    let users = users_by_id(value);
    let posts: Vec<Value> = value
        .get("data")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|t| {
                    let author = t
                        .get("author_id")
                        .and_then(Value::as_str)
                        .and_then(|id| users.get(id));
                    normalize_post(t, author)
                })
                .collect()
        })
        .unwrap_or_default();
    let next = value
        .get("meta")
        .and_then(|m| m.get("next_token"))
        .and_then(Value::as_str)
        .map(str::to_string);
    json!({
        "count": posts.len(),
        "posts": posts,
        "has_next_page": next.is_some(),
        "next_cursor": next,
    })
}

/// Clamp a caller page size into `[5, MAX_PAGE]` (v2 rejects fewer than 5).
pub(crate) fn clamp_page(limit: Option<u32>, default: u32) -> u32 {
    limit.unwrap_or(default).clamp(5, MAX_PAGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_joins_authors_and_carries_cursor() {
        let body = json!({
            "data": [{
                "id": "20", "text": "just setting up my twttr", "author_id": "12",
                "created_at": "2006-03-21T20:50:14.000Z",
                "public_metrics": {"like_count": 310653, "retweet_count": 129000, "reply_count": 18009, "quote_count": 7075, "bookmark_count": 21361, "impression_count": 0},
                "referenced_tweets": [{"type": "quoted", "id": "1"}]
            }],
            "includes": {"users": [{"id": "12", "username": "jack", "name": "jack"}]},
            "meta": {"next_token": "abc"}
        });
        let page = normalize_page(&body);
        assert_eq!(page["count"], 1);
        assert_eq!(page["has_next_page"], true);
        assert_eq!(page["next_cursor"], "abc");
        let post = &page["posts"][0];
        assert_eq!(post["author_username"], "jack");
        assert_eq!(post["url"], "https://x.com/jack/status/20");
        assert_eq!(post["kind"], "quote");
        assert_eq!(post["metrics"]["likes"], 310653);
    }

    #[test]
    fn empty_page_has_no_cursor_and_unknown_author_gets_generic_url() {
        let page = normalize_page(&json!({"meta": {"result_count": 0}}));
        assert_eq!(page["count"], 0);
        assert_eq!(page["has_next_page"], false);
        assert!(page["next_cursor"].is_null());
        let post = normalize_post(&json!({"id": "5", "text": "x"}), None);
        assert_eq!(post["url"], "https://x.com/i/status/5");
        assert_eq!(post["kind"], "post");
    }

    #[test]
    fn user_flattens_metrics() {
        let u = normalize_user(&json!({
            "id": "1", "username": "aomi_labs", "name": "Aomi",
            "public_metrics": {"followers_count": 588, "following_count": 99, "tweet_count": 40}
        }));
        assert_eq!(u["metrics"]["followers"], 588);
        assert_eq!(u["url"], "https://x.com/aomi_labs");
    }

    #[test]
    fn errors_are_short_and_name_the_fix() {
        let e = api_error(
            "/users/by/username/x",
            401,
            r#"{"title":"Unauthorized","detail":"Unauthorized","type":"about:blank","status":401}"#,
            None,
        );
        assert!(e.contains("Bearer Token"), "{e}");
        let e = api_error("/tweets/search/recent", 429, "{}", Some("1700000000"));
        assert!(e.contains("1700000000"), "{e}");
        let e = api_error(
            "/tweets/1",
            404,
            r#"{"errors":[{"title":"Not Found Error","detail":"Could not find tweet with id: [1].","value":"1"}]}"#,
            None,
        );
        assert!(e.contains("Could not find tweet"), "{e}");
        assert!(api_error("/x", 500, &"<html>".repeat(200), None).len() < 400);
    }

    #[test]
    fn page_size_clamps_to_v2_bounds() {
        assert_eq!(clamp_page(None, 20), 20);
        assert_eq!(clamp_page(Some(1), 20), 5);
        assert_eq!(clamp_page(Some(500), 20), 100);
    }
}
