# SDK Version Compatibility

The compatibility contract between the host runtime and published plugins is the
exact `aomi-sdk` crate version they were built against.

The plugin exports that value via the `aomi_sdk_version` symbol, includes it in
`DynManifest.sdk_version`, and the host rejects any plugin whose SDK version
does not match the host's own `AOMI_SDK_VERSION`.

## Current Rule

**Exact-match only** — host and plugin must be built with the same
`aomi-sdk` version.

## Why This Is The Gate

- The plugin FFI surface and manifest format live inside `aomi-sdk`.
- Published plugins from this repo are rebuilt as a unit when the SDK changes.
- The hosted runtime already treats SDK drift as a coordinated rebuild event.

This makes the SDK crate version the clearest operational source of truth for
compatibility.

## What Changes `AOMI_SDK_VERSION`

`AOMI_SDK_VERSION` is compiled from `sdk/Cargo.toml` `package.version`.

That means a version bump on `main` changes all of these together:

1. The SDK crate version published to crates.io.
2. The host's compiled `AOMI_SDK_VERSION`.
3. The plugin's exported `aomi_sdk_version`.
4. The plugin manifest's `sdk_version`.

## What Does Not Change It

- App-only release tags like `apps-v0.1.14`
- Plugin implementation changes that do not bump `sdk/Cargo.toml`
- Rebuilding the same SDK version on `publish`

Those change delivery state, not compatibility state.

## Release Checklist

When changing the SDK compatibility contract:

1. Bump `sdk/Cargo.toml`.
2. Merge that SDK bump through `main` so crates.io and host builds align.
3. Rebuild plugins from `publish` so released bundles carry the new SDK version.
4. Run the SDK test suite: `cargo test -p aomi-sdk`.
5. Run plugin validation: `cargo xtask build-aomi --release`.
