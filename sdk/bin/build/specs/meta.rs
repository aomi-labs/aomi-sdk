use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::SpecHit;

#[derive(Debug, Serialize, Deserialize)]
pub struct SpecMeta {
    pub platform: String,
    pub source: String,
    pub source_url: String,
    pub fetched_at: DateTime<Utc>,
    pub upstream_version: Option<String>,
}

impl SpecMeta {
    pub fn from_hit(platform: &str, hit: &SpecHit) -> Self {
        Self {
            platform: platform.to_string(),
            source: hit.source_kind.to_string(),
            source_url: hit.source_url.clone(),
            fetched_at: Utc::now(),
            upstream_version: hit.version.clone(),
        }
    }
}
