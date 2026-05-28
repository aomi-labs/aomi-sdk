use eyre::Result;

use super::{SpecHit, detect_format, fetch_text};

/// Standard paths a lot of modern APIs publish their OpenAPI spec at.
/// We append each to a small set of host guesses derived from the platform name.
const PATHS: &[&str] = &[
    "/openapi.json",
    "/openapi.yaml",
    "/api/openapi.json",
    "/api/openapi.yaml",
    "/swagger.json",
    "/swagger.yaml",
    "/v1/openapi.json",
    "/v2/openapi.json",
    "/api-docs",
];

/// Probe the well-known locations for a discoverable spec. Tries `--from-url`
/// first (when supplied via cascade caller), otherwise builds host guesses
/// from the platform name.
pub fn find(platform: &str, from_url: Option<&str>) -> Result<Option<SpecHit>> {
    if let Some(url) = from_url {
        return try_url(url);
    }

    let hosts = host_guesses(platform);
    for host in hosts {
        for path in PATHS {
            let url = format!("https://{host}{path}");
            if let Some(hit) = try_url(&url)? {
                return Ok(Some(hit));
            }
        }
    }
    Ok(None)
}

fn host_guesses(platform: &str) -> Vec<String> {
    let p = platform.to_lowercase();
    vec![
        format!("api.{p}.com"),
        format!("{p}.com"),
        format!("api.{p}.xyz"),
        format!("{p}.xyz"),
        format!("api.{p}.io"),
        format!("{p}.io"),
    ]
}

fn try_url(url: &str) -> Result<Option<SpecHit>> {
    match fetch_text(url) {
        Ok(body) => {
            // Sanity check: must look like an OpenAPI/Swagger doc.
            if !looks_like_spec(&body) {
                return Ok(None);
            }
            let format = detect_format(&body);
            Ok(Some(SpecHit {
                body,
                format,
                source_url: url.to_string(),
                source_kind: "well-known",
                version: None,
            }))
        }
        Err(_) => Ok(None),
    }
}

fn looks_like_spec(body: &str) -> bool {
    // Some specs put the `openapi` key after big `components`/`paths` blocks,
    // so scan the whole body rather than just the first window.
    let lower = body.to_lowercase();
    lower.contains("\"openapi\"")
        || lower.contains("openapi:")
        || lower.contains("\"swagger\"")
        || lower.contains("swagger:")
}
