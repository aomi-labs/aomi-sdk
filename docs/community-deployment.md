# Community App Deployment Guide

This guide is for community members deploying an app from their own GitHub
repository to the `community` platform. You work in your source repo; you do
not open a PR against `aomi-labs/community-apps`, and you do not need write
access to that platform repo.

`aomi-build deploy` is the happy path. It checks the SDK pin, validates the
source, asks the backend to build a platform release, waits for readiness,
activates the release tag, and verifies the runtime loaded the app.

## What You Need

- `aomi-build` installed locally.
- A GitHub source repo containing an Aomi app and `aomi.toml`.
- The source commit pushed to GitHub.
- The Aomi GitHub App installed on that source repo.
- A community platform activation token.
- Backend URL: `https://api.aomi.dev`.

You do not need a GitHub personal access token, `community-apps` write access,
database access, or an admin private key.

## 1. Install The CLI

```bash
cargo install --git https://github.com/aomi-labs/aomi-sdk --features cli aomi-sdk
```

Confirm the deploy command is available:

```bash
aomi-build deploy --help
```

## 2. Prepare Your Source Repo

Your repo should contain a normal Rust cdylib app:

```text
my-app/
|-- aomi.toml
|-- Cargo.toml
|-- src/
|   `-- lib.rs
`-- .gitignore
```

Use `community` in `aomi.toml`:

```toml
name = "my-app"
display_name = "My App"
platform = "community"
public = true
```

Pin the SDK version required by the platform. The CLI can check the required
version from the backend:

```bash
aomi-build sdk check --backend https://api.aomi.dev
```

If the SDK pin is stale, let the CLI rewrite it:

```bash
aomi-build sdk fix --backend https://api.aomi.dev
```

Run your local checks, then commit and push:

```bash
cargo test
git status --short
git add aomi.toml Cargo.toml Cargo.lock src
git commit -m "Prepare Aomi community deploy"
git push
```

The backend deploys the pushed GitHub commit. Local uncommitted changes are not
included.

If your app lives in a subdirectory, run app-local checks against that
subdirectory and deploy from the source repo root with an explicit manifest:

```bash
aomi-build sdk check --path apps/my-app --backend https://api.aomi.dev
aomi-build deploy --platform community --repo owner/repo --aomi-toml apps/my-app/aomi.toml
```

## 3. Install The Aomi GitHub App

Set the backend once for the shell:

```bash
export AOMI_BACKEND_URL=https://api.aomi.dev
```

Start the connection flow. Replace `owner/repo` with your GitHub repo:

```bash
aomi-build connect --platform community --repo owner/repo
```

The command opens GitHub's Aomi Build app install page. In GitHub:

1. Choose the GitHub account or organization that owns `owner/repo`.
2. Select **Only select repositories**.
3. Select the app source repo you want to deploy.
4. Click **Install** or **Save**.
5. Return to the terminal and paste the installation id if the CLI asks for it.
6. Paste the community activation token issued to you.

The CLI saves the backend URL, platform, and token in local `aomi-build` config
so future deploy commands can omit those flags.

If your browser cannot open from the terminal, print the install URL instead:

```bash
aomi-build connect --platform community --repo owner/repo --no-browser
```

Open the printed URL, install the Aomi Build GitHub App on the source repo, then
return to the terminal and continue the prompts.

If the Aomi Build GitHub App is already installed on the repo, re-authorize and
bind the existing install instead of creating a new one:

```bash
aomi-build connect --platform community --repo owner/repo --authorize
```

You can verify the backend can resolve your installed repo:

```bash
aomi-build source sync --platform community --repo owner/repo
```

## 4. First Deploy

From the root of your source repo:

```bash
aomi-build deploy --platform community --repo owner/repo
```

If you prefer not to save credentials, pass them explicitly:

```bash
AOMI_BACKEND_URL=https://api.aomi.dev \
AOMI_APP_ACTIVATION_TOKEN=<community-token> \
aomi-build deploy --platform community --repo owner/repo
```

The command runs the full lifecycle:

```text
sdk check -> preflight -> deploy run -> wait for ready -> activate -> verify loaded
```

Success means the CLI reports all of these:

- preflight passed
- backend returned a deployment id
- platform PR or CI information was recorded
- release tag was recorded in `.aomi/deployment.json`
- deployment status reached `ready`
- activation succeeded
- final app verification shows `is_active=true`, `artifact_ready=true`, and
  `loaded=true`

## 5. Check Status

The deploy writes `.aomi/deployment.json` in your source repo. Check the latest
local and backend state with:

```bash
aomi-build deploy status
```

For machine-readable output:

```bash
aomi-build deploy status --json
```

## 6. Re-Deploy After Changes

Make changes in the same source repo, then commit and push:

```bash
cargo test
git status --short
git add aomi.toml Cargo.toml Cargo.lock src
git commit -m "Update my Aomi app"
git push
```

Deploy again from the repo root:

```bash
aomi-build deploy --platform community --repo owner/repo
```

You usually do not need to pass `--app-source-id`. The CLI resolves source in
this order:

```text
--app-source-id -> AOMI_APP_SOURCE_ID -> .aomi/deployment.json -> --repo owner/repo
```

The new deploy writes a new deployment record and activates the new release tag
after the platform build is ready.

## 7. Manual Recovery Commands

Use these when you want to stop at a specific lifecycle step.

Validate inputs without opening a platform PR:

```bash
aomi-build deploy preflight --platform community --repo owner/repo
```

Create or update the platform deployment but do not activate:

```bash
aomi-build deploy run --platform community --repo owner/repo
```

Activate release tags recorded in `.aomi/deployment.json`:

```bash
aomi-build deploy activate
```

Activate an explicit release tag:

```bash
aomi-build deploy activate \
  --platform community \
  --release-tag apps-<installation-id>-<repo-key>-<app>-<commit>
```

## Common Problems

| Symptom | What it means | What to do |
|---|---|---|
| `deploy requires an activation token` | The CLI has no community token. | Run `aomi-build connect --platform community --repo owner/repo`, or export `AOMI_APP_ACTIVATION_TOKEN`. |
| `deploy needs --app-source-id` | The backend cannot identify your installed repo. | Pass `--repo owner/repo` or run `aomi-build source sync --platform community --repo owner/repo`. |
| SDK mismatch | Your app pins a different `aomi-sdk` than the platform requires. | Run `aomi-build deploy --fix-sdk --platform community --repo owner/repo`, commit the SDK change, and deploy again. |
| Branch rejected | Deploy accepts immutable commits. | Check out the branch locally, push it, and deploy local `HEAD`; or pass `--commit <sha>`. |
| Final verification is not loaded | Activation completed but the runtime did not load the plugin. | Run `aomi-build deploy status --json`; if it stays false, share the deployment id and release tag with Aomi support. |
