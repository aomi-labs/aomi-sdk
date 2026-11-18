# Repo Structure

A package is one crate with a manifest, built as a `wasm32-wasip2` component:

```text
packages/my-package/
├─ Cargo.toml     crate-type = ["cdylib"], depends on aomi-sdk
├─ aomi.toml      identity, network hosts, secrets, capabilities, limits
├─ icon.svg       Library icon
└─ src/lib.rs     Tool impls and aomi_sdk::package!(..)
```

- `Tool` impls hold typed args (`JsonSchema`, documented fields) and the behaviour.
- A generated client from `aomi-build gen-client` lives in `src/client/` or a shared `crates/clients/<name>`.
- `icon.svg` is a monochrome SVG drawn in `currentColor`, at most 32 KB, with no scripts,
  event attributes, `<image>`, `<foreignObject>`, external `href`s or DTDs.
- `aomi.toml` is the only source of the package's capabilities; code does not widen it.

Guidelines: one package per external product; normalize upstream errors into short
actionable `err(code, message)` values; depend only on the host functions in
`host-interop.md`. The full author guide is [`sdk/README.md`](../sdk/README.md).
