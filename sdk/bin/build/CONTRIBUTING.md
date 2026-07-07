# Shipping an Aomi app with `aomi-build`

This is the CLI-side guide to authoring and shipping an Aomi app. `aomi-build`
is a **source-bound relay**: it sends deploy/activate *intent* to the Aomi
backend and never holds GitHub tokens, clones the platform repo, or builds
release artifacts itself. The backend fetches your source through the Aomi
GitHub App, stages it under the platform repo, CI builds the release, and
activation loads it on the runtime.

```
author (you)  ->  aomi-build  ->  backend  ->  platform repo  ->  CI  ->  runtime
  aomi.toml +       deploy/         fetch src,     candidate PR,    build    activate
  Rust cdylib       activate        stage, PR      release tag      cdylib   + load
```

`aomi-build` is the Rust twin of the TypeScript [`@aomi-labs/deploy`](https://github.com/aomi-labs/aomi-widget/tree/main/packages/deploy)
client — both speak the same backend contract (`POST /api/platforms/:platform/deploy`,
`POST /api/platforms/:platform/apps/activate`). Use the CLI from a dev machine
or CI; use `@aomi-labs/deploy` from a server (e.g. the portal). For a no-CLI
path, see [One-shot deploy](#one-shot-deploy-no-cli) below.

---

## Install

```bash
cargo install --git https://github.com/aomi-labs/aomi-sdk --features cli aomi-sdk
# binary lands at ~/.cargo/bin/aomi-build
```

## Command map

| Command | What it does |
|---|---|
| `aomi-build init` | Scaffold a bare app skeleton (`aomi.toml`, `Cargo.toml`, `src/lib.rs`). |
| `aomi-build compile` | Build the app as a Rust `cdylib` locally (codesigns on macOS). |
| `aomi-build deploy` | Run the full backend deploy lifecycle: SDK check, preflight, deploy, wait, activate, verify. |
| `aomi-build deploy preflight` | Validate backend/source/app inputs without platform repo writes. |
| `aomi-build deploy run` | Create/update the platform deployment and write `.aomi/deployment.json`. |
| `aomi-build deploy activate` | Activate release tags from `.aomi/deployment.json` and verify runtime load. |
| `aomi-build deploy status` | Show local deploy state + backend deployment/app runtime state. |
| `aomi-build sdk check` | Verify the app pins the SDK version required by the platform. |
| `aomi-build sdk fix` | Update the app's SDK pin when possible. |
| `aomi-build request` | Ask ops (via Discord) for source access or an activation token. |

For one release cycle, `aomi-build status` aliases `aomi-build deploy status`,
`aomi-build activate` aliases `aomi-build deploy activate`, and
`aomi-build deploy --preflight` aliases `aomi-build deploy preflight`.

(The `gen-specs` / `gen-client` / `gen-tool` / `new-app` / `tighten-spec`
commands are for *generating* an app from an API spec — out of scope here.)

---

## 1. Author your app

Your app lives in **its own source repo**, separate from any platform repo:

```
my-cool-app/
|-- aomi.toml
|-- Cargo.toml
|-- .gitignore       (include .aomi/, target/, and Cargo.lock)
`-- src/
    `-- lib.rs       (dyn_aomi_app! registers your tools)
```

```toml
# aomi.toml
[app]
name         = "my-cool-app"        # app slug, staged under apps/<installation>/<repo-key>/
display_name = "My Cool App"
platform     = "community"          # the platform you're shipping to
public       = true

# Optional: which backend tier may load this release. Defaults to ["staging"].
# server_tags = ["staging", "prod"]
```

Pin the SDK to the version the platform's CI expects (see `platform.json` in the
platform repo for `required_sdk_version`):

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
aomi-sdk = "=3.0.2"
```

Do **not** put GitHub tokens or a `git` field in `aomi.toml`. Source access is
bound by `app_source_id`, not by the manifest.

## 2. Connect a source repo

Install the Aomi GitHub App on your source repo. `aomi-build deploy` can use
`--app-source-id`/`AOMI_APP_SOURCE_ID` directly, or `--repo owner/repo` to ask
the backend to resolve or sync the installed source. The backend mints
short-lived GitHub App tokens server-side to read your source and write the
platform repo.

## 3. Build locally

```bash
cargo check && cargo test
aomi-build compile          # produces the cdylib the platform CI will also build
```

## 4. Deploy

```bash
aomi-build deploy \
  --platform community \
  --repo owner/repo \
  --backend https://api.aomi.dev \
  --activation-token <platform-or-app-token>
```

That command is the full lifecycle: SDK check, preflight, backend deploy, wait
for readiness, activate, then verify every selected app is active,
artifact-ready, and loaded. Use `--fix-sdk` when the CLI should update the SDK
pin before continuing.

A developer receiving only the `aomi-build` binary still needs:

- a committed and pushed source repo containing `aomi.toml`
- the Aomi GitHub App installed on that repo
- backend URL and a valid platform/app activation token
- either `--repo owner/repo` or `--app-source-id <id>`

They do not need a GitHub PAT, platform repo write access, database access, or
an admin private key.

Resolution order:

| Input | Resolution |
|---|---|
| backend | `--backend` → `AOMI_BACKEND_URL` → saved config |
| platform | `--platform` → `aomi.toml` → saved config → `community` |
| token | `--activation-token` → `AOMI_APP_ACTIVATION_TOKEN` → saved config |
| source | `--app-source-id` → `AOMI_APP_SOURCE_ID` → `.aomi/deployment.json` → `--repo owner/repo` source sync |
| commit | `--commit` → local `HEAD`; branches are rejected |

Admin-only token minting follows the same flag/env rule:
`AOMI_ADMIN_KEY` -> `--admin-key <pkcs8-pem-or-path>` and `AOMI_ADMIN_KID`
-> `--admin-kid <kid>`.

`deploy` sends:

```json
POST /api/platforms/community/deploy
{
  "app_source_id": 123,
  "source_ref": "<commit-sha>",
  "aomi_toml_paths": ["aomi.toml"],
  "preflight": false
}
```

Use `aomi-build deploy preflight` to validate source commit access, archive
fetch, `aomi.toml` parsing, and backend manifest generation without opening a
platform PR. Use `aomi-build deploy run` when you only want to create/update the
platform deployment and write `.aomi/deployment.json`.

The backend then: fetches the exact source commit archive, stages each app under
`apps/<installation-id>/<repo-key>/<app>/`, writes
`.aomi/deployment.json`, pushes a candidate branch
(`<owner>/<repo>/<installation-id>/<short-commit>`), and opens/updates a platform
PR against `publish`. Platform CI validates the manifest and publishes a release
tagged `apps-<installation-id>-<repo-key>-<app>-<short-commit>`.

> **`<repo-key>`** is a short, stable key the backend derives from your source
> repo (e.g. `r14902bb079`). It namespaces the staged path and release tag so one
> GitHub App installation can host several source repos — or the same app name
> from different repos — without collisions. The CLI never constructs or parses
> it; it round-trips whatever the backend records in `.aomi/deployment.json`.

## 5. Check status

```bash
aomi-build deploy status --backend https://api.aomi.dev        # or --json
```

`deploy status` reads your local `.aomi/deployment.json`, queries the backend
deployment status when available, and checks the platform app endpoint for the
recorded apps. Before activation an app may not be active or loaded yet.

## 6. Activate

Activation is backend-owned; the CLI passes release tags through.

```bash
# By tag (explicit, repeatable):
aomi-build deploy activate \
  --release-tag apps-<installation-id>-<repo-key>-<app>-<short-commit> \
  --platform community \
  --target-tag prod

# Or use the release tags recorded in .aomi/deployment.json:
aomi-build deploy activate
```

`activate` sends:

```json
POST /api/platforms/community/apps/activate
{
  "target": { "kind": "release_tags", "value": ["apps-<installation-id>-<repo-key>-<app>-<short-commit>"] },
  "apps": ["my-cool-app"],
  "target_tags": ["staging"]
}
```

The backend resolves the target, checks CI, fetches the release artifact,
validates SDK version + target + hashes, and loads the app. Confirm with
`aomi-build deploy status` — the app should report `is_active=true`,
`artifact_ready=true`, and `loaded=true`.

`server_tags` is the build's declared scope: ops can **narrow** at activate time
(`staging` only) but cannot **widen** to `prod` unless the source commit declared
it. Promoting to prod means a new commit with the wider `server_tags` and a
fresh deploy.

## Onboarding (no token yet)

```bash
aomi-build request --platform community --email you@example.com --git-account your-gh-handle
# --dry-run to preview the ops message without posting
```

This posts an onboarding request to ops; it never carries a token. Ops issues a
scoped activation token out of band.

---

## One-shot deploy (no CLI)

Not everyone runs the CLI. The **portal** (`https://chat.aomi.dev` →
Settings → **Deploy**) offers a one-click path that does the whole thing in the
browser:

