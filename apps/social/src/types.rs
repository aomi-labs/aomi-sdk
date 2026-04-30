use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UserInfoQuery<'a> {
    #[serde(rename = "userName")]
    pub(crate) user_name: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UserLastTweetsQuery<'a> {
    #[serde(rename = "userName")]
    pub(crate) user_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cursor: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AdvancedSearchQuery<'a> {
    pub(crate) query: &'a str,
    #[serde(rename = "queryType")]
    pub(crate) query_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cursor: Option<&'a str>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct TrendsQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) woeid: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) count: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TweetInfoQuery<'a> {
    #[serde(rename = "tweetId")]
    pub(crate) tweet_id: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SearchCastsQuery<'a> {
    pub(crate) q: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cursor: Option<&'a str>,
    pub(crate) limit: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UserByUsernameQuery<'a> {
    pub(crate) username: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BulkUsersQuery {
    pub(crate) fids: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChannelQuery<'a> {
    pub(crate) id: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LimitQuery {
    pub(crate) limit: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChannelFeedQuery<'a> {
    pub(crate) channel_ids: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cursor: Option<&'a str>,
    pub(crate) limit: u32,
}
