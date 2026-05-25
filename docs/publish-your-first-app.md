# Publish your first Aomi app

This guide walks you from zero to a published Aomi app on the public **community** platform. At the end you have a real plugin compiled to `.so`, validated by CI, released as a versioned GitHub artifact, and ready for the Aomi team to activate against the live runtime.

Time to complete: ~10 minutes, plus CI build time.

---

## What you need

- Rust toolchain, `rustc 1.91+` (install via `rustup`).
- `git`.
- `gh` ([GitHub CLI](https://cli.github.com/)), authenticated. `gh auth status` should show you are logged in.
- The `aomi-deploy` binary and its underlying `aomi-git`. Build both from `aomi-labs/product-mono`:
  ```bash
  gh repo clone aomi-labs/product-mono
  cd product-mono/aomi
  cargo build --release -p aomi-git -p aomi-deploy
  # put both binaries on your PATH:
  export PATH="$PWD/target/release:$PATH"
  ```

---

## The architecture in one breath

You write a Rust `cdylib` crate that depends on `aomi-sdk` and registers tools via the `dyn_aomi_app!` macro. `aomi-deploy push` stages your source into a *platform repo* (`aomi-labs/community-apps`) on its `publish` branch. CI in that repo compiles your crate, validates the plugin, packages a tarball, and publishes a GitHub release tagged `apps-{your-slug}-{short-commit}`. The Aomi runtime polls for that release, downloads it, verifies SHA-256, and loads your plugin into the live catalog. Once the team activates your app, it shows up in the agent tool catalog.

```
your-repo/                aomi-deploy push          community-apps         CI build              GitHub Release        Aomi runtime
  src/lib.rs    ───────►   stages files  ───────►   apps/your-app/  ───►  cdylib + tarball  ──► apps-your-app-abc ──► loaded
  Cargo.toml               writes manifest          on publish branch     + SHA-256              + manifest.json       + activated
```

---

## TL;DR (two commands plus an edit)

```bash
aomi-deploy init hello-aomi-bot
cd hello-aomi-bot
# Edit src/lib.rs to add your real tool, then:
git commit -am "add real tool"
aomi-deploy push
```

Then watch CI at `https://github.com/aomi-labs/community-apps/actions` and ping the team to activate your release tag.

---

## Walkthrough

### 1. Scaffold the crate

```bash
aomi-deploy init hello-aomi-bot
```

This creates:

```
hello-aomi-bot/
├── .gitignore
├── Cargo.toml             # pinned aomi-sdk = "=0.1.19", cdylib crate type
└── src/
    └── lib.rs             # dyn_aomi_app! invocation with an echo tool stub
```

It also runs `git init` and `git add .` for you. Prints next steps.

> **Want a different platform?** `aomi-deploy init my-app --platform krexa` scaffolds against the private partner platform instead. Skip `git init` with `--no-git`.

### 2. Write your tool

Open `src/lib.rs`. The scaffold has an `EchoTool` placeholder. Replace its `NAME`, `DESCRIPTION`, args struct, and `run` body with your real tool. Keep your toolset to 3 to 8 tools per app.

Two things matter for the LLM:
- The `DESCRIPTION` is read by the model. Write triggers ("Use when the user wants…"), not feature lists.
- The `preamble` in the `dyn_aomi_app!` macro at the bottom sets the app persona and decision priors.

For reference, a real production app: `apps/krexa/` in this repo.

Commit your changes:

```bash
git commit -am "init hello-aomi-bot"
```

### 3. Sanity check locally (optional)

```bash
cargo build --release --lib
```

If this fails on the `aomi-sdk` resolve step, your version pin is wrong. `aomi-deploy init` should have pinned it correctly. If it is wrong, check it against [`community-apps/ci/platform.json`](https://github.com/aomi-labs/community-apps/blob/main/ci/platform.json). If the macro fails, compare against [`community-apps/examples/hello-ci`](https://github.com/aomi-labs/community-apps/tree/main/examples/hello-ci).

### 4. Dry run the publish plan

`aomi-deploy push` does not have a dedicated dry run flag yet, but the underlying `aomi-git deploy --dry-run` does and is safe:

```bash
aomi-git deploy --dry-run --platform community
```

Output names your app, the expected release tag, the publish repo. No side effects:

```
Publish plan (dry-run)
  platform             : community
  app_slug             : hello-aomi-bot
  expected_release_tag : apps-hello-aomi-bot-3fc98da326bb
  visibility           : public
  effects              : no file staging, no git push, no backend calls
  ...
```

Note the `expected_release_tag`. You will use it for activation later.

### 5. Push

```bash
aomi-deploy push
```

This auto clones `aomi-labs/community-apps` into `~/.aomi/platforms/community/` on first run (cached for next time), refreshes the publish branch, then stages your files, commits with a structured message, and pushes. End to end output looks like this:

```
  ✓ Cloned aomi-labs/community-apps · 2.1s
  ✓ Fetched publish · up to date
  ✓ Staged Hello Aomi Bot · hello-aomi-bot
  ✓ Committed 10757dd · on publish

  Publish plan
    platform  community
    app       Hello Aomi Bot (hello-aomi-bot)
    target    apps/hello-aomi-bot
    commit    10757dd on publish
    pushed    yes

  ┌─ Next steps ──────────────────────────────────────┐
  │  1  gh run watch -R aomi-labs/community-apps      │
  │  2  aomi-deploy status apps-hello-aomi-bot-...    │
  │  3  Ask team to activate                          │
  └────────────────────────────────────────────────────┘
```

Want to commit locally but skip the push? Add `--no-push`.

### 6. Watch CI

```bash
gh run watch -R aomi-labs/community-apps
```

The workflow:

1. Detects which app dirs changed.
2. Cross compiles your cdylib for `x86_64-unknown-linux-gnu`.
3. `dlopen`s the resulting `.so` and validates its runtime manifest against the bundle contract (`aomi-plugin-bundle-v1`).
4. Packages a `aomi-plugins-<slug>-<short-commit>-x86_64-unknown-linux-gnu.tar.gz`.
5. Publishes a GitHub release tagged `apps-hello-aomi-bot-<short-commit>` with three assets: the tarball, `manifest.json`, and `aomi-release.json`.

Build typically takes 5 to 10 minutes.

Check status anytime:

```bash
aomi-deploy status apps-hello-aomi-bot-<short-commit>
```

### 7. Activation (currently team gated)

Activation flips your app from "release published" to "loaded into the running backend" by writing a row in the `applications` table. The endpoint is `/api/admin/apps/activate`, guarded by `AOMI_APP_ACTIVATION_TOKEN`. **The activation token currently lives with the Aomi team. This is the manual review gate.**

When CI publishes your release tag, post it in the Aomi builder Discord or your partnership channel along with a one line summary. The team will run:

```bash
AOMI_BACKEND_URL=https://api.aomi.dev \
AOMI_APP_ACTIVATION_TOKEN=<...> \
aomi-deploy activate apps-hello-aomi-bot-<short-commit> --public --label "Hello Aomi Bot"
```

Within ~5 minutes of activation, the runtime `PluginFetcher` pulls your release, verifies SHA-256, codesigns it on macOS, and atomically swaps your plugin into the live catalog. Your tool then appears in the agent tool surface for new sessions.

Confirm load state:

```bash
curl https://api.aomi.dev/api/control/apps/status | jq '.apps[] | select(.name == "hello-aomi-bot")'
```

---

## What happens next

- **Iterate**: change your code, commit, rerun `aomi-deploy push`. Each push produces a fresh release tagged with the new short commit. The previous release stays in GitHub for rollback.
- **Add more tools**: implement another `DynAomiTool` and append it to `tools = [...]` in `dyn_aomi_app!`. Keep your toolset to ~3 to 8 tools per app for clean LLM behavior.
- **Need wallet signing or simulation?** Add the `forge` or `database` namespaces and use the public host conventions in [host-interop.md](./host-interop.md).
- **Want a private deployment** (your app should not be public)? Use `aomi-deploy push --platform krexa` for partner scoped deployments. Same flow.

---

## Common errors

- **`aomi-deploy: command not found`**. Build and put the binary on PATH. See "What you need" above.
- **`failed to run aomi-git deploy; ensure aomi-git is on PATH`**. `aomi-deploy` shells out to `aomi-git`. Both need to be on PATH.
- **`git tree is dirty; commit or stash changes, or pass --allow-dirty`**. Commit first. CI requires a clean source commit.
- **`resolved aomi-sdk version does not match product-mono host contract`**. Your `Cargo.toml` does not pin `=0.1.19`. If you used `aomi-deploy init`, this should already be correct. If you copied from somewhere else, fix the version pin.
- **`Cargo.toml must set [lib].crate-type = ["cdylib"]`**. Add the `[lib]` section. `aomi-deploy init` does this for you.
- **CI fails on `aomi_create returned null`**. Your `dyn_aomi_app!` macro is missing or malformed. Compare against [`community-apps/examples/hello-ci`](https://github.com/aomi-labs/community-apps/tree/main/examples/hello-ci).

---

## Under the hood (skip if you just want to ship)

`aomi-deploy` is a thin wrapper. It shells out to two existing tools:

- `aomi-git` does the actual app discovery (reads `Cargo.toml`), source staging (per file SHA-256, manifest writing), and git transport (clone, commit, push). Lives at [`product-mono/aomi/bin/aomi-git`](https://github.com/aomi-labs/product-mono/tree/main/aomi/bin/aomi-git).
- `gh` clones the platform repo and inspects releases.

The wrapper job is to remove friction: caching the platform repo clone under `~/.aomi/platforms/<platform>/`, refreshing it on each invocation, and giving you a single command instead of five. If you prefer the lower level tools, the same flow with `aomi-git` directly works fine. See the [`community-apps` README](https://github.com/aomi-labs/community-apps/blob/main/README.md) for that path.

---

## Reference

- [`community-apps/README.md`](https://github.com/aomi-labs/community-apps/blob/main/README.md): publication contract.
- [`community-apps/ci/platform.json`](https://github.com/aomi-labs/community-apps/blob/main/ci/platform.json): current required SDK version and targets.
- [sdk-version-compatibility.md](./sdk-version-compatibility.md): why exact SDK match.
- [repo-structure.md](./repo-structure.md): recommended app file layout.
- [host-interop.md](./host-interop.md): host capability contract for signing and execution.
- `apps/krexa/`: a real production app for reference.
