# Aomi SDK

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![CI](https://github.com/aomi-labs/aomi-sdk/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/aomi-labs/aomi-sdk/actions/workflows/ci.yml)

The guest SDK for Aomi packages and the `aomi-build` CLI. A package is a
`wasm32-wasip2` component that implements the `aomi:package@1.0.0` world; the
Aomi runtime loads it, calls its tools and enforces the capabilities its
`aomi.toml` declares. Start with [`sdk/README.md`](./sdk/README.md).

## What lives here

- `sdk`: the `aomi-sdk` crate (the `Tool` trait, `package!`, `http`, `secrets`, `testing`) and its WIT in `sdk/wit`
- `sdk/bin/build`: the `aomi-build` CLI, which scaffolds packages (`init`, `new-package`), generates clients from OpenAPI specs and deploys. See [`docs/aomi-build.md`](./docs/aomi-build.md)
- `sdk/examples/hello-reader`, `sdk/examples/keyed-header`: example packages
- `progenitor-impl`, `progenitor-client`: the OpenAPI client generator and its runtime
- `docs/`: [`repo-structure.md`](./docs/repo-structure.md) (package layout), [`host-interop.md`](./docs/host-interop.md) (host capabilities), deployment and auth guides

The runtime and loader are not in this repository.

## Build a package

```bash
cargo install aomi-sdk --features cli
aomi-build init my-package        # packages/my-package: Cargo.toml, aomi.toml, src/lib.rs
cargo build -p my-package --target wasm32-wasip2 --release
```

Test the tool logic natively with `aomi_sdk::testing`; the examples show the
pattern. Packages are published to Aomi from a source repository with
`aomi-build deploy` ([guide](./docs/community-deployment.md)).
