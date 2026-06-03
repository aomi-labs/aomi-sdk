# `svm_sign_tx` — runtime contract

The SDK marker for this host primitive lives in [sdk/src/builder.rs](../sdk/src/builder.rs) (look for `host_target!(SvmSignTx, "svm_sign_tx")`). The host runtime now exposes the same verb; this doc records the app-facing contract.

## Why this primitive exists

All existing host signing primitives (`commit_tx`, `evm_commit_message`) are EVM-only. The byreal app ([apps/byreal](../apps/byreal)) — and any future Solana app — needs an equivalent for SVM. We chose a **sign-only, singular** primitive that mirrors `evm_commit_message`'s shape: app builds an unsigned tx, host wallet signs, app submits. No batching, no host-side broadcast — those are caller concerns.

## Tool contract

**Name:** `svm_sign_tx` (verbatim — must match the SDK marker).

**Args (LLM-facing, direct form):**
```json
{
  "unsigned_tx": "<base64-encoded serialized Solana transaction bytes>",
  "description": "<human-readable summary for the wallet UX>"
}
```

**Args (LLM-facing, staged form):**
```json
{
  "tx_id": 42,
  "description": "<optional override>"
}
```

`unsigned_tx` is the output of `VersionedTransaction.serialize()` (or legacy `Transaction.serialize()`) base64-encoded. Versioned (v0) transactions are the common case; legacy is acceptable too. The wallet must support both.

`description` is free-text (e.g. "Swap 1 USDC for 0.005 SOL via byreal RFQ"). Display it to the user alongside the wallet's own decoded view.

**Bound artifact (what the SDK route plan awaits):** the signed transaction bytes, base64-encoded, as a single string. Apps bind this via `.bind_as("signed_tx")` and the runtime splices it into the matching `submit_*` continuation's `signed_tx` arg.

```json
"<base64-encoded serialized signed tx, ready to broadcast or hand off>"
```

Do not return the transaction signature (the 64-byte sigblob) instead — apps need the full signed tx because byreal's submit endpoints take the whole serialized tx, not just the sig.

## Runtime outline

1. Register the tool in the host runtime's tool catalog under the exact name `svm_sign_tx`.
2. Validate args: either `unsigned_tx` or `tx_id` must be present.
3. For direct args, decode → `VersionedTransaction::deserialize(&base64::decode(unsigned_tx)?)`. If that fails, fall back to legacy `Transaction::deserialize`. Reject if neither parses. For staged args, resolve `tx_id` through the host's pending SVM transaction store and reconstruct the unsigned transaction bytes from the staged blob.
4. Resolve the connected SVM wallet from session state. The convention used by app code ([apps/byreal/src/tool/mod.rs](../apps/byreal/src/tool/mod.rs) `resolve_address(_, ctx, "svm")`) is `domain.svm.address` in the user_state attributes — make sure the wallet adapter populates that on connect.
5. Hand the deserialized tx to the wallet adapter (Phantom / Backpack / Solflare via the standard Solana wallet adapter) for `signTransaction(tx)`. Wallets typically render their own decoded preview alongside `description`.
6. On approval, re-serialize the signed tx (`signedTx.serialize()`), base64-encode, return as the bound artifact.
7. On user reject, return a structured error the runtime treats as terminal — same way `evm_commit_message` rejection is handled.

**Do not broadcast.** Byreal's submit endpoints (and other Solana dApp patterns) want pre-broadcast bytes so they can route through their own RPCs / sequencers. If a future use case needs a sign-and-broadcast variant, add `host::SignAndSendTxSolana` as a separate primitive — don't conflate.

## Reference files

- **SDK marker + unit test:** [sdk/src/builder.rs](../sdk/src/builder.rs) — search for `SvmSignTx` and `route_builder_serializes_solana_sign_plan`.
- **App-side route builder:** [apps/byreal/src/tool/mod.rs](../apps/byreal/src/tool/mod.rs) — `build_svm_sign_tx_routes` shows exactly what shape the runtime will see in `args` for the `svm_sign_tx` step.
- **App-side consumers:** [apps/byreal/src/tool/spot.rs](../apps/byreal/src/tool/spot.rs) `BuildSwap` + `SubmitSwap`, and [apps/byreal/src/tool/lp.rs](../apps/byreal/src/tool/lp.rs) `BuildClaimRewards` + `SubmitClaimRewards`.
- **Mirror primitive (EVM):** existing `evm_commit_message` in the host runtime.
- **Wire-format reference:** [byreal-cli/src/core/transaction.ts](https://github.com/byreal-git/byreal-cli/blob/main/src/core/transaction.ts) shows the exact `deserialize / sign / serialize` flow we expect.

## Validation

After implementing, the byreal app's spot/lp write tools become exercisable end-to-end. Useful smoke tests:

1. **Manual:** in a chat session, ask the LLM to swap $0.50 USDC for SOL on byreal. Watch the LLM call `byreal_spot_build_swap`, the runtime invoke `svm_sign_tx`, the wallet prompt, then `byreal_spot_submit_swap`. Verify the tx lands on Solana.
2. **Automated:** see the scaffolded gate at [apps/byreal/tests/byreal_solana_smoke.rs](../apps/byreal/tests/byreal_solana_smoke.rs). Read smokes already pass (3/3); a write smoke can be added by mirroring the perps `place_live_smoke_order` in [apps/byreal/src/testing.rs](../apps/byreal/src/testing.rs) — sign locally with `solana-sdk` Keypair gated on `BANANA_SOLANA_PRIVATE_KEY`, but route through `byreal::testing::*` helpers so the byreal HTTP client paths are exercised.

## Conventions to match

- **Domain attribute:** `domain.svm.address` — use this key when the wallet adapter publishes the connected pubkey. Apps look it up via `resolve_address(_, ctx, "svm")`.
- **Chain-id convention:** Solana doesn't use EVM-style numeric chain IDs. If the host needs one for routing, use the string `"solana:mainnet"` / `"solana:devnet"` (Solana CAIP-2). Don't repurpose `--chain` flags from the EVM path.
- **Single sign per call:** keep it singular. Wallets prompt once per tx; batch flows must issue separate `SvmSignTx` route steps. There is intentionally no plural form.
