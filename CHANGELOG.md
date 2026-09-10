# Changelog

## 5.0.1

### Fixed

- **App skill `content_digest` depended on the compiling workspace's
  `serde_json` features.** `skill::digest` canonicalized a skill's
  `GuardTable` with `serde_json::Value::to_string`, whose object key order
  follows `serde_json/preserve_order`: struct-field order when the feature is
  resolved on (the host workspace pulls it in transitively), sorted order when
  it is off (a typical community plugin workspace). A plugin and the host
  therefore hashed different bytes for the same guard, and
  `validate_app_skills` rejected every guarded skill with
  `content_digest mismatch: manifest says <a>, content hashes to <b>`.
  The digest now hashes a canonical form (compact JSON, object keys sorted
  bytewise at every depth) that is independent of `serde_json` features and of
  struct field order. Guard-less skills never touched a JSON map, so their
  digests are unchanged. `skill::tests::guard_digest_is_pinned` locks both
  values to literals.

### Compatibility

- The host and every plugin must run the **same fixed SDK version**; the
  exact-match `AOMI_SDK_VERSION` gate applies as usual
  (`docs/sdk-version-compatibility.md`).
- Plugins built against **5.0.0 that ship guard tables** must be rebuilt
  against the fixed SDK. Their stored digest was computed by the pre-fix
  code and only happens to match the new canonical form when that build
  resolved `serde_json` *without* `preserve_order`; do not rely on that.
- Plugin workspaces that enabled `serde_json = { features = ["preserve_order"] }`
  purely to work around this mismatch can drop the override once they are on
  the fixed SDK.
