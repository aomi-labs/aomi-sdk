use crate::client::*;
use aomi_sdk::*;
use serde_json::{Value, json};

impl DynAomiTool for GetLifiSwapQuote {
    type App = LifiApp;
    type Args = GetLifiSwapQuoteArgs;
    const NAME: &'static str = "get_lifi_swap_quote";
    const DESCRIPTION: &'static str = "Get a LI.FI swap quote for same-chain or cross-chain swaps.";

    fn run(_app: &LifiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LifiClient::new()?;
        let (chain_name, _) = get_chain_info(&args.chain)?;
        let from_decimals = get_token_decimals(chain_name, &args.sell_token);
        let amount_base_units = amount_to_base_units(args.amount, from_decimals)?;
        let from_addr = get_token_address(chain_name, &args.sell_token)?;

        let destination_chain = args
            .destination_chain
            .as_deref()
            .unwrap_or(args.chain.as_str());
        let (to_chain_name, _) = get_chain_info(destination_chain)?;
        let to_addr = get_token_address(to_chain_name, &args.buy_token)?;

        client.get_quote(
            &args.chain,
            destination_chain,
            &from_addr,
            &to_addr,
            &amount_base_units,
            &args.sender_address,
            args.receiver_address.as_deref(),
        )
    }
}

impl DynAomiTool for PlaceLifiOrder {
    type App = LifiApp;
    type Args = PlaceLifiOrderArgs;
    const NAME: &'static str = "place_lifi_order";
    const DESCRIPTION: &'static str = "Get executable tx data via LI.FI. Returns approval_tx (if needed) and main_tx. Use the host's encode_and_simulate and send_transaction_to_wallet tools for execution.";

    fn run(_app: &LifiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LifiClient::new()?;
        let (chain_name, _) = get_chain_info(&args.chain)?;
        let from_decimals = get_token_decimals(chain_name, &args.sell_token);
        let amount_base_units = amount_to_base_units(args.amount, from_decimals)?;
        let from_addr = get_token_address(chain_name, &args.sell_token)?;

        let to_chain = args
            .destination_chain
            .clone()
            .unwrap_or_else(|| args.chain.clone());
        let (to_chain_name, _) = get_chain_info(&to_chain)?;
        let to_addr = get_token_address(to_chain_name, &args.buy_token)?;

        let payload = client.place_order(
            &args.chain,
            &to_chain,
            &from_addr,
            &to_addr,
            &amount_base_units,
            &args.sender_address,
            args.receiver_address.as_deref(),
            args.slippage,
        )?;

        Ok(json!({
            "source": "lifi",
            "payload": payload,
            "approval_tx": payload.get("approval_tx").cloned().unwrap_or(Value::Null),
            "main_tx": payload.get("main_tx").cloned().unwrap_or(Value::Null),
            "note": "If approval_tx is present, use the host's encode_and_simulate and send_transaction_to_wallet tools for the approval first, then do the same for main_tx.",
        }))
    }
}

impl DynAomiTool for GetLifiBridgeQuote {
    type App = LifiApp;
    type Args = GetLifiBridgeQuoteArgs;
    const NAME: &'static str = "get_lifi_bridge_quote";
    const DESCRIPTION: &'static str = "Get cross-chain bridge route with executable tx data via LI.FI. Returns executable bridge payload when available; otherwise planning-only estimate.";

    fn run(_app: &LifiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LifiClient::new()?;
        client.get_bridge_quote(
            &args.from_chain,
            &args.to_chain,
            &args.from_token,
            &args.to_token,
            args.amount,
            args.from_address.as_deref(),
            args.to_address.as_deref(),
            args.slippage_bps,
        )
    }
}

impl DynAomiTool for GetLifiTransferStatus {
    type App = LifiApp;
    type Args = GetLifiTransferStatusArgs;
    const NAME: &'static str = "get_lifi_transfer_status";
    const DESCRIPTION: &'static str = "Track the status of a cross-chain transfer by transaction hash. Returns status (NOT_FOUND, INVALID, PENDING, DONE, FAILED), substatus, and transaction details.";

    fn run(_app: &LifiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LifiClient::new()?;
        client.get_transfer_status(
            &args.tx_hash,
            args.from_chain.as_deref(),
            args.to_chain.as_deref(),
            args.bridge.as_deref(),
        )
    }
}

impl DynAomiTool for GetLifiChains {
    type App = LifiApp;
    type Args = GetLifiChainsArgs;
    const NAME: &'static str = "get_lifi_chains";
    const DESCRIPTION: &'static str = "List all chains supported by LI.FI. Optionally filter by chain type (e.g. EVM, SVM).";

