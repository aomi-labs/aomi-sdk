# ABI Versioning

The plugin ABI version (`AOMI_ABI_VERSION`) is the contract between compiled plugins and the host runtime. The host checks this version at load time via the `aomi_abi_version` symbol and rejects any plugin that does not match exactly.

## Current Version

**ABI v5** — coordinated release bump for the secret-aware dynamic tool rollout; host and plugins must be rebuilt and released together.

## Version History

| Version | Changes |
|---------|---------|
| v1 | Initial sync-only plugin contract |
| v2 | Async execution primitives: `aomi_async_tool_start`, `aomi_dyn_exec_poll`, `aomi_dyn_exec_cancel` |
| v3 | Namespace declarations in manifest (`namespaces` field) |
| v4 | Explicit namespace contract enforced by the host; SDK supports explicit `[]` and defaults omitted namespaces to `["common"]` for compatibility |
| v5 | Coordinated release bump for secret-aware dynamic tools; forces exact-match rebuilds for published apps and runtime |

## What Bumps the ABI Version

A new ABI version is required when:

- **FFI symbols change** — a symbol is added, removed, or its signature changes.
- **Envelope wire format changes** — the JSON shape of `DynToolStart`, `AsyncExecPool`, `DynManifest`, or `DynExecCancel` gains or loses required fields.
- **Behavioral contracts change** — e.g. the host starts requiring a new lifecycle call.

A new ABI version is **not** required when:

- Adding optional fields with `#[serde(default)]` to existing envelopes.
- Changing tool implementations inside plugins.
- Adding new host-side namespaces (these are additive).

## Compatibility Strategy

The current approach is **exact-match only**: the host rejects any plugin whose `aomi_abi_version()` does not equal the host's `AOMI_ABI_VERSION`. This is intentionally strict because:

1. Plugins are compiled from this repo alongside the host — version drift is rare.
2. Mismatched FFI calls can cause memory corruption, so silent degradation is worse than a clear error.

### Future: Range Negotiation

If the plugin ecosystem grows beyond this monorepo, consider:

- Plugin exports `aomi_abi_version_min()` and `aomi_abi_version_max()` to declare a supported range.
- Host checks `host_version >= plugin_min && host_version <= plugin_max`.
- Backward-compatible additions (new optional fields, new symbols the plugin doesn't call) can be handled without rebuilding all plugins.

This is not yet implemented. Until then, **bumping `AOMI_ABI_VERSION` requires rebuilding all plugins**.

## Release Checklist

When bumping the ABI version:

1. Update `AOMI_ABI_VERSION` in `sdk/src/types.rs`.
2. Update the version history table above.
3. Add a changelog entry describing the breaking change.
4. Rebuild all plugins: `cargo xtask build-aomi --release`.
5. Run the full test suite: `cargo test -p aomi-sdk`.
