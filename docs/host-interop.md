# Host Interop

This repository treats host capabilities as a public contract, not private infrastructure.

Execution-oriented apps may assume a host runtime exposes some or all of the following tools:

- `view_state`
  - Purpose: encode calldata from ABI arguments and execute a read-only `eth_call`.
  - Typical input: `function_signature`, `arguments`, and `to`.
  - Typical output: decoded read result, revert reason, and call metadata.

- `run_tx`
  - Purpose: encode calldata from ABI arguments and simulate a state-changing contract call without staging it.
  - Typical input: `function_signature`, `arguments`, `to`, and optional `value`.
  - Typical output: simulation verdict, revert reason, and tx metadata.

- `stage_tx`
  - Purpose: stage an EVM transaction in `user_state.pending_txs` for later simulation and signing.
  - Typical input: `to`, `description`, optional `value` / `gas_limit` / `kind`, plus either `data: { encode: { signature, args } }` or `data: { raw: "0x..." }`.
  - Typical output: a staged tx payload with the authoritative `pending_tx_id` attached by the runtime.

- `simulate_batch`
  - Purpose: simulate one or more staged transactions by `pending_tx_id` before prompting a wallet.
  - Typical input: ordered list of staged tx references keyed by `pending_tx_id`.
  - Typical output: batch pass/fail status, revert reason, and any gas or decoded-call context the host can provide.

- `commit_tx`
  - Purpose: ask the user wallet to sign and broadcast one staged transaction.
  - Typical input: `pending_tx_id`.
  - Typical callback artifact: `transaction_hash`.

- `commit_eip712`
  - Purpose: ask the user wallet to sign EIP-712 typed data.
  - Typical input: `typed_data`, human description.
  - Typical callback artifact: `signature`.

## SYSTEM_NEXT_ACTION

Some apps, especially execution workflows like `khalani`, return a machine-readable next-step plan. The recommended shape is:

```json
{
  "SYSTEM_NEXT_ACTION": [
    {
      "name": "stage_tx",
      "args": {},
      "reason": "Why this step is next.",
      "condition": "Optional human-readable gate."
    }
  ]
}
```

Hosts should preserve the returned `args` exactly when they forward these steps to host tools. If an app returns `stage_tx`, the host transaction model still applies: stage first, then `simulate_batch`, then `commit_tx`.

## Design Rules

- App crates should describe host capability requirements in tool descriptions and preambles.
- App crates that receive raw external tx payloads should stage them with `stage_tx` using `data.raw` instead of trying to reconstruct ABI calls.
- App crates that need known ABI-driven checks or approvals should use `view_state`, `run_tx`, or `stage_tx` with `data.encode` rather than inventing calldata manually.
- App crates should not refer to private namespaces like `CommonNamespace`.
- App crates should not assume a hidden internal fallback network.
- If a host does not implement one of these tools, it should surface that absence explicitly instead of silently redirecting behavior.
