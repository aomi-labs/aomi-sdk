use eyre::Result;

use super::SpecHit;

/// Postman public collections require an API key and a non-trivial conversion
/// step (postman-to-openapi). Stubbed for now — when implemented, this should:
///   1. Search https://api.getpostman.com/collections with the platform name.
///   2. Let the user pick a collection (inquire).
///   3. Run the postman-to-openapi converter (npm shellout or pure-rust port).
///   4. Return the result as a SpecHit.
pub fn find(_platform: &str) -> Result<Option<SpecHit>> {
    println!("  postman: not implemented yet");
    Ok(None)
}
