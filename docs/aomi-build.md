# aomi-build

CLI for scaffolding Aomi packages and generating API clients and tools.

```
gen-specs ──▶ gen-client ──▶ gen-tool ──▶ curate ──▶ cargo build
   (1)          (2)            (3)         (4)          (5)
```

Each stage is independently runnable; `aomi-build new-package` is the one-shot orchestrator for stages 1–3 + 5.

---

## Pipeline at a glance

```
External docs / OpenAPI
        │
        ▼
(1) gen-specs           crates/clients/<platform>/openapi.yaml   — OR — packages/<platform>/openapi.yaml
        │                  (--shared)                                    (default, package-local)
        ▼
(2) gen-client          crates/clients/<platform>/src/            — OR — packages/<platform>/src/client/
        │
        ▼
(3) gen-tool            packages/<platform>/src/{lib.rs, tool.rs, Cargo.toml}    ← mechanical stubs
        │
        ▼
(4) curate              packages/<platform>/src/tool.rs                           ← user-centric tools
        │               packages/<platform>/src/lib.rs (PREAMBLE)
        ▼
(5) cargo build         packages/<platform>/ --target wasm32-wasip2           ← package component
```

Files marked `--shared` live in a client crate under `crates/clients/` and are reusable across several packages that wrap the same upstream (e.g. several packages wrapping Binance). The default is **package-local** — every artefact lives under `packages/<platform>/`.

---

## Quick start: new package from scratch

```sh
# One-shot orchestrator. Chains gen-specs → gen-client → gen-tool → cargo build.
aomi-build new-package petstore

# With explicit URL when discovery doesn't find the spec:
aomi-build new-package petstore --from-url https://example.com/openapi.json

# Stop after gen-client (skip tool scaffolding):
aomi-build new-package petstore --no-tool

# Treat as shared (client crate under crates/clients/):
aomi-build new-package binance --shared
```

