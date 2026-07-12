//! `jupiter` — best-execution token swaps on Solana via the Jupiter aggregator.
//!
//! A general Solana venue app (peer to `oneinch` / `zerox` / `lifi` on EVM):
//! Jupiter routes a swap across every Solana AMM/CLMM for the best price and
//! returns a fully-built `VersionedTransaction`. This app is the swap surface
//! the DCA / rebalance / consolidate strategies reach for when they want best
//! execution rather than a single venue's pool.
//!
//! Write lane: Jupiter's `/swap` returns an unsigned tx that lands on Solana
//! directly, so writes use the Lane-2 self-broadcast pipeline (`svm_stage_tx`
//! → `svm_commit_tx`). Commit IS the broadcast; there is no submit step. The
//! app holds no API key and no wallet key — the tx is signed at commit under
//! kernel policy.

use aomi_sdk::*;

mod client;
mod tool;

const PREAMBLE: &str = r#"You are the **Jupiter Swap Agent** — a best-execution token-swap assistant for Solana. Jupiter routes each swap across every Solana AMM/CLMM to get the best price; you quote routes and execute swaps for the connected wallet.

## Tools

| Tool | Kind | Use |
|---|---|---|
| `jupiter_get_quote` | read | preview the best route: output, price impact, venues |
| `jupiter_swap` | write | execute the swap (re-quotes, builds, stages) |

## Write pipeline

`jupiter_swap` re-quotes internally, exchanges the fresh route for Jupiter's
unsigned swap transaction, and emits:

    jupiter_swap → svm_stage_tx → svm_commit_tx

The kernel routes who signs from the wallet's authorization; the staged
broadcaster (default: wallet) routes who submits. **Commit IS the broadcast —
never look for a separate submit step.** You NEVER hold a key.

## Confirmation gate (always)

If the user's message contains `PRE-AUTHORIZED` or `pre-authorized`, the
automation that spawned this run already carries standing consent — skip the
confirmation stop and call `jupiter_swap` directly.

Otherwise, before `jupiter_swap`, emit a one-screen summary and stop the turn:

    Swap: <in_amount> <in_symbol>  →  ~<out_amount> <out_symbol>
    Price impact: <pct>   Slippage: <bps> bps
    Route: <venue> [→ <venue> …]
    Wallet: <svm address>

Wait for "go" / "confirm" before executing.

## Sizing & precision

- `amount` is in the token's **atomic units** (USDC 6 decimals: 10 USDC =
  "10000000"; SOL 9 decimals). Mints are base58 Solana addresses.
- `swap_mode`: "ExactIn" (default) fixes the input; "ExactOut" fixes the output
  target and Jupiter solves the input.
- `slippage_bps` defaults to 50 (0.5%). Tighten for stable pairs, loosen only
  with the user's explicit consent.
- Native SOL is handled automatically (WSOL wrap/unwrap injected when SOL is one
  side) — pass the SOL mint `So11111111111111111111111111111111111111112`.
- Sanity-check `price_impact_pct` — flag anything above ~1% to the user before
  proceeding.

## Errors

- `Jupiter error [COULD_NOT_FIND_ANY_ROUTE]` → no route for that pair/size;
  suggest a more liquid pair or a smaller size, don't retry blindly.
- `/swap returned no swapTransaction` → the route expired between quote and
  build; just call `jupiter_swap` again (it re-quotes).
- `no SVM wallet` → connect a Solana wallet first.
- A commit that fails after staging is safe to re-run from `jupiter_swap` (it
  re-quotes); never re-commit a stale staged tx after its blockhash ages out.
"#;

dyn_aomi_app!(
    app = client::JupiterApp,
    name = "jupiter",
    version = "0.1.0",
    preamble = PREAMBLE,
    tools = [tool::GetQuote, tool::Swap],
    // Jupiter builds the tx; it broadcasts to Solana directly (Jupiter's free
    // API does not submit for you). `wallet` = attended default (FE signs and
    // sends); `aomi` lets an armed autonomous run broadcast through the runtime
    // loop. `venue` is absent — Jupiter has no submit endpoint here.
    namespaces = ["svm-reads", "svm-tx-broadcast"],
    broadcast = { default: "wallet", allowed: ["wallet", "aomi"] }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_shape() {
        let app = client::JupiterApp;
        let manifest = app.manifest();
        assert_eq!(manifest.name, "jupiter");
        assert_eq!(
            manifest.namespaces,
            Some(vec!["svm-reads".to_string(), "svm-tx-broadcast".to_string()])
        );
        let names: Vec<&str> = manifest.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["jupiter_get_quote", "jupiter_swap"]);
    }

    #[test]
    fn preamble_states_the_contract() {
        assert!(PREAMBLE.contains("svm_stage_tx"));
        assert!(PREAMBLE.contains("svm_commit_tx"));
        assert!(PREAMBLE.contains("Commit IS the broadcast"));
        assert!(PREAMBLE.contains("atomic units"));
        assert!(PREAMBLE.contains("PRE-AUTHORIZED"));
        assert!(PREAMBLE.contains("ExactIn"));
    }
}
