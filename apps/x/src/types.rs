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
pub(crate) struct XUserView {
    pub(crate) id: Option<String>,
    pub(crate) username: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) bio: Option<String>,
    pub(crate) location: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) profile_image: Option<String>,
    pub(crate) banner_image: Option<String>,
    pub(crate) followers: Option<u64>,
    pub(crate) following: Option<u64>,
    pub(crate) posts_count: Option<u64>,
    pub(crate) likes_count: Option<u64>,
    pub(crate) listed_count: Option<u64>,
    pub(crate) created_at: Option<String>,
    pub(crate) verified: Option<bool>,
    pub(crate) blue_verified: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct XPostAuthorView {
    pub(crate) id: Option<String>,
    pub(crate) username: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) profile_image: Option<String>,
    pub(crate) blue_verified: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct XPostMentionView {
    pub(crate) username: Option<String>,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct XPostUrlView {
    pub(crate) url: Option<String>,
    pub(crate) display: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct XPostMediaView {
    pub(crate) url: Option<String>,
    #[serde(rename = "type")]
    pub(crate) media_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct XPostView {
    pub(crate) id: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) created_at: Option<String>,
    pub(crate) author: Option<XPostAuthorView>,
    pub(crate) reposts: Option<u64>,
    pub(crate) likes: Option<u64>,
    pub(crate) replies: Option<u64>,
    pub(crate) quotes: Option<u64>,
    pub(crate) views: Option<u64>,
    pub(crate) language: Option<String>,
    pub(crate) is_repost: Option<bool>,
    pub(crate) is_quote: Option<bool>,
    pub(crate) reply_to: Option<String>,
    pub(crate) conversation_id: Option<String>,
    pub(crate) hashtags: Option<Vec<String>>,
    pub(crate) mentions: Option<Vec<XPostMentionView>>,
    pub(crate) urls: Option<Vec<XPostUrlView>>,
    pub(crate) media: Option<Vec<XPostMediaView>>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct XPostsView {
    pub(crate) posts_count: usize,
    pub(crate) posts: Vec<XPostView>,
    pub(crate) cursor: Option<String>,
    pub(crate) has_more: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct XSearchResultsView {
    pub(crate) query: String,
    pub(crate) query_type: String,
    pub(crate) results_count: usize,
    pub(crate) posts: Vec<XPostView>,
    pub(crate) cursor: Option<String>,
    pub(crate) has_more: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct XTrendView {
    pub(crate) name: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) post_count: Option<u64>,
    pub(crate) description: Option<String>,
    pub(crate) category: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct XTrendsView {
    pub(crate) trends_count: usize,
    pub(crate) trends: Vec<XTrendView>,
}
