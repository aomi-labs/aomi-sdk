# byreal app — handoff

The app is merged to main and the plugin binary is registered in the runtime
manifest, but **spot/lp write paths are blocked on a runtime primitive that
hasn't shipped yet**, and the backend hasn't surfaced byreal in `aomi app
list`. Everything else is done. This doc tells you exactly where things sit and
what to pick up.

## TL;DR

- **What ships:** standalone byreal app covering all three product lines (perps
  on Hyperliquid, spot on Solana, LP/Copy Farming on Solana). 30 tools across
  three namespaces. Plugin compiled and registered.
- **What works end-to-end:** all reads (perps + spot + lp). Perps writes
  (order/cancel/leverage) — verified with a real $11 ETH order on Hyperliquid.
- **What's blocked:** spot swaps + LP reward claims. The app side is complete
  (build/submit pairs, route plans, smoke tests) but the host can't service the
  `svm_sign_tx` route step because the runtime primitive isn't implemented.
- **What's left for you:**
  1. Implement `host::SvmSignTx` in the runtime (see
     [`docs/sign-tx-solana-runtime.md`](../../docs/sign-tx-solana-runtime.md)).
  2. Get `byreal` listed by the backend in `aomi app list`.
  3. Run the Solana write smoke once (1) is live.

## What's in the repo

```
apps/byreal/
├── Cargo.toml                       # crate-type = ["cdylib", "rlib"]; deps pinned
├── BYREAL_CLI_FLOWS.md              # mermaid sequence diagrams for every flow
├── src/
│   ├── lib.rs                       # PREAMBLE + dyn_aomi_app! registration (30 tools)
│   ├── client/
│   │   ├── mod.rs                   # ByrealApp marker, envelope decode (byreal_get/post)
│   │   ├── perps.rs                 # PerpsClient — Hyperliquid /info + /exchange
│   │   ├── spot.rs                  # SpotClient — api2.byreal.io spot/CLMM/RFQ
│   │   └── lp.rs                    # LpClient — Copy Farming, positions, rewards
│   ├── tool/
│   │   ├── mod.rs                   # shared helpers (ok, resolve_address,
│   │   │                            #   validate_confirmation,
│   │   │                            #   build_evm_signed_routes,
│   │   │                            #   build_solana_signed_routes)
│   │   ├── perps.rs                 # 14 tools (8 read + 3 build/submit pairs)
│   │   ├── spot.rs                  # 9 tools (7 read + 1 build/submit pair)
│   │   └── lp.rs                    # 7 tools (5 read + 1 build/submit pair)
│   └── testing.rs                   # smoke helpers used by both test files
└── tests/
    ├── smoke_order.rs               # live Hyperliquid order place + cancel
    └── byreal_solana_smoke.rs       # live api2.byreal.io read probes
```

Also relevant:

- [`sdk/src/builder.rs`](../../sdk/src/builder.rs) — `host_target!(SvmSignTx, "svm_sign_tx")` lives here (search "SvmSignTx"). The marker is in; only the runtime side is missing.
- [`docs/sign-tx-solana-runtime.md`](../../docs/sign-tx-solana-runtime.md) — the runtime-impl spec (read this first if you're picking up the Solana piece).
- [`plugins/manifest.json`](../../plugins/manifest.json) and `aomi/plugins/manifest.json` — `byreal.dylib` is registered in both.

## Architecture in one paragraph

byreal is structurally three apps sharing one preamble. Each product line has
its own client (`client/{perps,spot,lp}.rs`) and tool module
(`tool/{perps,spot,lp}.rs`). Every write tool is a **build/submit pair**:
`build_*` returns a preview value + a routed signing step; the host wallet
signs; the runtime splices the signature into the matching `submit_*` step.
Perps routes through `host::CommitEip712` (already supported); spot and lp
route through `host::SvmSignTx` (not yet supported).

App never holds a key. Wallet addresses come from host context — `domain.evm.address`
for perps, `domain.svm.address` for spot/lp. A user can have both wallets
connected at once; they're independent trust models.

## Tool catalogue (30 tools)

| Namespace | Reads | Build/submit pairs |
|---|---|---|
| `byreal_perps_*` | `get_meta`, `get_all_mids`, `get_l2_book`, `get_account_state`, `get_open_orders`, `get_user_fills`, `get_funding_history`, `get_candles` | `order`, `cancel`, `update_leverage` |
| `byreal_spot_*` | `get_pools`, `get_pool`, `get_klines`, `get_tokens`, `get_token_prices`, `get_global_overview`, `get_swap_quote` | `swap` (handles AMM + RFQ via `router_type`) |
| `byreal_lp_*` | `get_top_performers`, `get_provider_overview`, `get_positions`, `get_unclaimed_rewards`, `get_epoch_bonus` | `claim_rewards` (v1: single-tx only) |

Full preamble is in [`src/lib.rs`](src/lib.rs) — it covers confirmation gates,
sizing/precision rules, error catalogue, and the build/submit contract.

## What's verified

- **Perps writes — real funds.** Smoke test in [`tests/smoke_order.rs`](tests/smoke_order.rs)
  placed and canceled a real ~$11 ETH Alo buy on Hyperliquid using
  `BANANA_PRIVATE_KEY` during development. The funding path (bridging USDC
  mainnet → Arbitrum → Hyperliquid via Across + 7702 AA batch) is documented in
  the previous session transcript; the test itself just needs the wallet
  pre-funded.
- **Spot + LP reads.** [`tests/byreal_solana_smoke.rs`](tests/byreal_solana_smoke.rs)
  hits `api2.byreal.io` for real and verifies all three readers
  (`spot_reads_decode_cleanly`, `swap_quote_returns_unsigned_tx`,
  `copy_farming_top_performers_returns_records`). Last green run was against
  live mainnet during the build.
- **CI.** Plugin builds via `aomi-build compile --app byreal`.
  `byreal.dylib` is in both manifests with matching sha256s.

Re-run smokes:

```bash
# Hyperliquid live order (needs funded BANANA_PRIVATE_KEY)
BANANA_PRIVATE_KEY=0x... cargo test --manifest-path apps/byreal/Cargo.toml \
  --test smoke_order -- --ignored --nocapture

# byreal Solana reads (no key, no funds)
cargo test --manifest-path apps/byreal/Cargo.toml \
  --test byreal_solana_smoke -- --ignored --nocapture
```

## What's blocked

### 1. `host::SvmSignTx` runtime impl — the main thing

The SDK marker exists and the app emits the route step. The runtime needs to
handle it. **Full spec is in [`docs/sign-tx-solana-runtime.md`](../../docs/sign-tx-solana-runtime.md)** —
~7 steps, mirrors `commit_eip712`'s wallet-adapter integration. Until this lands:

- `byreal_spot_build_swap` returns a route plan but no host can execute it.
- `byreal_lp_build_claim_rewards` same.
- All reads still work. Perps writes still work (different signing primitive).

Key points the runtime impl must hit (lifted from the spec doc):

- Tool name **must** be `svm_sign_tx` verbatim.
- Args: `{ unsigned_tx: <base64 versioned tx bytes>, description: <string> }`.
- Bound artifact: `<base64 signed tx bytes>` — full re-serialized tx, **not**
  just the 64-byte signature (byreal's submit endpoints take the whole tx).
- Resolve wallet from `domain.svm.address`.
- Do **not** broadcast. byreal does that.
- Single sign per call; batch flows issue multiple route steps.

### 2. Backend doesn't list byreal

`aomi app list` doesn't include `byreal` even though the plugin is in the
runtime manifest. Talk to whoever owns the backend app registry — the plugin
binary is shipped and registered locally, this is just the published catalog
side.

### 3. v2 deferred features (not blockers, just scope cuts)

From the preamble's "Out of scope (today)" section:

- **Perps:** inline TP/SL on opening orders, set TP/SL on existing position,
  isolated-margin update, dedicated close-position helper (today: use
  `byreal_perps_build_order` with `reduce_only: true`).
- **Perps:** agent wallet approval flow (`ApproveAgent` action) — every perps
  action is currently signed by the master.
- **Spot:** open/modify/close CLMM positions (only swaps + reads in v1).
- **LP:** multi-tx reward claims when `encode-v2` returns >1 tx. v1 errors out;
  workaround is to split positions into smaller batches client-side. Adding
  multi-step route plans here is the most natural v2.
- **LP:** `byreal_lp_build_copy_strategy` to actually mirror a top LP's
  tick-range into a new position (the marquee Copy Farming "execute" tool).
  Today the app surfaces the plan and the user executes in byreal's UI.
- **Cross-product bridging** (Solana ↔ Hyperliquid via Arbitrum) — out of
  scope; users handle this via the `across` / `lifi` apps.

## Conventions worth knowing before you touch this

These are non-obvious things that have already bitten us; keep them in mind so
you don't relearn the same lessons:

- **byreal API envelope is inconsistent.** Most endpoints return
  `{retCode, retMsg, result: {data: ...}}`; some (swap quote) flatten fields
  onto `result` directly. The decoder in `client/mod.rs` handles both via the
  `#[serde(flatten)] rest` capture — preserve this if you refactor.
- **Hyperliquid wire format is strict.** Sizes/prices must round to
  `szDecimals` / `pxDecimals`, ≤5 sig figs on px, and limit price can't deviate
  >~80% from mid. The smoke test caps deviation at 15% and rounds to the ETH
  tick for this reason.
- **`hl_ranger` is pinned to `=0.6.0-ranger.20`.** It's a prerelease, the next
  version may not be published. `OrderRequest` and `Limit` aren't re-exported
  at the crate root — go through `ClientOrderRequest::convert()` and let type
  inference handle it.
- **Spot `userPublicKey` must be base58.** byreal's quote endpoint pre-builds
  the unsigned tx for a specific signer. Passing an EVM hex address there
  returns envelope success but no `transaction` field — looks like a silent
  byreal bug. Validate at the tool layer if you add new entry points.
- **`amount` in spot quotes is the input token's atomic units**, not human
  decimals. The preamble warns the LLM but the tool layer doesn't enforce —
  worth tightening if a user hits it.
- **Confirmation gate is per-product-line.** `validate_confirmation` requires
  the literal token `"confirm"`. It's a soft gate (the LLM has to surface the
  preview first); don't remove it without thinking about what replaces it.
- **`byreal_lp_build_claim_rewards` errors if encoder returns >1 tx.** This is
  intentional in v1 — see the v2 item above. The error message tells the LLM
  to claim positions in smaller groups.

## When the runtime primitive lands — what to test

Bare minimum to declare spot/lp writes working end-to-end:

1. **Manual smoke via Aomi CLI.** Start a session with an SVM wallet connected,
   ask for a small USDC → SOL swap. Watch for the sequence:
   `byreal_spot_get_swap_quote` → `byreal_spot_build_swap` →
   wallet prompt (`svm_sign_tx`) → `byreal_spot_submit_swap` → tx confirmed
   on Solana. The `routerType` in the quote response determines which submit
   path runs.
2. **Automated smoke.** Mirror `place_live_smoke_order` in `testing.rs` for the
   Solana side: load a key from `BANANA_SOLANA_PRIVATE_KEY`, sign locally with
   `solana-sdk` `Keypair`, route through `byreal::testing::*` helpers so the
   HTTP paths get exercised. The scaffolded test file is at
   [`tests/byreal_solana_smoke.rs`](tests/byreal_solana_smoke.rs) — add a
   `swap_places_and_settles` test next to the existing read tests, gated on
   the env var.
3. **LP claim smoke.** Same shape but against `encode_reward` →
   `submit_reward_order`. Needs a wallet with actual pending fees in a byreal
   position, so this is harder to automate — manual is fine for v1.

## Files to read first, in order

1. [`src/lib.rs`](src/lib.rs) — the preamble. It's the actual product spec.
2. [`BYREAL_CLI_FLOWS.md`](BYREAL_CLI_FLOWS.md) — the mermaid diagrams. Easier
   to absorb than reading 1500 lines of Rust.
3. [`src/tool/mod.rs`](src/tool/mod.rs) — `build_evm_signed_routes` /
   `build_solana_signed_routes`. Once you grok these, every write tool reads
   the same way.
4. [`docs/sign-tx-solana-runtime.md`](../../docs/sign-tx-solana-runtime.md) — if
   you're picking up the runtime piece.
5. Pick one product line and read its `client.rs` + `tool.rs` together —
   `spot` is the cleanest example; `perps` is the gnarliest (Hyperliquid
   wire-format quirks).
