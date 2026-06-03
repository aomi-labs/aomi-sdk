# Marinade aomi-app — handoff

Reference SVM app demonstrating `BuiltinApp::SvmSelfBroadcast` variant
declaration + the new `host::SvmStageIx` / `host::SvmCommitIx` route
markers (SDK 0.1.23). Second reference app after byreal; the contrast
is intentional:

| | byreal | marinade |
|---|---|---|
| Namespace declaration | string-typed (`["evm-core", "svm-reads"]`) | variant-typed (`variant = SvmSelfBroadcast`) |
| Broadcaster | venue HTTP submit (byreal `/dex/v2/send-swap-tx`) | wallet (today) / runtime RPC (post-#38-pipeline-c) |
| Chain mix | cross-chain (Hyperliquid + Solana) | SVM-only |
| Tx production | venue-built unsigned tx blob (Lane 2) | app-composed ix list (Lane 1) |
| Route markers used | `host::SvmSignTx` | `host::SvmStageIx` + `host::SvmCommitIx` |

The full matrix of pipeline shapes lives in
`product-mono/docs/topics/solana/ralph/state/svm-evm-gap-39.md` § Variant-
by-variant gap audit.

## What works today

- **All four read tools** hit `api.marinade.finance` directly and return
  live data: `marinade_get_apy`, `marinade_get_tvl`,
  `marinade_get_exchange_rate`, `marinade_get_validators`. Manifest +
  variant declaration verified by the 4 smoke tests at
  `src/lib.rs::testing::tests`.
- **Write tools** (`marinade_build_stake`,
  `marinade_build_liquid_unstake`) produce a structurally correct route
  plan: preview → `host::SvmStageIx` → `host::SvmCommitIx({mode: "wallet"})`
  → `submit_*` continuation. The plan shape is end-to-end correct against
  the route-builder contract (verified in the
  `route_builder_serializes_*` tests on the SDK side too).
- **Anchor discriminators** for `deposit` and `liquid_unstake` are
  computed from `sha256("global:<method>")[..8]` and pinned in tests.
  Production-correct against Marinade IDL.
- **Variant + namespace composition**: `MarinadeApp.variant()` returns
  `Some(SvmSelfBroadcast)`; `manifest.namespaces` is `None` because the
  variant arm of `dyn_aomi_app!` sets the trait method to `None` and
  expects the host to seed from `variant.default_namespaces()`.

## What's stubbed (the production-readiness gap)

**Instruction account lists are placeholders.** The Marinade `deposit`
ix takes ~11 accounts, most of which are PDAs (state, liq-pool legs,
reserve, msol mint authority, the user's mSOL ATA). Resolving them
correctly requires either:

1. Linking the **marinade-anchor-common** crate (or `marinade-finance`
   client) and calling its typed account-resolution helpers.
2. Hand-rolling PDA derivation against the Marinade IDL: deriving the
   liq-pool authority via
   `find_program_address(&[state.key.as_ref(), b"liq_pool_authority"],
   program_id)`, the user's mSOL ATA via the SPL Token associated-
   account derivation, etc.

Today `deposit_accounts_stub` and `liquid_unstake_accounts_stub` return
**`"__TODO_<role>"` placeholder strings** for these accounts. The ix
data (discriminator + amount bytes) is correct; the account list is
not. A simulate on this tx will fail with `InvalidAccountData` or
`AccountNotFound`.

The route-plan-shape contract is validated end-to-end (program ID,
discriminators, ix data layout, accounts list structure). The on-chain
correctness gap is bounded to the account-list contents.

## Follow-up items (ordered by priority)

### 1. Real account resolution

Pick option (a) — add `marinade-anchor-common` as a path/git dep — or
option (b) — write a small `pda.rs` module with seeds + derivation.
Update `deposit_accounts_stub` / `liquid_unstake_accounts_stub` to
return real `Pubkey` strings. Update tests to verify deterministic
PDA derivation matches Marinade's on-chain accounts.

Est: ~half day with the marinade-anchor-common crate; ~1 day hand-
rolled.

### 2. Delayed unstake (ticket NFT flow)

Marinade's zero-fee unstake path: `order_unstake` returns a ticket
NFT, user holds for 1-2 epochs, then calls `claim` to redeem SOL.
Needs:

- `marinade_build_delayed_unstake` / `_submit_*` pair (same route
  shape as liquid_unstake; different ix).
- `marinade_get_my_tickets` read tool — query the user's outstanding
  ticket NFTs.
- `marinade_build_claim` / `_submit_*` pair to redeem matured tickets.

Est: ~1 day (same patterns; new ix + a read tool).

### 3. Internal-rpc broadcast mode

Once host `#38-pipeline-c` lands (runtime broadcast loop +
`WalletCallback::Tx*` callbacks), flip the `mode` arg in
`build_marinade_route_plan` from `"wallet"` to `"internal-rpc"`.
Marinade is a textbook self-broadcast use case — autonomous portfolio
agents staking/unstaking on schedule want the runtime to handle
confirm + rebroadcast.

Est: trivial code change (one arg); behavior change is on the host
side.

### 4. Variant consumption tracking

When host `#DynManifest variant consumption` lands (loader reads the
field and composes `variant.default_namespaces() ∪ explicit_namespaces`),
this app starts getting `svm-reads + svm-ix-broadcast + svm-tx-broadcast`
registered automatically from the variant declaration alone. No app-
side change needed; just verify on the host side that variant-typed
apps load with the expected namespace set.

### 5. Stats endpoint shape validation

The four read tools today return Marinade's stats responses verbatim.
If the upstream shape drifts (a field gets renamed), the agent's
preamble references stale field names. Pin a structural fixture per
endpoint (canned response JSON in `tests/fixtures/`) and add a
round-trip test loading each fixture into the typed wrapper. Catches
upstream drift before it hits prod.

Est: ~half day.

## Tracking the host ralph coder

See `product-mono/docs/topics/solana/ralph/state/svm-evm-gap-39.md`
for the variant-vs-tools gap audit and impl row order. As of iter 39:

- `#38-pipeline-c` (runtime broadcast loop) — critical path; unblocks
  this app's `internal-rpc` mode.
- `#39-svm-apps-c` (`svm_sign_tx`) — landed; relevant to byreal-style
  sign-only apps, not this app (Marinade uses commit).
- `#38-pipeline-b1/b2/b3` (Lane 2 storage + `svm_stage_tx`) — unblocks
  the `host::SvmStageTx` marker addition to SDK + apps that use
  venue-built tx blobs (byreal again, not Marinade).
- `#DynManifest variant consumption` — needs to be filed; without it,
  this app's `variant` field is informational only.

## How to test locally

```sh
cd apps/marinade
cargo test --lib   # all unit tests
cargo build         # produces target/debug/libmarinade.dylib (or .so)
```

The dylib loads against any host that knows the new
`svm-reads + svm-ix-broadcast + svm-tx-broadcast` sub-namespaces and
will surface 4 read tools + 4 write tools to the LLM. Read tools work
immediately; write tools execute up through stage, then fail at simulate
with `InvalidAccountData` until follow-up #1 lands.