- **One-click (one-shot)** — uses the `aomi-oneshot` GitHub App to **create a
  repo from a template** (`aomi-labs/playground-example`) in your account and
  deploy it for you. Broad grant (can create repositories). This is the path the
  CLI does *not* have — there's no `aomi-build create-from-template`; the backend
  endpoint `POST /api/platforms/:platform/sources/create-from-template` is driven
  by the portal.
- **Fork & customize (bootstrap)** — the browser equivalent of this CLI flow:
  you make your own repo from the template, install the `aomi-build` GitHub App
  on that one repo (narrow grant), and the portal deploys it.

Both portal paths and `aomi-build` converge on the **same backend deploy →
CI → activate → runtime** contract and the same repo-keyed paths/tags. Choose the
CLI when you already have a source repo and want scriptable/CI deploys; choose
one-shot when you want a working agent in your account with zero local setup.

---

## Common errors

| Error | Cause | Fix |
|---|---|---|
| `deploy needs an app source id` | CLI doesn't know which connected source repo to deploy | pass `--repo owner/repo`, pass `--app-source-id <id>`, or set `AOMI_APP_SOURCE_ID` |
| `deploy needs an activation token` | backend deploy needs platform/app authority | pass `--activation-token <token>`, export `AOMI_APP_ACTIVATION_TOKEN`, or `aomi-build request` one |
| `git tree is dirty` | uncommitted files in your source repo | commit, or ignore `.aomi/`, `target/`, `Cargo.lock` |
| `source is bound to platform ...` | source/app row is already bound to a different platform | deploy to the bound platform or ask ops to repair the binding |
| `deployment failed before activation: no CI ran` | backend created no candidate CI run | check the platform PR and backend deploy status |
| `candidate app dir must be apps/<installation-id>/<repo-key>/<app>` | staged path doesn't match the backend contract | redeploy through the backend (don't hand-push the platform repo) |
| `sdk_version mismatch` | `aomi-sdk` dep doesn't match `platform.json` | run `aomi-build deploy --fix-sdk` or pin the exact `required_sdk_version` |
| final verification is not loaded | activation returned but runtime has not loaded the app | retry `aomi-build deploy activate`; if it repeats, inspect backend/runtime logs |
| `... returned 502` | release tarball not built yet, or backend can't reach GitHub | retry after CI finishes |

## Quick reference

| Where | What |
|---|---|
| `https://staging-api.aomi.dev` / `https://api.aomi.dev` | staging / production backend |
| `POST /api/platforms/:platform/deploy` | source fetch, staging, manifest generation |
| `POST /api/platforms/:platform/apps/activate` | artifact resolution + activation |
| platform app endpoint | app verification (`is_active` / `artifact_ready` / `loaded`) |
| `--backend` / `--app-source-id` / `--activation-token` | CLI deploy inputs; env fallbacks are `AOMI_BACKEND_URL` / `AOMI_APP_SOURCE_ID` / `AOMI_APP_ACTIVATION_TOKEN` |
| `.aomi/deployment.json` | the backend's deploy record, kept locally |
