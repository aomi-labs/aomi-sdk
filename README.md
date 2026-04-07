# aomi-apps

Open-source app layer for the Aomi ecosystem.

This repository contains public dynamic app crates, the public SDK they build against, and a small build toolchain for compiling plugins. It intentionally excludes:

- the runtime / loader implementation
- admin and database-facing apps
- oversized internal apps like `l2beat`
- proprietary infrastructure, internal namespaces, and private deployment wiring

## What Lives Here

- `apps/*`: public app crates that compile to dynamic plugins
- `sdk`: the public plugin SDK used by those apps
- `xtask`: helper commands for building and validating plugins in this repo
- `sdk/examples/app-template-http`: reference app showing the recommended file layout for a new plugin
- `docs/host-interop.md`: the public host capability contract used by execution-oriented apps
- `docs/repo-structure.md`: how to structure a new app crate in this repo

## Included Apps

- `defi`
- `delta`
- `kalshi`
- `khalani`
- `molinar`
- `para`
- `para-consumer`
- `pelagos`
- `polymarket`
- `prediction`
- `social`
- `x`

## Public Boundary

Apps in this repository may depend on:

- `sdk`
- public HTTP APIs
- environment variables for third-party API keys
- documented host interoperability conventions

Apps in this repository must not depend on:

- internal databases
- private control planes
- internal-only namespaces like `database`
- hidden fallback infrastructure

## Quick Start

1. Copy `sdk/examples/app-template-http` or an existing `apps/*` crate.
2. Keep the standard file split:
   - `src/lib.rs`: app manifest + preamble
   - `src/client.rs`: HTTP client + models
   - `src/tool.rs`: tool implementations
3. If your app needs wallet execution or signing, use the public host conventions from `docs/host-interop.md`.

## Build Plugins

Build every app plugin into `plugins/` with:

```bash
cargo run -p xtask -- build-aomi
```

Useful flags:

```bash
cargo run -p xtask -- build-aomi --app x
cargo run -p xtask -- build-aomi --release
cargo run -p xtask -- build-aomi --target aarch64-apple-darwin
```

## Publication Pipeline

Apps are developed via PR, built by CI, and delivered to the runtime as pre-built dynamic plugins.

### Workflow

1. Developer creates/modifies an app and opens a PR to `main`
2. CI runs tests, clippy, and builds all plugins to validate
3. PR is merged to `main`, then merged forward to `publish`
4. Push to `publish` triggers the release workflow which auto-tags, cross-compiles, and publishes a GitHub Release
5. The product-mono backend polls for new releases, downloads the tarball, and hot-reloads changed plugins

### Architecture

```mermaid
graph LR
    subgraph "aomi-apps repo"
        DEV[Developer]
        PR[PR to main]
        CI_CHECK[CI: test + build]
        PUBLISH[publish branch]
        RESOLVE[Resolve version<br/>+ auto-tag]
        BUILD_L[Build plugins<br/>linux x86_64]
        BUILD_M[Build plugins<br/>macOS ARM64]
        TARBALL_L[tarball + manifest<br/>linux]
        TARBALL_M[tarball + manifest<br/>macOS]
        RELEASE[GitHub Release<br/>apps-v0.x.y]
    end

    subgraph "product-mono backend"
        FETCHER[PluginFetcher<br/>polls every 5min]
        EXTRACT[Download + extract<br/>+ verify SHA256]
        LOADER[AppLoader<br/>dlopen + ABI check]
        REGISTRY["DashMap<br/>(AppKey → Arc&lt;App&gt;)"]
        SESSIONS[Active sessions<br/>keep old Arc]
        NEW[New sessions<br/>get new Arc]
    end

    DEV --> PR --> CI_CHECK --> PUBLISH
    PUBLISH --> RESOLVE
    RESOLVE --> BUILD_L & BUILD_M
    BUILD_L --> TARBALL_L --> RELEASE
    BUILD_M --> TARBALL_M --> RELEASE
    RELEASE -.->|HTTP poll| FETCHER
    FETCHER --> EXTRACT --> LOADER --> REGISTRY
    REGISTRY --> SESSIONS
    REGISTRY --> NEW
```

### Release Sequence

```mermaid
sequenceDiagram
    participant DEV as Developer
    participant GH as GitHub (aomi-apps)
    participant CI as Release Workflow
    participant BE as Backend (product-mono)
    participant RT as AomiRuntime

    DEV->>GH: merge PR to publish
    GH->>CI: push event (publish branch)

    CI->>CI: Read version from sdk/Cargo.toml
    CI->>GH: Create tag apps-v0.x.y (if new)

    par Linux build
        CI->>CI: cargo xtask build-aomi --release --target x86_64-linux
        CI->>CI: Generate manifest.json + SHA256 checksums
        CI->>CI: tar czf plugins tarball
    and macOS build
        CI->>CI: cargo xtask build-aomi --release --target aarch64-apple-darwin
        CI->>CI: Generate manifest.json + SHA256 checksums
        CI->>CI: tar czf plugins tarball
    end

    CI->>GH: gh release create apps-v0.x.y (attach tarballs)

    loop Every AOMI_PLUGINS_POLL_SECS (default 300s)
        BE->>GH: GET /repos/.../releases/latest
        GH-->>BE: { tag_name: "apps-v0.x.y" }
        BE->>BE: Compare with .version marker

        alt New version detected
            BE->>GH: Download tarball for host target
            GH-->>BE: tarball bytes
            BE->>BE: Extract + verify SHA256 + codesign (macOS)
            BE->>BE: Write .version marker

            loop Each plugin in manifest
                BE->>RT: reload_plugin(name)
                RT->>RT: dlopen new .so/.dylib
                RT->>RT: Validate ABI version
                RT->>RT: Build DynApp
                RT->>RT: Atomic swap in DashMap
                Note over RT: Old sessions keep old Arc<br/>New sessions get new Arc
            end
        end
    end
```

### Tarball Format

Each GitHub Release contains per-target tarballs:

```
aomi-plugins-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
└── plugins/
    ├── manifest.json
    ├── defi.so
    ├── delta.so
    ├── kalshi.so
    ├── khalani.so
    ├── molinar.so
    ├── para.so
    ├── para_consumer.so
    ├── pelagos.so
    ├── polymarket.so
    ├── prediction.so
    ├── social.so
    └── x.so
```

`manifest.json` contains version, ABI version, target triple, commit SHA, and per-plugin SHA256 checksums.

### Environment Variables (Backend)

| Variable | Default | Description |
|---|---|---|
| `AOMI_PLUGINS_VERSION` | `latest` | Version to fetch (`0.1.0` or `latest`) |
| `AOMI_PLUGINS_REPO` | `aomi-labs/aomi-sdk` | GitHub `owner/repo` for releases |
| `AOMI_PLUGINS_POLL_SECS` | `300` | Poll interval in seconds |
| `GITHUB_TOKEN` | — | Optional auth for private repos |

### Local Development

```bash
# Build all plugins locally
cargo xtask build-aomi

# Build a single app
cargo xtask build-aomi --app defi

# Scaffold a new app
cargo xtask new-app my-app

# Test against product-mono (from product-mono root)
LOCAL_AOMI_APPS=/path/to/aomi-apps bash scripts/dev.sh --local-apps
```

## SDK and Examples

The SDK is vendored in `sdk`, including its tests and `examples/hello-app`, so this repository compiles without reaching back into `product-mono`.