After `new-package` finishes, the package **compiles** but has mechanical one-tool-per-endpoint stubs. Bring it to "actually useful" with stage (4) — see [Curating the tool layer](#4-curating-the-tool-layer).

---

## CLI surface

```
aomi-build gen-specs    <p>   # discover/fetch OpenAPI → write YAML spec
aomi-build gen-client   <p>   # OpenAPI YAML → progenitor → typed Rust client
aomi-build gen-tool     <p>   # generated client → scaffold package + stub tools
aomi-build new-package  <p>   # orchestrator: all three above + cargo build
aomi-build tighten-spec <p>   # sharpen additionalProperties:true from real samples
aomi-build test-schema  <p>   # schemathesis validation against live API
aomi-build check-icon [dir]   # a package's [package] icon against the host's icon rule
cargo build --target wasm32-wasip2 --release   # build a package component
```

All stage-1/2/3 commands accept `--shared` (default off → package-local). `gen-tool` auto-detects the shared/package-local mode by checking for `packages/<p>/src/client/`; `--shared` forces it.

---

## (1) `gen-specs` — produce an OpenAPI spec

Discovers an OpenAPI 3.x YAML/JSON for the platform from one of:

- `--from-url <URL>` — direct fetch
- `apis.guru` — community OpenAPI directory
- `github` — search repos for `openapi.yaml`/`swagger.json`
- `well-known` — try `/openapi.yaml`, `/swagger.json` etc. on the platform's docs origin
- `postman` — search Postman public API network

Outputs to `packages/<p>/openapi.yaml` (or `crates/clients/<p>/openapi.yaml` with `--shared`).

If discovery fails, **draft a spec by hand** using the [`aomi-package-client-api-gen`](../.claude/skills/aomi-package-client-api-gen/SKILL.md) skill — it reads platform docs and produces an OpenAPI 3.0 YAML.

---

## (2) `gen-client` — typed Rust client via progenitor

```
packages/<platform>/openapi.yaml      ← source-of-truth (you/skill edit this)
        │
        ▼
   load_and_preprocess()              ← in sdk/bin/build/spec_load
   (7 normalisation passes)
        │
        ├──▶  --preprocessed <path>   ← optional debug dump
        │
        ▼
   in-memory openapiv3::OpenAPI       ← passed to progenitor
        │
        ▼
   packages/<platform>/src/client/    ← typed Rust client
```

### Source vs. debug dump

- **`openapi.yaml`** — the **only** file you (or the skill) ever edit. `tighten-spec`, `gen-specs`, the `aomi-package-client-api-gen` skill, and you-by-hand all write to this file.
- **the preprocessed dump** — a **byproduct** `gen-client --preprocessed <path>` writes after `load_and_preprocess` runs. When progenitor blows up cryptically, `diff` the two to see exactly what the preprocessor rewrote, then debug from there. Don't edit it (overwritten on every `gen-client` run) and don't commit it (it's regenerable).

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

- Edit `openapi.yaml` → run `aomi-build gen-client neynar --force` → the client regenerates from the in-memory preprocessed spec.
- When progenitor errors, rerun with `--preprocessed /tmp/neynar.pre.yaml` and `diff` it against `openapi.yaml` to see what the preprocessor did, then either patch the spec or add a new preprocessing pass to `spec_load` if it's a generic problem.

---

## (3) `gen-tool` — scaffold the package crate

Package-local layout (the default; verified end-to-end on petstore):

```
packages/petstore/
├── openapi.yaml + openapi.meta.json
├── Cargo.toml          # progenitor-client, chrono, reqwest 0.13 directly
└── src/
    ├── lib.rs                         # Tool impls + aomi_sdk::package!
    ├── client/{mod.rs, client.rs}     # generated
    └── tool.rs                        # use crate::client::Client as GenClient;
```

Shared layout (when `--shared`):

```
crates/clients/<platform>/      ← shared client crate `<platform>-client`
packages/<platform>/
├── Cargo.toml                  # depends on <platform>-client
└── src/
    ├── lib.rs
    └── tool.rs                 # uses the shared client crate
```

`gen-tool` writes **one stub per OpenAPI operationId**, names like `dune_getv1_query_queryid_results`. The stubs compile, but they're not user-facing — every `--all` op becomes a tool. Curate them next.

---

## (4) Curating the tool layer

Rewrite the stubs in `packages/<p>/src/tool.rs` by hand into a curated set of 5–12 user-centric tools:

- Map user stories to tools, with verb-led names (`defillama_get_token_price`, `lifi_quote_swap`).
- Compose endpoints into one tool when one user goal needs several.
- Drop endpoints that aren't valuable as tools (admin, internal, niche).
- Trim arg surfaces (≤6 args/tool) and compute derivable values in Rust.
- Keep tool descriptions true to behaviour; they are model-facing.

Verify with `cargo build -p <p> --target wasm32-wasip2` and test the tool logic natively with `aomi_sdk::testing`.

---

## (Optional) `tighten-spec` — sharpen response schemas from samples

Many APIs ship specs with `additionalProperties: true` everywhere, which makes the generated client return `serde_json::Map<String, Value>` for every body — usable but typeless. `tighten-spec` infers concrete schemas from real captured JSON in `packages/<p>/openapi.samples/`:

```sh
mkdir -p packages/khalani/openapi.samples
curl ... > packages/khalani/openapi.samples/getQuote.200.json
aomi-build tighten-spec khalani
aomi-build gen-client khalani --force   # regenerate with tighter types
```

---

## (Optional) `test-schema` — schemathesis validation

Validates the spec against the live API to catch schema drift:

```sh
aomi-build test-schema khalani --base-url https://api.hyperstream.dev
```

Auto-detects whether `schemathesis` is installed; otherwise prints install instructions.

---

## (5) `cargo build`: produce the component

```sh
cargo build -p <platform> --target wasm32-wasip2 --release
# → target/wasm32-wasip2/release/<platform>.wasm
```

The component exports `describe` and `call-tool` through `aomi_sdk::package!`; see [`sdk/README.md`](../sdk/README.md).

---

## End-to-end author workflow (skills + CLI together)

```
1. aomi-package-client-api-gen        ─▶  packages/<p>/openapi.yaml      (when no public spec exists)
2. aomi-build gen-specs           ─▶  packages/<p>/openapi.yaml      (when one does)
3. aomi-build gen-client          ─▶  packages/<p>/src/client/
4. aomi-build gen-tool            ─▶  package crate + stub tools
5. curate by hand                 ─▶  curated tool.rs
6. cargo build                    ─▶  <p>.wasm
```

Or, in one line for a happy-path platform:

```sh
aomi-build new-package <platform>             # 2 + 3 + 4 + cargo build
# then curate the tools
```

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `progenitor` panics during `gen-client` | Spec violation not yet covered by a preprocessing pass | Diff the spec against the `--preprocessed` dump to see what the preprocessor did, then either patch the spec or add a pass to `spec_load.rs` |
| `gen-tool` produces a tool with a wildly long name | Spec's `operationId` is path-derived | Pre-rewrite the operationId in the spec, or curate the name in stage (4) |
| `cargo build` errors on a package | Args struct mismatch with progenitor's positional API | Look at `client.rs` signature for the method; adjust the `client.method(...)` call in `tool.rs` |

For deeper runtime issues (routed enforcement not firing, alias binding, namespace mismatches), see the runtime test fixtures and route handling in `product-mono/aomi/crates/core/src/internal_action.rs` and `product-mono/aomi/crates/runtime/src/pending.rs`.
