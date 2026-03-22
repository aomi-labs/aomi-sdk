# aomi-apps

Open-source app layer for the Aomi ecosystem.

This repository contains public dynamic app crates, the public SDK they build against, and a small build toolchain for compiling plugins. It intentionally excludes:

- the runtime / loader implementation
- admin and database-facing apps
- oversized internal apps like `l2beat`
- proprietary infrastructure, internal namespaces, and private deployment wiring

## What Lives Here

- `apps/*`: public app crates that compile to dynamic plugins
- `sdk`: the public plugin SDK used by those apps
- `xtask`: helper commands for building and validating plugins in this repo
- `sdk/examples/app-template-http`: reference app showing the recommended file layout for a new plugin
- `docs/host-interop.md`: the public host capability contract used by execution-oriented apps
- `docs/repo-structure.md`: how to structure a new app crate in this repo

## Included Apps

- `defi`
- `delta`
- `khalani`
- `molinar`
- `para`
- `polymarket`
- `prediction`
- `social`
- `x`

## Public Boundary

Apps in this repository may depend on:

- `sdk`
- public HTTP APIs
- environment variables for third-party API keys
- documented host interoperability conventions

Apps in this repository must not depend on:

- internal databases
- private control planes
- internal-only namespaces like `database`
- hidden fallback infrastructure

## Quick Start

1. Copy `sdk/examples/app-template-http` or an existing `apps/*` crate.
2. Keep the standard file split:
   - `src/lib.rs`: app manifest + preamble
   - `src/client.rs`: HTTP client + models
   - `src/tool.rs`: tool implementations
3. If your app needs wallet execution or signing, use the public host conventions from `docs/host-interop.md`.

## Build Plugins

Build every app plugin into `plugins/` with:

```bash
cargo run -p xtask -- build-aomi
```

Useful flags:

```bash
cargo run -p xtask -- build-aomi --app x
cargo run -p xtask -- build-aomi --release
cargo run -p xtask -- build-aomi --target aarch64-apple-darwin
```

## SDK and Examples

The SDK is vendored in `sdk`, including its tests and `examples/hello-app`, so this repository compiles without reaching back into `product-mono`.
