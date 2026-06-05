# aomi-git

CLI for publishing Aomi app source through the current direct-Git platform
transport and activating the resulting backend registry row.

```text
aomi-git request  -> ask ops for direct-Git onboarding and activation details
aomi-git deploy   -> stage into platform repo -> push -> CI builds + cuts release
aomi-git status   -> poll CI, release, and backend registry health
aomi-git activate -> backend fetches + loads the release
```

| Command | What it does | Who runs it |
|---|---|---|
| `request` | Posts a direct-Git onboarding request to the Aomi apps Discord so ops can invite your GitHub account to the platform repo and issue activation details. | The app author |
| `deploy` | Snapshots your source repo, stages it under `apps/<slug>/` in the platform repo, commits, and pushes to the platform's deployment branch (resolved from the backend). CI then builds the cdylib and cuts a GitHub release. | The app author (a collaborator) |
| `status` | Reads `.aomi/deployment.json`, polls GitHub Actions and release state, then reports whether activation is ready. | The app author |
| `activate` | Tells a backend to fetch a published release, validate it, and load it. | Whoever holds an activation bearer for the app/platform/release scope |

Everything `deploy` learns about your app is written to
`.aomi/deployment.json` - a plan artifact whose centerpiece is the
**validation pipeline** (see [Checks: the validation pipeline](#checks-the-validation-pipeline)).

---

## `aomi-git request`

The direct-Git onboarding step for a new contributor. You can't use the current
Git transport until ops invites your GitHub account to the platform repo, and
you can't self-activate until ops issues an activation bearer. `request` posts
that ask - carrying your GitHub account, email, and app - to the Aomi apps
Discord, pinging the ops role.

```bash
# Resolves app / platform / repo from aomi.toml in --path.
aomi-git request --email you@example.com --git-account your-github-user

# Preview the exact Discord message without posting.
aomi-git request --email you@example.com --git-account your-github-user --dry-run
```

Ops then (1) sends your GitHub account a collaborator invite and (2) issues the
activation details out-of-band. The bearer is never part of the request and
never travels over Discord.

### Flags

| Flag | Mirrors | Meaning |
|---|---|---|
| `--email <EMAIL>` | - | Where ops sends activation details. **Required.** |
| `--git-account <USER>` | - | GitHub account to invite as a platform-repo collaborator. **Required.** |
| `--app <NAME>` | `aomi.toml [app].name` | App slug. Defaults to the value in `aomi.toml`. |
| `--platform <NAME>` | `aomi.toml [app].platform` | Platform tag. Falls back to `aomi.toml`, then `community`. |
| `--path <DIR>` | - | Source repo for the `aomi.toml` lookup. Default: `.` |
| `--dry-run` | - | Print the Discord message; post nothing. |

> Community-tier only: B2B partners deploy through a server-side proxy
> (ADR 0011) and don't get direct repo access, so they don't use `request`.

> Refactor boundary: this command describes the current direct-Git transport.
> A backend-publish transport should use a separate request kind instead of
> overloading collaborator-invite wording.

---

## `aomi-git deploy`

Run it from your **source repo** (the crate with `aomi.toml` + `src/lib.rs`).
It never edits your source - it copies a snapshot into a *transit clone* of the
platform repo and pushes from there.

### Three ways to run it

```sh
# 1. Dry-run - plan only. Writes .aomi/deployment.json, pushes nothing.
#    Always attempts online checks when a backend URL is available.
aomi-git deploy --dry-run

# 2. Real deploy via the managed transit cache (the default).
#    aomi-git clones/refreshes the platform repo for you under a cache dir.
aomi-git deploy

# 3. Real deploy via a hand-managed clone (escape hatch).
#    You own the clone; aomi-git only stages + commits + pushes into it.
aomi-git deploy --platform-dir /path/to/platform-repo
```

### Flags

| Flag | Mirrors | Meaning |
|---|---|---|
| `[PATH]` (`--path`) | - | App source directory. Default: `.` |
| `--platform <NAME>` | `aomi.toml [app].platform` | Platform tag. Default: aomi.toml's value, then `community`. |
| `--source-repo <URL\|owner/repo>` | `aomi.toml [app].git` | Platform repo location. When omitted, resolved from the backend's platform record. |
| `--platform-dir <DIR>` | - | Escape hatch: hand-managed local clone to stage/push from. Skips the managed transit cache. |
| `--backend <URL>` | `AOMI_BACKEND_URL` | Backend base URL. **Required for a live deploy** — `aomi-git` reads the platform's deployment branch from `GET /api/control/platforms` and pushes there. (Not needed for `--dry-run`.) |
| `--dry-run` | - | Plan + best-effort backend reads. No staging, push, or activation. Refreshes `.aomi/deployment.json`. |
| `--activate` | `AOMI_APP_ACTIVATION_TOKEN` | After a successful push, explicitly attempt backend activation. Normally run `status` first and then `activate` once the release exists. |
| `--allow-dirty` | - | Permit a dirty working tree in the plan and during staging. |
| `--json` | - | Print the plan/outcome as JSON instead of the human summary. |

> **The deployment branch is owned by the backend, not you.** `aomi-git` has no
> `--branch` flag and `aomi.toml` has no `[app].branch` — the branch a platform's
> releases land on is whatever `GET /api/control/platforms` reports for that
> platform, and `deploy` pushes to exactly that. A live deploy therefore needs a
> reachable backend (`--backend`/`AOMI_BACKEND_URL`); it will refuse rather than
> guess a branch.

> `deploy` no longer auto-activates just because `AOMI_APP_ACTIVATION_TOKEN` is
> present. Activation is a separate backend step unless `--activate` is passed
> explicitly.

---

## `aomi-git status`

Run after `deploy` to see when CI has finished, when the release asset exists,
and whether the backend has loaded the app.

```sh
aomi-git status --path /path/to/app
```

### Flags

| Flag | Mirrors | Meaning |
|---|---|---|
| `[APP_RELEASE_TAG]` | - | app_release_tag to check. Falls back to `.aomi/deployment.json`'s `target.app_release_tag`. |
| `--source-repo <URL\|owner/repo>` | `aomi.toml [app].git` | Platform repo location. Falls back to deployment.json. |
| `--backend <URL>` | `AOMI_BACKEND_URL` | Backend base URL for registry/health checks. Pass `--backend ''` to skip. |
| `--access-token <$ENV\|VAL>` | `aomi.toml [app].access_token` | GitHub PAT for private-repo reads. Omit for public repos. |
| `--path <DIR>` | - | Source repo for the deployment.json fallback. Default: `.` |
| `--json` | - | Print the status report as JSON. |

---

## `aomi-git activate`

Run by whoever holds an activation bearer for the app/platform/release scope.
Tells the backend to fetch a release by tag, validate it, and load it.

```sh
aomi-git activate apps-my-bot-abc1234 \
  --backend https://staging-api.aomi.dev \
  --activation-token <activation-bearer> \
  --source-repo aomi-labs/community-apps \
  --target-tag staging \
  --visibility public
```

### Flags

| Flag | Mirrors | Meaning |
|---|---|---|
| `[APP_RELEASE_TAG]` | - | app_release_tag to activate (e.g. `apps-my-bot-abc1234`). Falls back to `.aomi/deployment.json`'s `target.app_release_tag`. |
| `--platform <NAME>` | `aomi.toml [app].platform` | Platform tag. Falls back to deployment.json, then `community`. |
| `--source-repo <URL\|owner/repo>` | `aomi.toml [app].git` | `source_repo` recorded on the app row. Falls back to deployment.json, then a backend lookup. |
| `--backend <URL>` | `AOMI_BACKEND_URL` | Backend base URL. **Required.** |
| `--activation-token <T>` | `AOMI_APP_ACTIVATION_TOKEN` | Activation bearer. **Required.** |
| `--access-token <$ENV\|VAL>` | `aomi.toml [app].access_token` | GitHub PAT (or `$ENV_VAR` ref) for the backend's one-shot release fetch. Only needed for **private** platform repos. |
| `--target-tag <TAG>` | - | Required backend server tag. Repeatable. |
| `--visibility <V>` | `aomi.toml [app].public` | `private` (default) or `public`. |
| `--display-name <STR>` | `aomi.toml [app].display_name` | Registry label. Falls back to deployment.json. |
| `--source-commit <SHA>` | - | Provenance. Falls back to deployment.json. |
| `--source-tree <SHA>` | - | Provenance. Falls back to deployment.json. |
| `--source-digest <SHA>` | - | Provenance. Falls back to deployment.json. |
| `--path <DIR>` | - | Source repo for the deployment.json fallback. Default: `.` |
| `--dry-run` | - | Print the activation request that would be sent; no HTTP. |
| `--json` | - | Print the backend response as JSON. |

> **Where does the activation token come from?** Platform ops issue an
> activation bearer out-of-band. The bearer is platform-wide or app-scoped,
> depending on backend policy. Set `AOMI_APP_ACTIVATION_TOKEN` or pass
> `--activation-token`.

## Defaults pyramid

Commands resolve each value through a best-effort chain:

```
CLI flag -> .aomi/deployment.json (at --path) -> backend lookup -> hardcoded default
```

Each step is best-effort: a missing `deployment.json` or unreachable backend
never aborts the plan - only the specific operation that genuinely needs the
unresolved value will error.

## Environment variables

| Var | Used by | Purpose |
|---|---|---|
| `AOMI_BACKEND_URL` | `deploy`, `status`, `activate` | Backend base URL when `--backend` is omitted. |
| `AOMI_APP_ACTIVATION_TOKEN` | `activate` and `deploy --activate` | Activation bearer. |

---

## Checks: the validation pipeline

Every `deploy` (including `--dry-run`) runs a validation pipeline and records
the result in `.aomi/deployment.json`. The pipeline is grouped into **four
ordered stages**, each a precondition for the next. A failing gate
short-circuits the rest - downstream stages are recorded as `skipped`.

```
1. workspace   ->   2. manifest   ->   3. platform   ->   4. backend
   (local git)        (aomi.toml)         (resolve repo        (server tags
                                           + branch)            + DB acceptance)

   offline ------------------            ---------- online (needs --backend) ----------
```

Stages 1-2 are **offline** (computed from local git + `aomi.toml`). Stages 3-4
are **online** - they only run when a backend URL is available; otherwise they
stay `skipped`.

### The four stages

| Stage | Question it answers | Establishes (resolved facts) |
|---|---|---|
| `workspace` | Is the local tree shippable? | - |
| `manifest` | Does `aomi.toml` declare what we need? | `server_tags`, `defaulted` |
| `platform` | Resolve the platform repo + its backend-owned deploy branch. | `name`, `github_repo`, `deployment_branch` |
| `backend` | Will the backend actually accept this release? | - |

### The checks

| Check | Stage | Severity | Verifies |
|---|---|---|---|
| `git_clean` | workspace | error | Working tree has no uncommitted changes. |
| `platform_declared` | manifest | error | `aomi.toml` has `[app].platform`. Nothing resolves without it. |
| `git_declared` | manifest | **warn** | `aomi.toml` has `[app].git`. Advisory - a backend lookup can supply the repo; a missing value only skips `git_url_matches_platform`. |
| `backend_reachable` | platform | error | `GET /api/control/platforms` succeeded. The gate that opens stages 3 & 4. |
| `platform_resolved` | platform | error | The declared platform is registered with the backend. |
| `deploy_branch_resolved` | platform | error | The platform's contractual `deployment_branch` was read from the backend registry. `deploy` pushes to exactly this branch — it is never a client/`aomi.toml` input, so there is no branch to "mismatch." |
| `git_url_matches_platform` | platform | **warn** | Your `aomi.toml` git URL matches the platform's record. Advisory - fork-tolerant. |
| `server_tags_subset` | backend | error | Your `server_tags` are a subset of the backend's `AOMI_SERVER_TAGS`. A mismatch is a 409 at activate time. |

### Checks vs. resolved facts

A **check** is a pass/fail assertion. A **resolved fact** is an output - a
value the stage established. They're kept separate so a red mark always means
"blocked," never "here's a value."

For example, `server_tags` is *not* a check - it always has a value (defaulting
to `["staging"]`). It's a resolved fact on the `manifest` stage, alongside
`defaulted` (whether you pinned it or we filled in the default).

### Severity and stage status

Each check carries a **severity**:

- **`error`** - a gate. A failure fails the whole stage and should block the deploy.
- **`warn`** - advisory. A failure downgrades the stage to `warning` but does not block.

A stage's **status** is rolled up from its checks (order-independent):

| Status | Meaning |
|---|---|
| `passed` | All checks passed. |
| `failed` | At least one `error` check failed - blocked here. |
| `warning` | Only `warn` checks failed - advisory, not blocking. |
| `skipped` | The stage didn't run (an upstream gate failed, or its inputs were absent, e.g. no backend URL, or no declared `server_tags`). |

### What you see

The human summary (non-`--json`) prints one line per stage, with the detail of
anything that didn't pass indented beneath:

```
Preflight
  [ok]   workspace git_clean
  [ok]   manifest  platform_declared, git_declared  |  defaulted=true server_tags=[staging]
  [ok]   platform  backend_reachable, platform_resolved, deploy_branch_resolved, git_url_matches_platform  |  deployment_branch=publish github_repo=aomi-labs/community-apps name=community
  [ok]   backend   server_tags_subset
```

The same data, structured, in `.aomi/deployment.json`:

```jsonc
{
  "stages": [
    {
      "stage": "manifest",
      "status": "passed",
      "checks": [
        { "name": "platform_declared", "passed": true, "severity": "error", "detail": "community" },
        { "name": "git_declared",      "passed": true, "severity": "warn",  "detail": "https://github.com/aomi-labs/community-apps" }
      ],
      "resolved": { "defaulted": true, "server_tags": ["staging"] }
    }
    // workspace, platform, backend
  ]
}
```

---

## `.aomi/deployment.json`

The plan artifact written next to your `aomi.toml`. It is **always rewritten in
full** on each operation (via temp-file + rename, so partial writes are never
observable). Beyond `stages`, it carries:

- `app`, `source`, `platform`, `target` - the resolved plan (slug, commit,
  app_release_tag, server tags, etc.).
- `state` - three independent flags that track progress:
  - `pushed` - the push to the platform repo succeeded.
  - `deployed` - the push landed on the contractual deploy branch (a strict
    subset of `pushed`; a `--dry-run` or `--platform-dir` run that didn't push
    stays `false`).
  - `activated` - the backend wrote the app row with `is_active = true`.
- `errors` - a flat log of failure details collected during the run.

`activate` reads this file for its defaults pyramid, so running it from the same
directory as a prior `deploy` lets you omit most flags.

> **Add `.aomi/` to `.gitignore`.** It's a local artifact; committing it tends
> to dirty your tree and trip `git_clean` on the next run.

---

## Related

- [`aomi-build`](./aomi-build.md) - scaffold, build, and e2e-test an app before publishing.
- [`sdk-version-compatibility`](./sdk-version-compatibility.md) - pinning `aomi-sdk` to a platform's `required_sdk_version`.
- Platform contributor guides: `CONTRIBUTING.md` in
  [`community-apps`](https://github.com/aomi-labs/community-apps) and
  [`krexa-hosted-apps`](https://github.com/aomi-labs/krexa-hosted-apps).
