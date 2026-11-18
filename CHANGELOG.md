# Changelog

## Unreleased

### Added

- `http::{put, patch, delete}` and `testing::reply` for native decoding tests.
- `aomi-progenitor-impl` and `aomi-progenitor-client`: a progenitor 0.14 fork
  whose generated clients send through `aomi_sdk::http`, compile for
  `wasm32-wasip2` and keep each reply's status, headers and record number.
  `aomi-build gen-client` uses them instead of progenitor and reqwest.

### Changed

- `aomi-build tighten-spec` and `test-schema` default to
  `packages/<platform>/openapi.yaml`, like `gen-specs`.

### Removed

- The former integrations and their shared client crate. Packages live in the
  packages repository.

## 8.0.0

### Changed (breaking)

- `aomi-sdk` is the guest SDK for `aomi:package@1.0.0` (contract version 1).
  A package is a `wasm32-wasip2` component, not a native `cdylib`. The WIT is
  in `wit/`.
- One `Tool` trait (`NAME`, `DESCRIPTION`, `Args`, `AUDIENCE`, `CONTINUATION`,
  `async fn run(args, &Ctx) -> Result<Outcome>`) and `package!(..)`, which
  exports `describe` and `call-tool`.
- Tools return `Outcome::data`, `Outcome::prepared` or `Outcome::sign`.
  Routed returns are gone.
- New modules over the host imports: `http` (reqwest-shaped over `wstd`;
  `Response::record()` reads the host's `aomi-record` header), `secrets`,
  `chain`, `log`.

### Removed

- The C ABI, dynamic loading, route forwarding and the development runtime.
- The hosted deployment commands and interactive wizard. The CLI generates
  package code; Project releases publish components.
- The four old examples; `examples/hello-reader` and `examples/keyed-header`
  replace them.
