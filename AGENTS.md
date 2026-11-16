# Aomi SDK Agent Guide

This repository holds the `aomi-sdk` guest SDK, the `aomi-build` CLI and example packages. When a package breaks, use this process.

## Structure

- A package is `Cargo.toml`, `aomi.toml` and `src/lib.rs` with `Tool` impls and `aomi_sdk::package!(..)`; see `docs/repo-structure.md`.
- The WIT is `sdk/wit`; do not edit it here.
- Build for `wasm32-wasip2`; test tool logic natively with `aomi_sdk::testing`.

## Debug Process

1. Read the package code first. Find the current base URL, request paths, payload builders, and `aomi.toml` hosts.
2. Check the official docs on the public internet. Prefer official docs, API references, quick starts, and integration guides over third-party summaries.
3. Probe live endpoints with `curl` before patching. Confirm:
   - the old host/path really fails
   - the current documented host/path responds
   - the documented payload shape is accepted
4. Patch the smallest correct surface first:
   - base URL
   - request method/path
   - request payload shape
   - response parsing
   - `aomi.toml` hosts if the API moved (a request to an undeclared host fails)
5. Fix the API client first and keep the tool's `Outcome` shape stable.

## API Migration Rules

- Do not assume old hosts remain valid. Verify them.
- Do not assume symbol inputs are acceptable when the API now expects token addresses. Resolve symbols to chain-specific addresses first if the docs require it.
- Do not assume chain names are accepted when the API now expects numeric chain IDs. Resolve common aliases before quoting or building.
- If docs and live behavior differ, prefer documented production hosts plus live successful responses.
- If a field is undocumented, avoid guessing unless live probing confirms it.

## Validation

Use the same checks this repo's CI uses:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -Dwarnings`
- `cargo build -p <package> --target wasm32-wasip2 --release`

Also run small targeted tests for any new helpers you add.

When debugging live integrations, keep one or two direct `curl` probes as evidence that the new host/schema works.

## PR Process

- Base new fix branches from `publish`, not from an unrelated feature branch.
- Use the `codex/` branch prefix by default.
- Keep unrelated local files out of commits and PRs.
- Open PRs against `publish`.
- After opening a PR, wait for GitHub CI to finish and report the actual status, not just local expectations.

## Writing Guidance

- Keep tool descriptions aligned with real behavior; they are model-facing.
- Prefer succinct operational instructions over long explanations.
