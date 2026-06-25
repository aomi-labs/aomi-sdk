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
| `aomi-build deploy` | Send a source-bound deploy request to the backend. |
| `aomi-build status` | Show local deploy state + the backend's runtime registry per app. |
| `aomi-build activate` | Activate a built release by tag / PR / branch. |
| `aomi-build request` | Ask ops (via Discord) for source access or an activation token. |

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
aomi-sdk = "=3.0.0"
```

Do **not** put GitHub tokens or a `git` field in `aomi.toml`. Source access is
bound by `app_source_id`, not by the manifest.

## 2. Connect a source repo

Install the Aomi GitHub App on your source repo and note the resulting
`app_source_id` (from ops or the portal). `aomi-build deploy` sends this id to
the backend; the backend mints short-lived GitHub App tokens server-side to read
your source and write the platform repo.

## 3. Build locally

```bash
cargo check && cargo test
aomi-build compile          # produces the cdylib the platform CI will also build
```

## 4. Deploy

```bash
export AOMI_BACKEND_URL=https://staging-api.aomi.dev
export AOMI_APP_SOURCE_ID=<your-app-source-id>
export AOMI_APP_ACTIVATION_TOKEN=<platform-or-app-token>

aomi-build deploy --platform community --aomi-toml aomi.toml
```

Resolution order for each input: CLI flag (`--backend`, `--app-source-id`,
`--activation-token`) → environment variable → error if missing.

`deploy` sends:

```json
POST /api/platforms/community/deploy
{
  "app_source_id": 123,
  "source_ref": { "kind": "branch", "value": "main" },
  "aomi_toml_paths": ["aomi.toml"],
  "preflight": false
}
```

Run `--preflight` first: with credentials it validates source resolution,
archive fetch, `aomi.toml` parsing, and backend manifest generation without
opening a platform PR; without credentials it just prints the request.

The backend then: resolves the source ref to a commit, fetches the archive,
stages each app under `apps/<installation-id>/<repo-key>/<app>/`, writes
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
aomi-build status --backend https://staging-api.aomi.dev        # or --json
```

`status` merges your local `.aomi/deployment.json` with the backend's **runtime
registry** (`GET /api/control/apps/status`) and reports, per app, whether it is
`active` and `loaded`. Before activation an app shows as not registered/not
loaded — that's expected while CI is still building.

## 6. Activate

Activation is backend-owned; the CLI passes release tags through.

```bash
# By tag (explicit, repeatable):
aomi-build activate \
  --release-tag apps-<installation-id>-<repo-key>-<app>-<short-commit> \
  --platform community \
  --target-tag staging

# Or by PR / branch:
aomi-build activate --pr https://github.com/aomi-labs/community-apps/pull/9 --target-tag staging
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
`aomi-build status` — the app should report `loaded`.

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
| `deploy needs --app-source-id` | CLI doesn't know which connected source repo to deploy | pass `--app-source-id` or set `AOMI_APP_SOURCE_ID` |
| `deploy requires an activation token` | backend deploy needs platform/app authority | export `AOMI_APP_ACTIVATION_TOKEN` or `aomi-build request` one |
| `git tree is dirty` | uncommitted files in your source repo | commit, or ignore `.aomi/`, `target/`, `Cargo.lock` |
| `candidate app dir must be apps/<installation-id>/<repo-key>/<app>` | staged path doesn't match the backend contract | redeploy through the backend (don't hand-push the platform repo) |
| `sdk_version mismatch` | `aomi-sdk` dep doesn't match `platform.json` | pin the exact `required_sdk_version` |
| `... returned 502` | release tarball not built yet, or backend can't reach GitHub | retry after CI finishes |

## Quick reference

| Where | What |
|---|---|
| `https://staging-api.aomi.dev` / `https://api.aomi.dev` | staging / production backend |
| `POST /api/platforms/:platform/deploy` | source fetch, staging, manifest generation |
| `POST /api/platforms/:platform/apps/activate` | artifact resolution + activation |
| `GET /api/control/apps/status` | runtime registry (`is_active` / `loaded`) |
| `AOMI_BACKEND_URL` / `AOMI_APP_SOURCE_ID` / `AOMI_APP_ACTIVATION_TOKEN` | CLI inputs |
| `.aomi/deployment.json` | the backend's deploy record, kept locally |
