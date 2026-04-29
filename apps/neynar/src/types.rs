use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UserByUsernameQuery<'a> {
    pub(crate) username: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SearchUsersQuery<'a> {
    pub(crate) q: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FeedQuery<'a> {
    #[serde(rename = "feed_type")]
    pub(crate) feed_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fid: Option<u64>,
    pub(crate) limit: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CastLookupQuery<'a> {
    pub(crate) identifier: &'a str,
    #[serde(rename = "type")]
    pub(crate) id_type: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SearchCastsQuery<'a> {
    pub(crate) q: &'a str,
    pub(crate) limit: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PublishCastRequest {
    pub(crate) signer_uuid: String,
    pub(crate) text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) embeds: Option<Vec<UrlEmbed>>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UrlEmbed {
    pub(crate) url: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChannelQuery<'a> {
    pub(crate) id: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TrendingFeedQuery<'a> {
    pub(crate) limit: u32,
    pub(crate) time_window: &'a str,
}
