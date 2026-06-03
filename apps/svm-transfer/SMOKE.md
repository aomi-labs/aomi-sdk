# svm-transfer smoke runbook

End-to-end smoke for the SVM Lane 1 + Lane 2 pipeline on devnet. Moves a
small amount of SOL (0.001 by default) from your connected wallet to a
recipient address, exercising the full stack: tool registration → agent
dispatch → route plan → host pipeline finalize → wallet approval → real
broadcast → on-chain confirmation.

Why this app exists: the wired-up pieces (host pipeline, SDK markers,
wallet approval shape) all unit-test green, but nothing in the workspace
has actually pushed bytes through to chain on the new Lane 2 commit
path. This is that smoke.

## Prereqs

1. **Rust + cargo**, already there for this repo.
2. **Solana CLI** (`solana`) for keypair generation + balance checks.
3. **product-mono aomi-cli built**:
   ```bash
   cd /Users/cecilia/.codex/worktrees/2a6c/product-mono/aomi
   cargo build --bin aomi-cli
   ```
4. **A devnet keypair with some SOL**:
   ```bash
   solana-keygen new --outfile ~/.config/solana/smoke.json
   solana config set --url https://api.devnet.solana.com
   solana airdrop 1 --keypair ~/.config/solana/smoke.json
   # If the airdrop rate-limits, try https://faucet.solana.com or
   # https://faucet.quicknode.com/solana/devnet
   solana balance --keypair ~/.config/solana/smoke.json
   ```
5. **A recipient address** (any base58 pubkey, e.g. another keypair you
   own, or a random throwaway).

## One-time setup

```bash
# 1. Build the svm-transfer dylib (release for speed)
cd /Users/cecilia/Code/aomi-sdk/apps/svm-transfer
cargo build --release

# 2. Place the dylib in the aomi-cli plugins dir
mkdir -p /Users/cecilia/Code/aomi-apps/plugins
cp target/release/libsvm_transfer.dylib \
   /Users/cecilia/Code/aomi-apps/plugins/svm-transfer.dylib

# 3. Sanity-check the runtime can see it
AOMI_APPS_DIR=/Users/cecilia/Code/aomi-apps/plugins \
  /Users/cecilia/.codex/worktrees/2a6c/product-mono/aomi/target/debug/aomi-cli app list
# Expected: `svm-transfer` shows up.
```

## Smoke env

These three env vars drive the run:

```bash
# REQUIRED — the CLI's safety gate for any real signing
export FULL_TESTNETS=true

# REQUIRED — the SVM signer (JSON keypair file or inline JSON)
export SOLANA_KEYPAIR=$HOME/.config/solana/smoke.json

# REQUIRED — tell the smoke app which cluster to fetch a blockhash from
# (Lane 2 only; Lane 1 uses the host's configured cluster)
export SVM_TRANSFER_CLUSTER=devnet

# REQUIRED — where the dylib lives
export AOMI_APPS_DIR=/Users/cecilia/Code/aomi-apps/plugins
```

## Lane 1 smoke

The app composes one System Program transfer ix as JSON, host's
`svm_stage_ix` accepts the list, `svm_commit_ix({mode: wallet})`
composes the VersionedTransaction at commit time, wallet signs +
broadcasts.

```bash
cd /Users/cecilia/.codex/worktrees/2a6c/product-mono/aomi

# Start fresh
./target/debug/aomi-cli close

# 1. Chat — agent should run the confirmation gate (per the preamble),
#    then call transfer_sol_via_ix(amount_lamports="1000000", to=<dest>),
#    which emits the stage_ix → commit_ix route plan
./target/debug/aomi-cli --app svm-transfer --cluster devnet chat \
  "Transfer 0.001 SOL to <YOUR_RECIPIENT_BASE58> using lane 1"
# When the agent emits the confirmation summary, reply: confirm

# 2. Inspect the queued wallet request
./target/debug/aomi-cli tx list
# Expected: one pending SVM request with the assembled VersionedTransaction.

# 3. Sign + broadcast (this is the actual on-chain action)
./target/debug/aomi-cli tx sign all

# 4. Verify on-chain
solana confirm -v <SIGNATURE_FROM_STEP_3> --url https://api.devnet.solana.com
solana balance <YOUR_RECIPIENT_BASE58> --url https://api.devnet.solana.com
```

**What's being smoked:**
- aomi-cli loads the cdylib, registers `transfer_sol_via_ix`
- Agent sees the tool in the catalogue with the right preamble
- Tool emits a `__aomi_tool_routes` envelope with two routes:
  `svm_stage_ix` and `svm_commit_ix`
- Host's `StageOp<Svm>` parses the ix list, mints one `pending_ix_id`
- Host's `CommitTxsOp<Svm>` deserializes the wallet wire as Lane 1
  (`SvmCommitWire::Ix`), resolves the staged ix into an
  `AssembledSvmTx`, attaches a fresh blockhash, emits `SvmTxApproval`