    fn run(_app: &LifiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LifiClient::new()?;
        client.get_chains(args.chain_types.as_deref())
    }
}

impl DynAomiTool for GetLifiTokens {
    type App = LifiApp;
    type Args = GetLifiTokensArgs;
    const NAME: &'static str = "get_lifi_tokens";
    const DESCRIPTION: &'static str = "List supported tokens on LI.FI. Optionally filter by chain IDs (comma-separated) or chain type.";

    fn run(_app: &LifiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LifiClient::new()?;
        client.get_tokens(
            args.chains.as_deref(),
            args.chain_types.as_deref(),
        )
    }
}

impl DynAomiTool for GetLifiToken {
    type App = LifiApp;
    type Args = GetLifiTokenArgs;
    const NAME: &'static str = "get_lifi_token";
    const DESCRIPTION: &'static str = "Get detailed information for a single token including decimals and price.";

    fn run(_app: &LifiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LifiClient::new()?;
        client.get_token(&args.chain, &args.token)
    }
}

impl DynAomiTool for GetLifiRoutes {
    type App = LifiApp;
    type Args = GetLifiRoutesArgs;
    const NAME: &'static str = "get_lifi_routes";
    const DESCRIPTION: &'static str = "Get multiple route alternatives for a swap or bridge via LI.FI advanced routing. Compare routes by cost, speed, or safety. Use order_preference to sort: CHEAPEST, FASTEST, SAFEST, or RECOMMENDED.";

    fn run(_app: &LifiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LifiClient::new()?;
        client.get_routes(
            &args.from_chain,
            &args.to_chain,
            &args.from_token,
            &args.to_token,
            args.amount,
            &args.from_address,
            args.slippage,
            args.order_preference.as_deref(),
        )
    }
}

impl DynAomiTool for GetLifiStepTransaction {
    type App = LifiApp;
    type Args = GetLifiStepTransactionArgs;
    const NAME: &'static str = "get_lifi_step_transaction";
    const DESCRIPTION: &'static str = "Get executable transaction data for a single route step returned by get_lifi_routes. Pass the step object directly.";

    fn run(_app: &LifiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LifiClient::new()?;
        client.get_step_transaction(&args.step)
    }
}

impl DynAomiTool for GetLifiConnections {
    type App = LifiApp;
    type Args = GetLifiConnectionsArgs;
    const NAME: &'static str = "get_lifi_connections";
    const DESCRIPTION: &'static str = "Check available transfer pathways between chains and tokens on LI.FI.";

    fn run(_app: &LifiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LifiClient::new()?;
        client.get_connections(
            args.from_chain.as_deref(),
            args.to_chain.as_deref(),
            args.from_token.as_deref(),
            args.to_token.as_deref(),
        )
    }
}

impl DynAomiTool for GetLifiTools {
    type App = LifiApp;
    type Args = GetLifiToolsArgs;
    const NAME: &'static str = "get_lifi_tools";
    const DESCRIPTION: &'static str = "List available bridges and DEX exchanges on LI.FI. Optionally filter by chain IDs.";

    fn run(_app: &LifiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LifiClient::new()?;
        client.get_tools(args.chains.as_deref())
    }
}

impl DynAomiTool for GetLifiReverseQuote {
    type App = LifiApp;
    type Args = GetLifiReverseQuoteArgs;
    const NAME: &'static str = "get_lifi_reverse_quote";
    const DESCRIPTION: &'static str = "Get a quote by specifying the desired output amount (reverse quote). LI.FI calculates the required input amount.";

    fn run(_app: &LifiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LifiClient::new()?;
        client.get_reverse_quote(
            &args.from_chain,
            args.to_chain.as_deref(),
            &args.from_token,
            &args.to_token,
            &args.to_amount,
            &args.from_address,
            args.to_address.as_deref(),
        )
    }
}

impl DynAomiTool for GetLifiGasSuggestion {
    type App = LifiApp;
    type Args = GetLifiGasSuggestionArgs;
    const NAME: &'static str = "get_lifi_gas_suggestion";
    const DESCRIPTION: &'static str = "Get suggested gas amount for a destination chain. Useful for estimating gas needs for cross-chain transfers.";

    fn run(_app: &LifiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = LifiClient::new()?;
        client.get_gas_suggestion(
            &args.chain,
            args.from_chain.as_deref(),
            args.from_token.as_deref(),
        )
    }
}
