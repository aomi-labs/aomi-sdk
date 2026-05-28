# aomi-build

CLI for scaffolding, building, and end-to-end testing Aomi apps.

```
gen-specs ──▶ gen-client ──▶ gen-tool ──▶ curate ──▶ cargo build ──▶ test.json + e2e runner
   (1)          (2)            (3)         (4)          (5)              (6)
```

Each stage is independently runnable; `aomi-build new-app` is the one-shot orchestrator for stages 1–3 + 5.

---

## Pipeline at a glance

```
External docs / OpenAPI
        │
        ▼
(1) gen-specs           ext/specs/<platform>.yaml          — OR — apps/<platform>/openapi.yaml
        │                  (--shared)                              (default, app-local)
        ▼
(2) gen-client          ext/src/<platform>/client.rs       — OR — apps/<platform>/src/client/
        │
        ▼
(3) gen-tool            apps/<platform>/src/{lib.rs, tool.rs, Cargo.toml}    ← mechanical stubs
        │
        ▼
(4) curate              apps/<platform>/src/tool.rs                           ← user-centric tools
        │               apps/<platform>/src/lib.rs (PREAMBLE)
        ▼
(5) cargo build         apps/<platform>/target/debug/lib<platform>.dylib      ← cdylib plugin
        │
        ▼
(6) e2e test            apps/<platform>/test.json + cargo test app_e2e_specs  ← real LLM, real APIs
```

Files marked `--shared` live under `ext/` and are reusable across multiple Aomi apps that wrap the same upstream (e.g. multiple apps wrapping Binance). The default is **app-local** — every artefact lives under `apps/<platform>/`.

---

## Quick start: new app from scratch

```sh
# One-shot orchestrator. Chains gen-specs → gen-client → gen-tool → cargo build.
aomi-build new-app petstore

# With explicit URL when discovery doesn't find the spec:
aomi-build new-app petstore --from-url https://example.com/openapi.json

# Stop after gen-client (skip tool scaffolding):
aomi-build new-app petstore --no-tool

# Treat as shared (lives under ext/, not apps/):
aomi-build new-app binance --shared
```

