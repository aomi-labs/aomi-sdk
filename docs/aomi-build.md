# aomi-build

CLI for scaffolding, building, deploying, activating, and testing Aomi apps.

`aomi-build` is the deterministic Rust CLI and lightweight wizard for codegen,
deploy, status, and activate. For app-from-scratch work that needs
Codex/Claude, approvals, resumable workflow state, validation loops, and local
smoke orchestration, use `aomi-workbench` from `../aomi-widget`.

```
gen-specs ──▶ gen-client ──▶ gen-tool ──▶ curate ──▶ cargo build
   (1)          (2)            (3)         (4)          (5)
```

Each stage is independently runnable; `aomi-build new-app` is the one-shot orchestrator for stages 1–3 + 5.

---

## Pipeline at a glance

```
External docs / OpenAPI
        │
        ▼
(1) gen-specs           crates/clients/<platform>/openapi.yaml   — OR — packages/<platform>/openapi.yaml
        │                  (--shared)                                    (default, app-local)
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

Files marked `--shared` live in a client crate under `crates/clients/` and are reusable across several packages that wrap the same upstream (e.g. several packages wrapping Binance). The default is **app-local** — every artefact lives under `packages/<platform>/`.

---

## Quick start: new app from scratch

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
aomi-build gen-tool     <p>   # generated client → scaffold app + stub tools
aomi-build new-package  <p>   # orchestrator: all three above + cargo build
aomi-build tighten-spec <p>   # sharpen additionalProperties:true from real samples
aomi-build test-schema  <p>   # schemathesis validation against live API
cargo build --target wasm32-wasip2 --release   # build a package component
aomi-build deploy             # full hosted deploy lifecycle
aomi-build deploy preflight   # validate source/backend/platform inputs only
aomi-build deploy run         # create/update backend deployment
aomi-build deploy activate    # activate release tags from deployment.json
aomi-build deploy status      # local deployment.json + backend load status
aomi-build request            # legacy ops onboarding request
```

For one release cycle, `aomi-build status` aliases `aomi-build deploy status`,
`aomi-build activate` aliases `aomi-build deploy activate`, and
`aomi-build deploy --preflight` aliases `aomi-build deploy preflight`.

All stage-1/2/3 commands accept `--shared` (default off → app-local). `gen-tool` auto-detects the shared/app-local mode by checking for `packages/<p>/src/client/`; `--shared` forces it.

---

## Hosted deployment

`aomi-build deploy` is the hosted-app command surface. It is a backend relay:
the CLI discovers local git facts, checks the SDK pin, sends a source-bound
request to the platform backend, waits for the backend-owned platform PR/CI
deployment, activates the release tag, verifies the runtime loaded the app, and
writes `.aomi/deployment.json` in the source repository. The CLI does not hold a
GitHub token, clone a platform repo, push branches, mint release tags, or write
manifests. The backend owns those operations through the platform-bound Project.

Community contributors should start with the step-by-step
[community deployment guide](./community-deployment.md). This section is the
lower-level command reference.

Prerequisite inputs can be passed as flags for a single run, or set as env vars
for repeated runs:

| Input | Flag | Env fallback | Used by |
|---|---|---|---|
| Backend URL | `--backend <url>` | `AOMI_BACKEND_URL` | deploy group, project creation, token commands, apps list, sdk check/fix |
| Activation token | `--activation-token <token>` | `AOMI_APP_ACTIVATION_TOKEN` | deploy group, project creation, token list/revoke, apps list |
| Admin signing key | `--admin-key <pkcs8-pem-or-path>` | `AOMI_ADMIN_KEY` | token mint |
| Admin key id | `--admin-kid <kid>` | `AOMI_ADMIN_KID` | token mint |

```sh
aomi-build project create \
  --platform community \
  --repo owner/repo \
  --backend https://api.aomi.dev \
  --activation-token <platform-or-app-token>
git add .aomi/config.json && git commit && git push
aomi-build deploy
```

That is enough for the happy path when the repo is committed, the Aomi GitHub
App is installed, the source is bound or syncable, and the app pins the platform
SDK version. A developer receiving only the `aomi-build` binary still needs:

- a committed and pushed source repo containing `.aomi/config.json` and its referenced manifests
- the Aomi GitHub App installed on that repo
- backend URL and a valid platform/app activation token
- an explicitly created Project for that repository

They do not need a GitHub PAT, platform repo write access, database access, or
an admin private key.

### Lifecycle

