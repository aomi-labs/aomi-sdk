# `sign_tx_solana` — runtime implementation guide

The SDK marker for this host primitive landed in [sdk/src/builder.rs](../sdk/src/builder.rs) (look for `host_target!(SignTxSolana, "sign_tx_solana")`). The runtime side is not yet wired up. This doc tells a fresh implementer just enough to ship it.

2026-05-29 verification note: `product-mono` currently has a separate
agent-facing `svm-core` namespace (`svm_stage_ix` → `svm_simulate_tx` →
`svm_commit_tx`, plus `svm_commit_message`). It does not implement a
`sign_tx_solana` tool. This doc remains specifically about supporting
existing app route plans, especially `apps/byreal`, that emit the SDK
`host::SignTxSolana` marker and expect signed serialized transaction bytes
to be routed into a submit tool.

## Why this primitive exists

Existing route-oriented host signing primitives (`commit_txs`, `commit_eip712`) are EVM-only. The byreal app ([apps/byreal](../apps/byreal)) — and any future Solana app — needs an equivalent for SVM. We chose a **sign-only, singular** primitive that mirrors `commit_eip712`'s shape: app builds an unsigned tx, host wallet signs, app submits. No batching, no host-side broadcast — those are caller concerns.

## Tool contract

**Name:** `sign_tx_solana` (verbatim — must match the SDK marker).

**Args (LLM-facing):**
```json
{
  "unsigned_tx": "<base64-encoded serialized Solana transaction bytes>",
  "description": "<human-readable summary for the wallet UX>"
}
```

`unsigned_tx` is the output of `VersionedTransaction.serialize()` (or legacy `Transaction.serialize()`) base64-encoded. Versioned (v0) transactions are the common case; legacy is acceptable too. The wallet must support both.

`description` is free-text (e.g. "Swap 1 USDC for 0.005 SOL via byreal RFQ"). Display it to the user alongside the wallet's own decoded view.

**Bound artifact (what the SDK route plan awaits):** the signed transaction bytes, base64-encoded, as a single string. Apps bind this via `.bind_as("signed_tx")` and the runtime splices it into the matching `submit_*` continuation's `signed_tx` arg.

```json
"<base64-encoded serialized signed tx, ready to broadcast or hand off>"
```

Do not return the transaction signature (the 64-byte sigblob) instead — apps need the full signed tx because byreal's submit endpoints take the whole serialized tx, not just the sig.

## Implementation outline

1. Register the tool in the host runtime's tool catalog under the exact name `sign_tx_solana`.
2. Validate args: `unsigned_tx` must be a non-empty string parseable as base64; `description` is required.
3. Decode → `VersionedTransaction::deserialize(&base64::decode(unsigned_tx)?)`. If that fails, fall back to legacy `Transaction::deserialize`. Reject if neither parses.
4. Resolve the connected SVM wallet from session state. The convention used by app code ([apps/byreal/src/tool/mod.rs](../apps/byreal/src/tool/mod.rs) `resolve_address(_, ctx, "svm")`) is `domain.svm.address` in the user_state attributes — make sure the wallet adapter populates that on connect.
5. Hand the deserialized tx to the wallet adapter (Phantom / Backpack / Solflare via the standard Solana wallet adapter) for `signTransaction(tx)`. Wallets typically render their own decoded preview alongside `description`.
6. On approval, re-serialize the signed tx (`signedTx.serialize()`), base64-encode, return as the bound artifact.
7. On user reject, return a structured error the runtime treats as terminal — same way `commit_eip712` rejection is handled.

**Do not broadcast.** Byreal's submit endpoints (and other Solana dApp patterns) want pre-broadcast bytes so they can route through their own RPCs / sequencers. If a future use case needs a sign-and-broadcast variant, add `host::SignAndSendTxSolana` as a separate primitive — don't conflate.

## Reference files

- **SDK marker + unit test:** [sdk/src/builder.rs](../sdk/src/builder.rs) — search for `SignTxSolana` and `route_builder_serializes_solana_sign_plan`.
- **App-side route builder:** [apps/byreal/src/tool/mod.rs](../apps/byreal/src/tool/mod.rs) — `build_solana_signed_routes` shows exactly what shape the runtime will see in `args` for the `sign_tx_solana` step.
- **App-side consumers:** [apps/byreal/src/tool/spot.rs](../apps/byreal/src/tool/spot.rs) `BuildSwap` + `SubmitSwap`, and [apps/byreal/src/tool/lp.rs](../apps/byreal/src/tool/lp.rs) `BuildClaimRewards` + `SubmitClaimRewards`.
- **Mirror primitive (EVM):** existing `commit_eip712` in the host runtime — copy the wallet-adapter integration pattern from there.
- **Wire-format reference:** [byreal-cli/src/core/transaction.ts](https://github.com/byreal-git/byreal-cli/blob/main/src/core/transaction.ts) shows the exact `deserialize / sign / serialize` flow we expect.

## Validation

After implementing, the byreal app's spot/lp write tools become exercisable end-to-end. Useful smoke tests:

1. **Manual:** in a chat session, ask the LLM to swap $0.50 USDC for SOL on byreal. Watch the LLM call `byreal_spot_build_swap`, the runtime invoke `sign_tx_solana`, the wallet prompt, then `byreal_spot_submit_swap`. Verify the tx lands on Solana.
2. **Automated:** see the scaffolded gate at [apps/byreal/tests/byreal_solana_smoke.rs](../apps/byreal/tests/byreal_solana_smoke.rs). Read smokes already pass (3/3); a write smoke can be added by mirroring the perps `place_live_smoke_order` in [apps/byreal/src/testing.rs](../apps/byreal/src/testing.rs) — sign locally with `solana-sdk` Keypair gated on `BANANA_SOLANA_PRIVATE_KEY`, but route through `byreal::testing::*` helpers so the byreal HTTP client paths are exercised.

## Conventions to match

- **Domain attribute:** `domain.svm.address` — use this key when the wallet adapter publishes the connected pubkey. Apps look it up via `resolve_address(_, ctx, "svm")`.
- **Chain-id convention:** Solana doesn't use EVM-style numeric chain IDs. If the host needs one for routing, use the string `"solana:mainnet"` / `"solana:devnet"` (Solana CAIP-2). Don't repurpose `--chain` flags from the EVM path.
- **Single sign per call:** keep it singular. Wallets prompt once per tx; batch flows must issue separate `SignTxSolana` route steps. There is intentionally no plural form.
