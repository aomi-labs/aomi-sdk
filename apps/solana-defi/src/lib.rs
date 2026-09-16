use aomi_sdk::*;

mod client;
mod tool;

const PREAMBLE: &str = r#"Use Aomi's Solana DeFi recipes to inspect Jupiter Lend markets,
Kamino Earn vaults and PumpSwap liquidity pools, read wallet positions, and prepare
unsigned instructions. These tools never sign, broadcast, or authorize an action.

Start with solana_defi_venues, then solana_defi_market for the exact market address
and connected wallet. Example markets are discovery starting points, not endorsements.
Check the asset, market program, creation date, liquidity and applicable charges.
Use solana_defi_prepare to construct the action, preserving the returned ordered
instruction batches exactly. Deposit amount_raw uses the asset's smallest units;
PumpSwap uses the target quote contribution and returns both maximum inputs.
Withdrawals use underlying asset units for Jupiter and share units for Kamino/PumpSwap,
or withdraw_all=true. Do not confuse share units with underlying tokens.

Hand prepared instructions to the host's shared Solana executor for staging and
simulation. Show wallet, network, movements and limits before requesting confirmation.
Aomi-side protocol recipes use official SDKs; this does not imply arbitrary protocol
support. A passing simulation is not execution or an authorization grant. Inspect
the receipt and solana_defi_position after confirmed execution. Local mirror evidence
does not demonstrate production signing or realized returns.

Deployment requires the bundled preparation service, AOMI_SOLANA_DEFI_URL in the app
host, and a matching AOMI_SOLANA_RPC_URL in that service. No RPC or signing keys belong
in tool arguments. Do not change networks to recover from an unavailable provider.
"#;

dyn_aomi_app!(
    app = client::SolanaDefiApp,
    name = "solana-defi",
    version = "0.1.0",
    preamble = PREAMBLE,
    tools = [tool::Venues, tool::Market, tool::Prepare, tool::Position],
    namespaces = ["svm-reads"]
);