```
aomi-build deploy
  ├─ sdk check
  ├─ deploy preflight
  ├─ deploy run
  ├─ wait until deployment status is ready
  ├─ deploy activate
  └─ verify is_active=true, artifact_ready=true, loaded=true
```

Use `aomi-build deploy --fix-sdk` when the CLI should run `sdk fix` before the
backend deploy/activate work.

Resolution order:

| Input | Resolution |
|---|---|
| backend | `--backend` → `AOMI_BACKEND_URL` → saved config |
| token | `--activation-token` → `AOMI_APP_ACTIVATION_TOKEN` → saved config |
| project | repository origin → existing backend Project |
| commit | `--commit` → local `HEAD`; branches are rejected |

### `deploy preflight`

`deploy preflight` validates backend/source/app inputs without platform repo
writes:

```sh
aomi-build deploy preflight --repo owner/repo
```

It resolves the repository's existing Project and sends `preflight: true` to
`POST /api/projects/:project_id/deploy`. If a
required input is missing, the CLI stops with the next command to run instead of
falling into documentation.

### `deploy run`

`deploy run` creates or updates the platform deployment and writes
`.aomi/deployment.json`:

```json
{
  "source_ref": "<commit-sha>",
  "preflight": false
}
```

Successful deploys record the backend deployment id, platform PR/CI state,
release tags, source commit, and local state:

```json
{
  "project_id": 123,
  "id": "...",
  "status": "pr_created",
  "source": {
    "installation_id": 1,
    "repository_id": 2,
    "repository_link": "https://github.com/acme/source-apps",
    "ref": { "kind": "commit", "value": "<sha>" },
    "commit_hash": "<sha>",
    "aomi_toml_paths": ["apps/foo/aomi.toml"]
  },
  "platform": {
    "platform": "community",
    "repository": "aomi-labs/community-apps",
    "platform_branch": "...",
    "deploy_branch": "...",
    "pr_url": "https://github.com/aomi-labs/community-apps/pull/9",
    "apps": [
      {
        "name": "foo",
        "path": "apps/foo",
        "aomi_toml_path": "apps/foo/aomi.toml",
        "release_tag": "apps-..."
      }
    ]
  },
  "state": {
    "deployed": true,
    "ci_passed": false,
    "activated": false
  }
}
```

### `deploy status`

`deploy status` is read-only. It reads `.aomi/deployment.json`, queries the
backend deployment status when available, and checks the platform app endpoint
for the recorded apps. Pass `--json` for machine-readable output.

### `deploy activate`

`deploy activate` waits for readiness if needed, then sends release tags from
`.aomi/deployment.json` to `POST /api/platforms/:platform/apps/activate`:

```sh
aomi-build deploy activate                 # all apps from deployment.json
aomi-build deploy activate foo bar         # named subset
aomi-build deploy activate --release-tag apps-foo-abc1234
```

```json
{
  "target": { "kind": "release_tags", "value": ["apps-..."] },
  "apps": ["foo"],
  "target_tags": ["prod"]
}
```

When app names are provided with `--release-tag`, their count must match the tag
count and the backend verifies each app name matches its release tag. Activation
is considered successful only after final verification shows every selected app
has `is_active=true`, `artifact_ready=true`, and `loaded=true`.

### `request`

`aomi-build request` remains available for legacy ops onboarding. It posts an
activation-details request to the Aomi apps Discord and never includes an
activation token in the payload.

---

## (1) `gen-specs` — produce an OpenAPI spec

Discovers an OpenAPI 3.x YAML/JSON for the platform from one of:

- `--from-url <URL>` — direct fetch
- `apis.guru` — community OpenAPI directory
- `github` — search repos for `openapi.yaml`/`swagger.json`
- `well-known` — try `/openapi.yaml`, `/swagger.json` etc. on the platform's docs origin
- `postman` — search Postman public API network

Outputs to `packages/<p>/openapi.yaml` (or `crates/clients/<p>/openapi.yaml` with `--shared`).

If discovery fails, **draft a spec by hand** using the [`aomi-app-client-api-gen`](../.claude/skills/aomi-app-client-api-gen/SKILL.md) skill — it reads platform docs and produces an OpenAPI 3.0 YAML.

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

- **`openapi.yaml`** — the **only** file you (or the skill) ever edit. `tighten-spec`, `gen-specs`, the `aomi-app-client-api-gen` skill, and you-by-hand all write to this file.
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

App-local layout (the default; verified end-to-end on petstore):

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
1. aomi-app-client-api-gen        ─▶  packages/<p>/openapi.yaml      (when no public spec exists)
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
