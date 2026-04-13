# Aomi Apps Agent Guide

This repository contains standalone Aomi app crates under `apps/<name>`. When an app breaks, use this process.

## Structure

- Each app is usually split across `src/lib.rs`, `src/client.rs`, and `src/tool.rs`.
- `lib.rs` defines the app preamble and tool list.
- `client.rs` usually owns HTTP clients, schemas, shared helpers, and runtime builders.
- `tool.rs` usually owns Aomi tool entrypoints and workflow orchestration.

## Runtime Rules

- Treat `SYSTEM_NEXT_ACTION` as the source of truth for execution workflows.
- Preserve wallet/tool args exactly when a tool returns `SYSTEM_NEXT_ACTION`.
- Do not rebuild wallet payloads manually if the tool already emitted them.
- Preserve callback artifacts exactly: `transaction_hash`, `signature`, `quote_id`, `route_id`, `submit_type`, and similar follow-up fields.
- Do not bypass harness-driven wallet steps with ad hoc tool calls.

## Debug Process

1. Read the app code first. Find the current base URL, request paths, payload builders, and runtime flow.
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
   - prompt/preamble wording if the runtime flow changed
5. Keep runtime flow compatibility where possible. If an app already uses `SYSTEM_NEXT_ACTION`, fix the API client first and preserve the harness contract.

## API Migration Rules

- Do not assume old hosts remain valid. Verify them.
- Do not assume symbol inputs are acceptable when the API now expects token addresses. Resolve symbols to chain-specific addresses first if the docs require it.
- Do not assume chain names are accepted when the API now expects numeric chain IDs. Resolve common aliases before quoting or building.
- If docs and live behavior differ, prefer documented production hosts plus live successful responses.
- If a field is undocumented, avoid guessing unless live probing confirms it.

## Validation

Use the same checks this repo's CI uses for changed apps:

- `cargo fmt --manifest-path apps/<app>/Cargo.toml`
- `cargo clippy --manifest-path apps/<app>/Cargo.toml --lib -- -Dwarnings`
- `cargo test --manifest-path apps/<app>/Cargo.toml --no-run`

Also run small targeted tests for any new helpers you add.

When debugging live integrations, keep one or two direct `curl` probes as evidence that the new host/schema works.

## PR Process

- Base new fix branches from `publish`, not from an unrelated feature branch.
- Use the `codex/` branch prefix by default.
- Keep unrelated local files out of commits and PRs.
- Open PRs against `publish`.
- After opening a PR, wait for GitHub CI to finish and report the actual status, not just local expectations.

## Writing Guidance

- Keep app preambles aligned with the real runtime behavior.
- If a tool now returns `SYSTEM_NEXT_ACTION`, say that explicitly in the preamble and tool descriptions.
- Prefer succinct operational instructions over long explanations.
