# Solana venue-broadcast commit — app-facing contract

> **2026-07-04:** the `svm_sign_tx` verb this doc used to describe is
> retired. Sign-only is no longer a tool — it is the **venue cell** of
> `svm_commit_tx`, selected by the staged artifact's `broadcaster`
> config. The SDK markers live in [sdk/src/builder.rs](../sdk/src/builder.rs)
> (`host_target!(SvmStageTx, ...)` / `host_target!(SvmCommitTx, ...)`);
> the old `SvmSignTx` marker is deleted.

## The model

Two orthogonal decisions route every SVM commit, and **neither is made
by the app's tools or the LLM**:

- **Who signs** — kernel policy on the user's wallet
  (`public_keys.signing_mode`, resolved by the host's signing gate):
  `human_sync` routes to the connected wallet, `autonomous` signs
  server-side via the delegated provider grant, `denied` rejects.
- **Who submits** — the `Broadcaster` config
  (`"wallet" | "venue" | "aomi"`): the app manifest's
  `broadcast = { default, allowed }` block declares the operator
  policy (see `BroadcastConfig` in [sdk/src/types.rs](../sdk/src/types.rs));
  a stage call may pin `broadcaster` explicitly for flows with a hard
  venue constraint (RFQ fills only settle through the venue). User
  preferences and runtime retries resolve within `allowed`.

`broadcaster: "venue"` is the app-broadcast pattern: the signed bytes
return to the app's own `submit_*` tool and the venue broadcasts
(byreal `send-swap-tx`, Jupiter `/execute`, Raydium tx-API).

## Route-plan contract (what an app emits)

The canonical venue flow is a three-node plan — see byreal's
`build_venue_commit_routes` in
[apps/byreal/src/tool/mod.rs](../apps/byreal/src/tool/mod.rs):

```rust
ToolReturn::route(preview)
    .next(|next| {
        next.add::<host::SvmStageTx>(json!({
            "tx": unsigned_tx_b64,          // base64 VersionedTransaction from the venue
            "description": description,
            "broadcaster": "venue",          // the artifact pin
        }));
        next.add::<host::SvmCommitTx>(json!({}))
            .note("Call with { \"tx_id\": <pending_tx_id> } from the stage step.")
            .bind_as("signed_tx");
    })
    .after::<SubmitSwap>(submit_template)
    .awaits("signed_tx")
```

1. **`svm_stage_tx`** decodes and validates the venue blob (payer must
   equal the connected wallet), stamps `broadcaster` +
   `preserve_blockhash` (default `true`; venue blobs must stay
   byte-stable), and mints a `pending_tx_id`.
2. **`svm_commit_tx { tx_id }`** executes under kernel policy. On a
   human-sync wallet the FE gets a sign-only request
   (`request_kind: "sign_transaction"`); on an autonomous-armed wallet
   the kernel signs server-side with **no FE round-trip** — either way
   the base64 signed bytes bind to the `signed_tx` alias.
3. The **`submit_*` continuation** fires with `signed_tx` spliced into
   its args and forwards the bytes to the venue endpoint.

The app code cannot tell which signer ran — that is what makes venue
flows schedulable/unattended without app changes.

## Bound artifact (unchanged from the old verb)

The signed transaction bytes, base64-encoded, as a single string —
the **full serialized signed tx**, not the 64-byte signature blob;
byreal's submit endpoints take the whole serialized tx. Apps bind it
via `.bind_as("signed_tx")` and the runtime splices it into the
matching `submit_*` continuation's `signed_tx` arg.

## Wallet expectations (unchanged)

- The staged blob is `VersionedTransaction::serialize()` base64; v0 is
  the common case, legacy acceptable.
- The FE signature request shows `description` alongside the wallet's
  own decoded view; the wallet signs, it does **not** broadcast.
- Single sign per commit: wallets prompt once per tx. Batch flows issue
  separate stage + commit step pairs, each binding a distinct alias.

## Conventions to match

- **Domain attribute:** `domain.svm.address` — the connected pubkey.
  Apps look it up via `resolve_address(_, ctx, "svm")`.
- **Chain-id convention:** Solana uses CAIP-2 strings
  (`"solana:mainnet"` / `"solana:devnet"`), not EVM numeric chain ids.

## Errors worth knowing

- Fee-payer mismatch → staged rejection at `svm_stage_tx` (re-quote
  with the connected address).
- `signing_denied` / `signing_wallet_owned_by_other_account` → kernel
  gate rejection at commit; not negotiable mid-run.
- Blockhash expiry: venue blobs cannot be refreshed (bytes are
  venue-authoritative) — on expiry, re-quote and re-stage. This is the
  expected path, not an exception.

## Reference files

- **SDK markers + plan test:** [sdk/src/builder.rs](../sdk/src/builder.rs)
  (SVM section) and `route_builder_serializes_solana_venue_commit_plan`
  in [sdk/tests/route_builder.rs](../sdk/tests/route_builder.rs).
- **App-side route builder:** [apps/byreal/src/tool/mod.rs](../apps/byreal/src/tool/mod.rs)
  — `build_venue_commit_routes`.
- **App-side consumers:** [apps/byreal/src/tool/spot.rs](../apps/byreal/src/tool/spot.rs)
  `BuildSwap` + `SubmitSwap`, and
  [apps/byreal/src/tool/lp.rs](../apps/byreal/src/tool/lp.rs)
  `BuildClaimRewards` + `SubmitClaimRewards`.
- **Host-side source of truth:** product-mono
  `aomi/crates/tools/src/svm/tx/{stage_tx,commit}.rs` and
  `aomi/crates/tools/src/svm/gate.rs`.
