# aomi-sdk

The guest SDK for Aomi packages. A package is a WebAssembly component for the
`aomi:package@1.0.0` world (contract version 1, WIT in [`wit/`](wit)),
built for `wasm32-wasip2`. The backend runs each call on a fresh sandboxed
instance.

## A package

A package crate is a `cdylib` with an `aomi.toml` beside its `Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
aomi-sdk = "8"
schemars = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```rust
use aomi_sdk::{Ctx, Outcome, Result, Tool, http};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct PriceArgs { coins: Vec<String> }

pub struct Price;                          // exposed as `<package>_price`
impl Tool for Price {
    const NAME: &'static str = "price";
    const DESCRIPTION: &'static str = "Current USD price of coins.";
    type Args = PriceArgs;                 // AUDIENCE / CONTINUATION default to Model / false
    async fn run(args: PriceArgs, _ctx: &Ctx) -> Result<Outcome> {
        let url = format!("https://coins.llama.fi/prices/current/{}", args.coins.join(","));
        let body: serde_json::Value = http::get(url).send().await?.json()?;
        Ok(Outcome::data(body))
    }
}

aomi_sdk::package!(Price);
```

`aomi.toml` declares identity, secret slots and allowed hosts; tools come from
code. See the two examples:

- [`examples/hello-reader`](examples/hello-reader): a keyless reader.
- [`examples/keyed-header`](examples/keyed-header): a builder key the host
  injects as the `x-api-key` header; the package checks `secrets::has` and
  returns the reply's evidence `record()`.

## The surface

| Item | What it does |
|---|---|
| `Tool` | `NAME`, `DESCRIPTION`, `type Args` (schema from schemars), `AUDIENCE`, `CONTINUATION`, `async fn run(args, &Ctx)` |
| `package!(A, B, …)` | exports `describe` and `call-tool`; dispatches by short name and runs the tool to completion |
| `Ctx` | call id, thread id, account id, agent id, wallets, chains, `public`; `ctx.wallet(Family::Evm)`, `ctx.continuation::<T>()` |
| `Outcome::data(json).image(Image::png(bytes).caption(..))` | JSON for the model, up to two images |
| `Outcome::prepared(summary).tx(StageTx::evm(args).evidence(n)).follow_up(tool, note, &data)` | transactions for the core staging lanes (`evm`, `svm_ix`, `svm_tx`) |
| `Outcome::sign(summary, SignRequest::typed_data(..))`<br>`.then(tool, note, &state)` | one signature request and the continuation tool; state at most 16 KiB |
| `err(code, message)`, `Result` | a package error; the model sees `[code] message` |
| `http::{get, post, put, patch, delete}` | `.query/.header/.json/.body`, `.send().await`; `Response::{status, headers, bytes, text, json, record}` |
| `secrets::{has, hmac_sha256, reveal}` | slots from `aomi.toml`; values stay in the host unless `reveal = true` |
| `chain::evm(id)` | `call`, `balance`, `code`, `nonce`, `block_number`, `receipt`, `transaction`, `read`, `.at(View)`; feature `alloy`: `.provider()`, an alloy `RootProvider` over the same reads (a "pending" block argument reads as "latest"; alloy crates re-exported as `aomi_sdk::alloy`) |
| `chain::svm(cluster)` | `account`, `balance`, `token_accounts`, `signatures`, `transaction`, `read` |
| `log::emit(Level, msg)` | to the backend's log, tagged with the package and call |

## Build and test

```sh
cargo build -p hello-reader --target wasm32-wasip2 --release
# target/wasm32-wasip2/release/hello_reader.wasm is the component
cargo test -p aomi-sdk
```

The crate compiles natively, so tool logic is unit-tested with `cargo test`.
Host imports (`http`, `chain`, `secrets`, `log`) panic outside the sandbox;
`testing::ctx()` builds a call context and `testing::reply(status, headers,
body)` an HTTP reply to decode.

## Generated clients

`aomi-build gen-client` turns an OpenAPI 3.0 spec into a typed client whose
async methods send through `http` and return
`aomi_progenitor_client::ResponseValue<T>`. Read `.record()` on it before
`.into_inner()` when the reply is evidence; `?` converts its error into a
package error. The generator (`progenitor-impl`) and runtime
(`progenitor-client`) are a fork of progenitor 0.14 (MPL-2.0).
