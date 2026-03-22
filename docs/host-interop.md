# Host Interop

This repository treats host capabilities as a public contract, not private infrastructure.

Execution-oriented apps may assume a host runtime exposes some or all of the following tools:

- `encode_and_simulate`
  - Purpose: preflight an EVM transaction before prompting a wallet.
  - Typical input: chain, `to`, `data`, `value`, and optional sender context.
  - Typical output: decoded call details, simulation result, revert reason, gas estimate.

- `send_transaction_to_wallet`
  - Purpose: ask the user wallet to sign and broadcast a transaction.
  - Typical input: chain, `to`, `data`, `value`, human description.
  - Typical callback artifact: `transaction_hash`.

- `send_eip712_to_wallet`
  - Purpose: ask the user wallet to sign EIP-712 typed data.
  - Typical input: `typed_data`, human description.
  - Typical callback artifact: `signature`.

## SYSTEM_NEXT_ACTION

Some apps, especially execution workflows like `khalani`, return a machine-readable next-step plan. The recommended shape is:

```json
{
  "SYSTEM_NEXT_ACTION": [
    {
      "name": "send_transaction_to_wallet",
      "args": {},
      "reason": "Why this step is next.",
      "condition": "Optional human-readable gate."
    }
  ]
}
```

Hosts should preserve the returned `args` exactly when they forward these steps to wallet or signing tools.

## Design Rules

- App crates should describe host capability requirements in tool descriptions and preambles.
- App crates should not refer to private namespaces like `CommonNamespace`.
- App crates should not assume a hidden internal fallback network.
- If a host does not implement one of these tools, it should surface that absence explicitly instead of silently redirecting behavior.