After `new-app` finishes, the app **compiles** but has mechanical one-tool-per-endpoint stubs. Bring it to "actually useful" with stage (4) — see [Curating the tool layer](#4-curating-the-tool-layer).

---

## CLI surface

```
aomi-build gen-specs    <p>   # discover/fetch OpenAPI → write YAML spec
aomi-build gen-client   <p>   # OpenAPI YAML → progenitor → typed Rust client
aomi-build gen-tool     <p>   # generated client → scaffold app + stub tools
aomi-build new-app      <p>   # orchestrator: all three above + cargo build
aomi-build tighten-spec <p>   # sharpen additionalProperties:true from real samples
aomi-build test-schema  <p>   # schemathesis validation against live API
```

All stage-1/2/3 commands accept `--shared` (default off → app-local). `gen-tool` auto-detects the shared/app-local mode by checking for `apps/<p>/src/client/`; `--shared` forces it.

---

## (1) `gen-specs` — produce an OpenAPI spec

Discovers an OpenAPI 3.x YAML/JSON for the platform from one of:

- `--from-url <URL>` — direct fetch
- `apis.guru` — community OpenAPI directory
- `github` — search repos for `openapi.yaml`/`swagger.json`
- `well-known` — try `/openapi.yaml`, `/swagger.json` etc. on the platform's docs origin
- `postman` — search Postman public API network

Outputs to `apps/<p>/openapi.yaml` (or `ext/specs/<p>.yaml` with `--shared`).

If discovery fails, **draft a spec by hand** using the [`aomi-app-client-api-gen`](../.claude/skills/aomi-app-client-api-gen/SKILL.md) skill — it reads platform docs and produces an OpenAPI 3.0 YAML.

---

## (2) `gen-client` — typed Rust client via progenitor

```
ext/specs/<platform>.yaml             ← source-of-truth (you/skill edit this)
        │
        ▼
   load_and_preprocess()              ← in aomi-build/src/spec_load.rs
   (7 normalisation passes)
        │
        ├──▶  ext/specs/<platform>.preprocessed.yaml   ← debug dump
        │
        ▼
   in-memory openapiv3::OpenAPI       ← passed to progenitor
        │
        ▼
   ext/src/<platform>/client.rs       ← typed Rust client
```

### Source vs. debug dump

- **`<platform>.yaml`** — the **only** file you (or the skill) ever edit. `tighten-spec`, `gen-specs`, the `aomi-app-client-api-gen` skill, and you-by-hand all write to this file.
- **`<platform>.preprocessed.yaml`** — a **byproduct** dumped to disk by `gen-client` after `load_and_preprocess` runs. When progenitor blows up cryptically, `diff` the two to see exactly what the preprocessor rewrote, then debug from there. Don't edit it (overwritten on every `gen-client` run) and don't commit it (it's regenerable).

### The 7 preprocessing passes

Real-world OpenAPI specs frequently violate progenitor's strict expectations. Each pass is a small named patch:

| Pass | Why progenitor needs it |
|---|---|
| `downgrade_to_30` | progenitor only supports OpenAPI 3.0.x — auto-downgrades `openapi: 3.1.x` headers |
| `fill_missing_operation_ids` | progenitor requires `operationId` on every op; synthesizes from `<method>_<path>` if missing |
| `rename_wildcard_content_types` | `'*/*'` content types (Spring/Java artefact) → `application/json` |
| `dedupe_success_responses` | progenitor allows only one response *type* per op; drops typed 4xx/5xx + extra 2xx with bodies |
| `drop_param_name_collisions` | If path `{queryId}` + query `queryID` both snake_case to `query_id`, drops the non-path one |
| `drop_multipart_ops` | progenitor doesn't support multipart bodies; skips those whole operations |
| `collapse_request_body_to_json` / `collapse_response_content_to_json` | progenitor allows one media type per body; keeps JSON, drops XML/CSV/etc. |

Every pass that fires prints a line like `dropped 209 duplicate success response(s)` so you see exactly what changed.

### Workflow

- Edit `neynar.yaml` → run `aomi-build gen-client neynar --shared --force` → `neynar.preprocessed.yaml` updates as a side effect → `client.rs` regenerates from the in-memory preprocessed spec.
- When progenitor errors, `diff neynar.yaml neynar.preprocessed.yaml` to see what the preprocessor did, then either patch `neynar.yaml` to avoid the issue or add a new preprocessing pass to `spec_load.rs` if it's a generic problem.
- The `.preprocessed.yaml` files are gitignored by convention — safe to delete; they come back on next `gen-client`.

---

## (3) `gen-tool` — scaffold the app crate

App-local layout (the default; verified end-to-end on petstore):

```
apps/petstore/
├── openapi.yaml + openapi.meta.json
├── Cargo.toml          # progenitor-client, chrono, reqwest 0.13 directly
└── src/
    ├── lib.rs                         # mod client; mod tool; + dyn_aomi_app!
    ├── client/{mod.rs, client.rs}     # generated
    └── tool.rs                        # use crate::client::Client as GenClient;
```

Shared layout (when `--shared`):

```
ext/src/<platform>/client.rs   ← shared client
apps/<platform>/
├── Cargo.toml                  # depends on aomi-ext::<platform> feature
└── src/
    ├── lib.rs
    └── tool.rs                 # use aomi_ext::<platform>::Client;
```

`gen-tool` writes **one stub per OpenAPI operationId**, names like `dune_getv1_query_queryid_results`. The stubs compile, but they're not user-facing — every `--all` op becomes a tool. Curate them next.

---

## (4) Curating the tool layer

**Run the [`aomi-app-ux-tool-maker`](../.claude/skills/aomi-app-ux-tool-maker/SKILL.md) skill.**

It rewrites `apps/<p>/src/tool.rs` from mechanical stubs into a curated set of 5–12 user-centric tools, plus a domain-specific PREAMBLE in `lib.rs`. The skill:

- Categorises the platform (price oracle / swap / lend / perps / prediction / social).
- Maps user stories to tools — verb-led names (`defillama_get_token_price`, `lifi_quote_swap`).
- Composes endpoints into composite tools when one user goal needs several.
- Drops endpoints that aren't valuable as tools (admin, internal, niche).
- Trims arg surfaces (≤6 args/tool) and computes derivable values in Rust.
- Authors the PREAMBLE with capabilities, conventions, workflow guidance.

Verify with `cd apps/<p> && cargo build`.

---

## (Optional) `tighten-spec` — sharpen response schemas from samples

Many APIs ship specs with `additionalProperties: true` everywhere, which makes the generated client return `serde_json::Map<String, Value>` for every body — usable but typeless. `tighten-spec` infers concrete schemas from real captured JSON in `ext/specs/<p>.samples/`:

```sh
mkdir -p ext/specs/khalani.samples/get_quote
curl ... > ext/specs/khalani.samples/get_quote/sample.json
aomi-build tighten-spec khalani
aomi-build gen-client khalani --shared --force   # regenerate with tighter types
```

---

## (Optional) `test-schema` — schemathesis validation

Validates the spec against the live API to catch schema drift:

```sh
aomi-build test-schema khalani --base-url https://api.hyperstream.dev
```

Auto-detects whether `schemathesis` is installed; otherwise prints install instructions.

---

## (5) `cargo build` — produce the cdylib

```sh
cd apps/<platform> && cargo build
# → apps/<platform>/target/debug/lib<platform>.dylib  (or .so / .dll)
```

The cdylib is what the runtime loads via `DynFnHandle::load`. The `dyn_aomi_app!` macro at the bottom of `src/lib.rs` exports the FFI `aomi_manifest()` and tool-dispatch entrypoints.

---

## (6) End-to-end testing — `test.json` + the runner

Each app has a single canonical e2e spec at:

```
apps/<platform>/test.json
```

The spec describes a real-LLM scenario: wallet seed, user prompts, expected tool calls per turn, optional wallet-callback injection, and a final state assertion. The runner lives in `product-mono` at `aomi/crates/runtime/tests/local-app-e2e.rs` and is invoked by setting `AOMI_E2E_APP_PATH` to the compiled dylib:

```sh
cd apps/khalani && cargo build

cd ~/Code/product-mono/aomi
AOMI_E2E_APP_PATH=~/Code/aomi-apps/apps/khalani/target/debug/libkhalani.dylib \
  cargo test -p aomi-runtime --test local-app-e2e app_e2e_specs -- --nocapture
```

The runner:
1. Walks up from `AOMI_E2E_APP_PATH` looking for `test.json`.
2. Loads the plugin with `AppLoader`, opens a `DefaultSessionState`.
3. Applies `wallet_seed` (if set) to `user_state`.
4. For each `turn`: sends `prompt`, waits for the LLM to settle, checks `expected_tools`, then optionally injects `callback_after`.
5. Evaluates `final_assertion` (state + transcript invariants).
6. **Fail-fast** with a transcript dump on the first failed assertion.

### Spec schema

```json
{
  "user_story": "Plain-English description shown in the test banner",
  "wallet_seed": {                          // optional — most apps need a connected wallet
    "address": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045",
    "chain_id": 1,
    "is_connected": true
  },
  "turns": [
    {
      "prompt": "Swap 100 USDC from Ethereum to ETH on Optimism via X.",
      "expected_tools": {
        "must_call": ["x_quote", "x_build_deposit"]
        // OR: "any_of": ["x_quote", "x_search_tokens"]
      }
    },
    {
      "prompt": "Looks good, ship it.",
      "expected_tools": { "must_call": ["x_build_deposit"] },
      "callback_after": {                   // optional wallet-style callback after this turn
        "type": "wallet:tx_complete",
        "status": "confirmed",
        "transaction_hash": "0xdeadbeef...0001",
        "tx_hashes": ["0xdeadbeef...0001"],
        "pending_tx_ids": "from_state"      // resolved at injection time from user_state
      }
    }
  ],
  "final_assertion": {
    "user_state": {
      "pending_txs": {
        "min_count": 1,
        "expect_any": [
          { "kind": "swap", "chain_id": 1, "data_starts_with": "0x", "data_min_len": 10 }
        ]
      },
      "pending_eip712s": { "max_count": 0 }
    },
    "tool_responses": { "any_returned_substring": "USDC" },   // optional substring scan
    "no_errors": true,                                         // fail on any tool_call_failed
    "max_turns": 30                                            // cap on assistant tool turns
  }
}
```

### Required env

| Var | Purpose |
|---|---|
| `AOMI_E2E_APP_PATH` | Absolute path to the compiled dylib (or a manifest bundle directory) |
| `ANTHROPIC_API_KEY` | Provider key for the real LLM call |

Both are honored by the existing test conventions. When unset, the test skips cleanly.

### Optional env

| Var | Purpose |
|---|---|
| `AOMI_E2E_SPEC` | Override `test.json` discovery and run a single explicit spec file |

### Authoring `test.json`

Use the [`aomi-app-e2e-tester`](../.claude/skills/aomi-app-e2e-tester/SKILL.md) skill — it expands a seed `user_story` (typically written by `aomi-app-ux-tool-maker`) into a complete spec and runs it.

### Limitations of the v1 spec format

1. **`expected_tools` matches by topic, which equals the tool name only for app-defined tools.** Host tools (`stage_tx`, `simulate_batch`, `commit_txs`) carry an LLM-set `topic` arg, so listing them in `must_call` won't work — the runtime fires those internally as part of routed enforcement anyway.
2. **`callback_after` consumes pending_txs.** A terminal `wallet:tx_complete` callback discards `pending_txs`, so `final_assertion.user_state.pending_txs.min_count: 1` after a callback fires will always fail. For wallet-bridge scenarios use `max_count: 0` (assert the callback consumed them) or split into two specs.
3. **No mid-turn topic assertions.** If you want to assert a tool fired *after* a callback, drive the LLM in a subsequent turn and observe via `expected_tools` on that turn's diff.
4. **No regex.** Topic and body matching is substring + exact only.

---

## End-to-end author workflow (skills + CLI together)

```
1. aomi-app-client-api-gen        ─▶  apps/<p>/openapi.yaml          (when no public spec exists)
2. aomi-build gen-specs           ─▶  apps/<p>/openapi.yaml          (when one does)
3. aomi-build gen-client          ─▶  client.rs
4. aomi-build gen-tool            ─▶  app crate + stub tools
5. aomi-app-ux-tool-maker         ─▶  curated tool.rs + PREAMBLE
6. cargo build                    ─▶  lib<p>.dylib
7. aomi-app-e2e-tester            ─▶  apps/<p>/test.json
8. AOMI_E2E_APP_PATH=… cargo test ─▶  pass / transcript-dumped failure
```

Or, in one line for a happy-path platform:

```sh
aomi-build new-app <platform>             # 2 + 3 + 4 + cargo build
# then run the curate + e2e-test skills
```

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `progenitor` panics during `gen-client` | Spec violation not yet covered by a preprocessing pass | `diff <p>.yaml <p>.preprocessed.yaml` to see what the preprocessor did, then either patch the spec or add a pass to `spec_load.rs` |
| `gen-tool` produces a tool with a wildly long name | Spec's `operationId` is path-derived | Pre-rewrite the operationId in the spec, or curate the name in stage (4) |
| `cargo build` errors on an app | Args struct mismatch with progenitor's positional API | Look at `client.rs` signature for the method; adjust the `client.method(...)` call in `tool.rs` |
| `test.json` fails on `expected_tools.must_call` | LLM took an unexpected path (introspecting balances, retrying quotes, etc.) | Tighten the prompt; loosen `must_call` to `any_of`; add a wallet seed that won't trigger error-recovery branches |
| `test.json` fails on `pending_txs.min_count` after a callback | Callback consumed pending_txs | Set `max_count: 0` instead, or remove `callback_after` from that scenario |
| Runner says "no test.json discovered" | Dylib path not under `apps/<p>/` | Set `AOMI_E2E_SPEC` to the explicit JSON path |

For deeper runtime issues (routed enforcement not firing, alias binding, namespace mismatches), see the runtime test fixtures and the routed-action plan store in `product-mono/aomi/crates/tools/src/route.rs`.
