use eyre::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;

use super::{SpecFormat, SpecHit, fetch_text};

const LIST_URL: &str = "https://api.apis.guru/v2/list.json";

/// APIs.guru list endpoint shape: keys are API IDs (e.g. "binance.com"),
/// values describe versions and where to fetch each spec from.
#[derive(Debug, Deserialize)]
struct Entry {
    versions: BTreeMap<String, Version>,
    #[serde(default)]
    preferred: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Version {
    swagger_url: Option<String>,
    swagger_yaml_url: Option<String>,
}

pub fn find(platform: &str) -> Result<Option<SpecHit>> {
    let list_text = fetch_text(LIST_URL).context("apis-guru list fetch failed")?;
    let list: BTreeMap<String, Entry> =
        serde_json::from_str(&list_text).context("apis-guru list is not valid JSON")?;

    let needle = platform.to_lowercase();
    let candidates: Vec<&String> = list.keys().filter(|id| matches(id, &needle)).collect();

    let id = match candidates.as_slice() {
        [] => return Ok(None),
        [single] => (*single).clone(),
        many => pick_best(many, &needle),
    };

    let entry = &list[&id];
    let version = entry
        .preferred
        .clone()
        .or_else(|| entry.versions.keys().last().cloned());
    let Some(ver_key) = version.as_ref() else {
        return Ok(None);
    };
    let v = &entry.versions[ver_key];

    let (url, format) = match (&v.swagger_yaml_url, &v.swagger_url) {
        (Some(yaml), _) => (yaml.clone(), SpecFormat::Yaml),
        (None, Some(json)) => (json.clone(), SpecFormat::Json),
        (None, None) => return Ok(None),
    };

    let body = fetch_text(&url).context("apis-guru spec fetch failed")?;
    Ok(Some(SpecHit {
        body,
        format,
        source_url: url,
        source_kind: "apis-guru",
        version: Some(ver_key.clone()),
    }))
}

fn matches(api_id: &str, needle: &str) -> bool {
    // Strict: only accept the platform name as the full ID, or as the
    // domain-name prefix before a TLD/path. Substring matching has too many
    // false positives (e.g. "x" matched "box.com").
    let id = api_id.to_lowercase();
    if id == needle {
        return true;
    }
    for tld in [".com", ".io", ".net", ".org", ".xyz", ".dev", ".app", ".fi"] {
        let with_tld = format!("{needle}{tld}");
        if id == with_tld || id.starts_with(&format!("{with_tld}:")) {
            return true;
        }
    }
    false
}

/// Prefer ID equal to the needle, then `<needle>.com`, then shortest.
fn pick_best(candidates: &[&String], needle: &str) -> String {
    let dot_com = format!("{needle}.com");
    if let Some(exact) = candidates.iter().find(|c| c.to_lowercase() == needle) {
        return (*exact).clone();
    }
    if let Some(c) = candidates.iter().find(|c| c.to_lowercase() == dot_com) {
        return (*c).clone();
    }
    let mut sorted = candidates.to_vec();
    sorted.sort_by_key(|c| c.len());
    sorted[0].clone()
}
