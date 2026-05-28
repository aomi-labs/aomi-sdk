use aomi_ext::hmac_auth::{base64_decode, hmac_sha256_base64, iso_timestamp_ms};

/// Compute the `lmts-signature` value for one Limitless API request.
///
/// `secret_b64` is the raw string from the dashboard (already base64).
/// `path_with_query` MUST include leading `/` and any `?query=...` suffix.
/// `body` is `""` for GET / DELETE without bodies.
pub fn sign(
    secret_b64: &str,
    timestamp: &str,
    method: &str,
    path_with_query: &str,
    body: &str,
) -> Result<String, String> {
    let key = base64_decode(secret_b64)
        .map_err(|e| format!("[gambit] base64 decode secret: {e}"))?;
    let prehash = format!("{timestamp}\n{method}\n{path_with_query}\n{body}");
    Ok(hmac_sha256_base64(&key, prehash.as_bytes()))
}

/// ISO-8601 UTC timestamp with millisecond precision.
pub fn iso_timestamp() -> String {
    iso_timestamp_ms()
}
