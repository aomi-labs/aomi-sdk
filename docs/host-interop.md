# Host Interop

This repository treats host capabilities as a public contract, not private infrastructure.

Execution-oriented apps may assume a host runtime exposes some or all of the following tools:

- `encode_and_call`
  - Purpose: encode calldata from ABI arguments and execute a read-only `eth_call`.
  - Typical input: `function_signature`, `arguments`, `to`, and optional `value`.
  - Typical output: decoded read result, revert reason, and call metadata.

- `stage_tx`
  - Purpose: stage an EVM transaction in `user_state.pending_txs` for later simulation and signing.
  - Typical input: `to`, `description`, optional `value` / `gas_limit` / `kind`, plus either `data: { encode: { signature, args } }` or `data: { raw: "0x..." }`.
  - Typical output: a staged tx payload with the authoritative `pending_tx_id` attached by the runtime.

- `simulate_batch`
  - Purpose: simulate one or more staged transactions by `pending_tx_id` before prompting a wallet.
  - Typical input: ordered list of staged tx references keyed by `pending_tx_id`.
  - Typical output: batch pass/fail status, revert reason, and any gas or decoded-call context the host can provide.

- `commit_txs`
  - Purpose: ask the user wallet to sign and broadcast one or more staged transactions as a batch.
  - Typical input: `tx_ids` from `user_state.pending_txs`.
  - Typical callback artifact: `transaction_hash`.

- `evm_commit_message`
  - Purpose: ask the user wallet to sign EIP-712 typed data.
  - Typical input: `typed_data`, human description.
  - Typical callback artifact: `signature`.

## Route hints (`ToolReturn` envelope)

Apps that drive multi-step execution flows (e.g. `khalani`, `polymarket`) return a `ToolReturn` instead of a bare JSON value. `ToolReturn` carries the tool's structured payload plus an ordered `routes: Vec<RouteStep>` list. Each `RouteStep` declares a triggering event (`OnSyncReturn` or `OnBoundEvent`), the next tool to call, hinted args, an optional `prompt` override, and optionally host-owned `enforcement` metadata for deterministic follow-up execution.

Builders are exposed in `aomi_sdk::route`:

```rust
use aomi_sdk::{RouteStep, ToolReturn};

ToolReturn::with_routes(value, [
    RouteStep::on_return("evm_commit_message", wallet_request)
        .bind_as("clob_l1_signature")
        .prompt("Suggested next step: call evm_commit_message with these args."),
    RouteStep::on_bound_event(
        "submit_polymarket_order",
        submit_template,
        "clob_l1_signature",
    )
    .prompt("Wallet signed — submit the order now."),
])
```

The host treats each route as advisory: `OnSyncReturn` steps render into the next system prompt the LLM sees, while `OnBoundEvent` steps wait for the named alias to resolve. Wallet callbacks, staged transaction completions, and other out-of-band events all flow through the runtime's `RoutedEventBridge` and splice fields like `signature` or `transaction_hash` into the hinted args before the continuation prompt is injected. The runtime never parses prose; the route's structured fields are the contract.

If an app returns a `stage_tx` route, the host transaction model still applies: stage first, then `simulate_batch`, then `commit_txs`. The SDK also still exposes legacy `commit_tx`, `view_state`, and `run_tx` markers for older route plans, but the current `product-mono` EVM namespace advertises `commit_txs` and `encode_and_call`.

RouteBuilder v1 invariants:

- artifact aliases (`bind_as(...)`, enforced-step `bind_as(...)`, and `awaits(...)`) must be non-empty
- aliases must be unique within one route plan
- at most one `next(...)` producer may attach `enforcement`
- bound producers must have unique tool names in one route plan

## Design Rules

- App crates should describe host capability requirements in tool descriptions and preambles.
- App crates that receive raw external tx payloads should stage them with `stage_tx` using `data.raw` instead of trying to reconstruct ABI calls.
- App crates that need known ABI-driven checks or approvals should use `encode_and_call` or `stage_tx` with `data.encode` rather than inventing calldata manually.
- App crates should not refer to private namespaces like `CommonNamespace`.
- App crates should not assume a hidden internal fallback network.
- If a host does not implement one of these tools, it should surface that absence explicitly instead of silently redirecting behavior.
