//! Secret slots declared in `aomi.toml`. Values stay in the host unless a
//! builder slot says `reveal = true`; header and query slots are injected by
//! the host into requests to the slot's hosts.

use crate::Result;
use crate::bindings::aomi::package::secrets as host;

/// True if the slot is declared and has a value for this call.
pub fn has(slot: &str) -> bool {
    host::has(slot)
}

/// HMAC-SHA256 with the slot's value over `message`; the 32-byte MAC.
/// The slot must declare `hmac = true`.
pub fn hmac_sha256(slot: &str, message: &[u8]) -> Result<Vec<u8>> {
    Ok(host::hmac_sha256(slot, message)?)
}

/// The slot's value. Only builder slots with `reveal = true`.
pub fn reveal(slot: &str) -> Result<String> {
    Ok(host::reveal(slot)?)
}
