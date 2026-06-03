use eyre::{Context, Result};
use inquire::Select;
use serde::Deserialize;

use super::{SpecHit, detect_format, fetch_text, http_client};

const SEARCH_URL: &str = "https://api.github.com/search/code";

#[derive(Debug, Deserialize)]
struct SearchResp {
    items: Vec<Item>,
}

#[derive(Debug, Deserialize)]
struct Item {
    path: String,
    html_url: String,
    repository: Repo,
}

#[derive(Debug, Deserialize)]
struct Repo {
    full_name: String,
    description: Option<String>,
}

pub fn find(platform: &str) -> Result<Option<SpecHit>> {
    let token = std::env::var("GITHUB_TOKEN").ok();
    if token.is_none() {
        // Unauth code search is heavily rate-limited; surface this as a soft skip.
        println!("  github: GITHUB_TOKEN not set, skipping (code search needs auth)");
        return Ok(None);
    }
    let token = token.unwrap();

    let query = format!(
        "{platform} filename:openapi extension:yaml OR filename:openapi extension:json \
         OR filename:swagger extension:yaml OR filename:swagger extension:json"
    );

    let client = http_client()?;
    let resp = client
        .get(SEARCH_URL)
        .query(&[("q", query.as_str()), ("per_page", "10")])
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .context("github code search failed")?;

    let status = resp.status();
    let text = resp.text()?;
    if !status.is_success() {
        eyre::bail!("github search returned {status}: {text}");
    }
    let parsed: SearchResp =
        serde_json::from_str(&text).context("github search response not JSON")?;

    if parsed.items.is_empty() {
        return Ok(None);
    }

    let labels: Vec<String> = parsed
        .items
        .iter()
        .map(|i| {
            let desc = i.repository.description.as_deref().unwrap_or("");
            format!(
                "{} — {} ({})",
                i.repository.full_name,
                i.path,
                truncate(desc, 60)
            )
        })
        .collect();

    let pick = Select::new(
        "github match — pick a candidate (esc to skip):",
        labels.clone(),
    )
    .prompt_skippable()
    .context("github picker failed")?;

    let Some(label) = pick else {
        return Ok(None);
    };
    let idx = labels
        .iter()
        .position(|l| l == &label)
        .expect("picked label exists");
    let item = &parsed.items[idx];

    // html_url is e.g. https://github.com/<org>/<repo>/blob/<sha>/path/to/openapi.yaml
    // Convert to raw.githubusercontent.com.
    let raw = item
        .html_url
        .replacen("github.com", "raw.githubusercontent.com", 1)
        .replacen("/blob/", "/", 1);

    let body = fetch_text(&raw).context("github raw fetch failed")?;
    let format = detect_format(&body);
    Ok(Some(SpecHit {
        body,
        format,
        source_url: raw,
        source_kind: "github",
        version: None,
    }))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
