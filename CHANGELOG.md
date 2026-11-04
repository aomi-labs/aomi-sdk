# Changelog

## 7.0.0

### Changed (breaking)

- Every `host::*` route target now names a host tool exactly as the backend
  registers it. EVM tools carry the `evm_` prefix like their Solana pairs:
  `StageTx` → `EvmStageTx` (`evm_stage_tx`), `SimulateBatch` → `EvmSimulate`
  (`evm_simulate`), `CommitTxs` → `EvmCommitTxs` (`evm_commit_txs`),
  `GetTimeAndOnchainContext` → `EvmGetContext`, `GetContract` →
  `EvmGetContract`, `GetAccountInfo` → `EvmGetAccountInfo`, `SyncChain` →
  `EvmSyncChain`. The old `stage_tx` / `simulate_batch` / `commit_txs` names
  no longer exist on the host, so routes built on them did nothing.
- Solana: `SvmSimulateIx` and `SvmSimulateTx` are one `SvmSimulate`
  (`svm_simulate`, both lanes), and `SvmSignData` is `SvmCommitMessage`
  (`svm_commit_message`).
- Removed targets with no host tool: `BraveSearch`, `CommitTx`, `ViewState`,
  `RunTx`.

### Added

- `EvmCall`, `EvmDiscardTx`, `EvmGetTokenBalance`, `EvmGetTokenHoldings`,
  `SvmDiscardTx`.

### Fixed

- `aomi-build` collision checks and the `aomi-run` host stubs use the host's
  current namespaces (`aomi-core`, `evm-core`, `evm-reads`, `evm-sim`,
  `svm-core`, `svm-reads`, `svm-write-ix`, `svm-write-tx`) and tool names.
- In-repo apps and skill docs route through the renamed targets and name the
  current host tools in their prompts.

### Compatibility

- FFI version (1) unchanged; only route tool names changed. Rebuild an app on
  7.0.0 and rename its `host::*` markers.

## 6.0.2

### Added

- `aomi-build project create` works with just `aomi-build login`: without an
  activation token it opens the GitHub App approval for the repo, waits for it,
  and the backend creates the Project for the approving GitHub account. `--repo`
  defaults to the checkout's origin and `--platform` to `community`.
- A failed approval (for example, a repo that is already a Project) is reported
  instead of waiting for the timeout, on backends that report failures.

### Compatibility

- CLI-only release; library and FFI version (1) unchanged. Activation tokens
  keep the admin path.

## 6.0.1

### Fixed

- `aomi-build` calls Build's `/api/build/{cli,deployments}/*` routes. Build
  removed the old `/api/bff/*` paths, so `login`, `deploy`, `status` and
  `activate` in 6.0.0 fail with 404 against current Build.
- `aomi-build login` ignores stray requests to its local callback port (editor
  and port-scanner probes) instead of aborting with "invalid state".

### Added

- `aomi-build --version` prints the CLI version.

### Compatibility

- CLI-only release. The library and FFI version (1) are unchanged; apps pinned
  to `=6.0.0` keep building and loading. Upgrade the CLI with
  `cargo install aomi-sdk --version 6.0.1 --features cli --locked`.

## 5.1.1

### Added

- Secret declarations can opt into per-user credentials with
  `Secret::new(...).user_owned()`. The manifest serializes this as
  `"user_own": true`.
- `resolve_user_secret_value` reads user-owned values only from the
  authenticated tool context, without argument or process-environment
  fallback.

### Fixed

- FFI failure traces no longer record arbitrary tool error text before the
  host can redact it. Async sink failures follow the same rule.
- `DynToolCallCtx` debug output redacts its injected secret values.

### Compatibility

- Existing `Secret::new(name, description, required)` declarations remain
  operator-owned and serialize `"user_own": false`. Manifests without the new
  field also deserialize as operator-owned.
- The host and every plugin must be rebuilt against exactly SDK 5.1.1 because
  the runtime keeps an exact SDK-version compatibility gate.

## 5.1.0

### Added

- App skills can own typed dynamic tools with `tools: [Tool, ...]`. Owned
  tools are included in the plugin manifest and dispatch router, while the
  host withholds their schemas and rejects dispatch until the owning skill is
  active.

### Changed

- Dynamic tool names now share one canonical SDK validator at build and host
  load boundaries. Names must contain only lowercase ASCII letters, digits,
  `_`, or `-`, and must be unique within the app manifest.
- App skill `content_digest` now covers descriptions, tags, owned tools, and
  hook bindings in addition to sections and guard tables.

### Compatibility

- The host and every plugin must be rebuilt against exactly SDK 5.1.0 before
  using skill-owned tools. The exact-version compatibility gate is unchanged.

## 5.0.1

### Fixed

- **App skill `content_digest` depended on the compiling workspace's
  `serde_json` features.** `skill::digest` canonicalized a skill's
  `GuardTable` with `serde_json::Value::to_string`, whose object key order
  follows `serde_json/preserve_order`: struct-field order when the feature is
  resolved on (the host workspace pulls it in transitively), sorted order when
  it is off (a typical community plugin workspace). A plugin and the host
  therefore hashed different bytes for the same guard, and
  `validate_app_skills` rejected every guarded skill with
  `content_digest mismatch: manifest says <a>, content hashes to <b>`.
  The digest now hashes a canonical form (compact JSON, object keys sorted
  bytewise at every depth) that is independent of `serde_json` features and of
  struct field order. Guard-less skills never touched a JSON map, so their
  digests are unchanged. `skill::tests::guard_digest_is_pinned` locks both
  values to literals.

### Compatibility

- The host and every plugin must run the **same fixed SDK version**; the
  exact-match `AOMI_SDK_VERSION` gate applies as usual
  (`docs/sdk-version-compatibility.md`).
- Plugins built against **5.0.0 that ship guard tables** must be rebuilt
  against the fixed SDK. Their stored digest was computed by the pre-fix
  code and only happens to match the new canonical form when that build
  resolved `serde_json` *without* `preserve_order`; do not rely on that.
- Plugin workspaces that enabled `serde_json = { features = ["preserve_order"] }`
  purely to work around this mismatch can drop the override once they are on
  the fixed SDK.
