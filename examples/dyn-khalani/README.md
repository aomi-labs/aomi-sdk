# dyn-khalani

Self-contained dynamic Khalani plugin example for `aomi-dyn-sdk`.

## What it includes

- Real Khalani HTTP calls (no `aomi-defi` dependency)
- Tool set modeled after the Khalani app:
  - `get_khalani_quote`
  - `build_khalani_order`
  - `submit_khalani_order`
  - `get_khalani_order_status`
  - `get_khalani_orders_by_address`
  - `get_khalani_tokens`
  - `search_khalani_tokens`
  - `get_khalani_chains`

## Env vars

- `KHALANI_API_ENDPOINT` (optional, default: `https://api.hyperstream.dev`)
- `KHALANI_API_KEY` (optional, set if your endpoint requires it)

## Build

```bash
cargo build -p dyn-khalani --release
```

This produces a shared library under `target/release` (`.so` on Linux, `.dylib` on macOS).

## Load (example)

```bash
cargo run -p backend -- --plugins-dir ./target/release --plugin dyn-khalani
```
