# Repo Structure

A package is one crate with a manifest, built as a `wasm32-wasip2` component:

```text
packages/my-package/
├─ Cargo.toml     crate-type = ["cdylib"], depends on aomi-sdk
├─ aomi.toml      identity, network hosts, secrets, capabilities, limits
├─ icon.svg       Library icon (optional)
└─ src/lib.rs     Tool impls and aomi_sdk::package!(..)
```

- `Tool` impls hold typed args (`JsonSchema`, documented fields) and the behaviour.
- A generated client from `aomi-build gen-client` lives in `src/client/` or a shared `crates/clients/<name>`.
- `icon.svg` is optional. It is an SVG of at most 32 KB with a square `viewBox`, drawn only
  in `currentColor` (every `fill`, `stroke`, `color` and `stop-color` is `currentColor` or
  `none`), with no `<script>`, event attributes, `<foreignObject>`, `<image>`, `<style>`,
  processing instructions, DOCTYPE or ENTITY, `href` or `url()` outside the file, or CSS
  escapes. The Library draws it as a mask in the accent colour; without one it shows the
  generic package icon. `aomi-build check-icon packages/my-package` applies the host's rules.
- `aomi.toml` is the only source of the package's capabilities; code does not widen it.

Guidelines: one package per external product; normalize upstream errors into short
actionable `err(code, message)` values; depend only on the host functions in
`host-interop.md`. The full author guide is [`sdk/README.md`](../sdk/README.md).