- aomi-cli queues the approval
- `tx sign` signs with SOLANA_KEYPAIR, broadcasts via the SVM gateway

## Lane 2 smoke

The app fetches a recent blockhash from devnet RPC, builds the full
legacy VersionedTransaction client-side via solana-sdk, base64-encodes
it, and emits a `svm_stage_tx` → `svm_commit_tx({mode: wallet})` route
plan. Host's `svm_commit_tx` looks up the stored blob and surfaces it
into the wallet approval as-is.

```bash
# 1. Fresh session
./target/debug/aomi-cli close

# 2. Chat
./target/debug/aomi-cli --app svm-transfer --cluster devnet chat \
  "Transfer 0.001 SOL to <YOUR_RECIPIENT_BASE58> using lane 2"
# Reply: confirm

# 3. Inspect — Lane 2's queued request carries the *stored blob*, not a
#    composed bundle. svm_tx_id will be the single pending id.
./target/debug/aomi-cli tx list

# 4. Sign + broadcast
./target/debug/aomi-cli tx sign all

# 5. Verify
solana confirm -v <SIG> --url https://api.devnet.solana.com
```

**What's being smoked (NEW since the lane-symmetry split):**
- `transfer_sol_via_tx` builds a real `VersionedTransaction`, blob
  serializes via `bincode`, round-trips through host's
  `bincode::deserialize::<VersionedTransaction>` in `svm_stage_tx`
- Host's `StageOp<Svm>::finalize_stage_tx` dispatches to
  `finalize_svm_stage_tx` (by attribute name), stages as
  `SvmPending::Tx`
- Host's `CommitTxsOp<Svm>` deserializes the wallet wire as Lane 2
  (`SvmCommitWire::Tx`), looks up the stored blob, wraps it directly
  into `SvmTxApproval` — no ix assembly, no compute-budget
  detect-and-skip, no signer-set derivation needed beyond what's
  already in the blob
- Same wallet signing path as Lane 1 — only the producer side differs

## Negative-path checks

Worth running once to confirm error messages reach the user cleanly:

```bash
# No wallet
unset SOLANA_KEYPAIR
./target/debug/aomi-cli --app svm-transfer --cluster devnet chat \
  "lane 1 transfer 0.001 SOL to <DEST>"
# Expected error: "no SVM wallet connected — set SOLANA_KEYPAIR…"

# Zero amount
./target/debug/aomi-cli --app svm-transfer --cluster devnet chat \
  "lane 1 transfer 0 SOL to <DEST>"
# Expected error: "amount_lamports must be > 0"

# Invalid recipient
./target/debug/aomi-cli --app svm-transfer --cluster devnet chat \
  "lane 1 transfer 0.001 SOL to nope"
# Expected error: "invalid `to`: address `nope` is not a base58 pubkey…"

# Lane 2 cluster unreachable (point at a bad URL)
SVM_TRANSFER_CLUSTER=bogus_no_such_cluster \
  ./target/debug/aomi-cli --app svm-transfer --cluster devnet chat \
  "lane 2 transfer 0.001 SOL to <DEST>"
# The fallback inside SvmTransferApp falls through to devnet (intentional),
# so this won't fail today — confirms the default-to-devnet path.
```

## Expected costs

- Devnet SOL is free (faucet).
- Solana mainnet fees would be ~0.00001 SOL per tx; devnet runs the same
  pipeline at no real cost.
- aomi-cli runs locally, no remote backend round-trip.

## What this smoke doesn't cover

- **`internal-rpc` mode**: both commit tools still error loud on
  `mode: "internal-rpc"` until host #38-pipeline-c lands.
- **Bundle (Lane 3 / Jito)**: that's a separate pipeline (`svm-bundle`),
  not exercised here.
- **EVM**: this is SVM-only. EVM smoke is a separate flow in the
  `docs/topics/clients/facts/rust-cli.md` doc.
- **Cross-chain apps** (byreal-style): byreal uses `host::SvmSignTx`
  with venue submit, not the wallet-broadcast commit path. byreal's
  smoke is upstream of this one.

## If the smoke fails

Order of likely culprits, by experience from the lane-split work:

1. **AOMI_APPS_DIR points at the wrong path or the dylib isn't there.**
   `app list` resolves this immediately.
2. **SOLANA_KEYPAIR not exported in the same shell as `tx sign`.** The
   CLI keeps the signer in env only, not in session files.
3. **FULL_TESTNETS not set.** `tx sign` refuses to run without it.
4. **Devnet RPC ratelimits the blockhash fetch (Lane 2 only).** The
   error message will cite `api.devnet.solana.com` and a 429. Retry, or
   wait a minute.
5. **The agent picks the wrong lane.** The preamble names both tools
   explicitly; if the LLM still picks wrong, the route plan tools list
   in `tx list` will tell you which path actually ran.
