use crate::client::*;
use aomi_sdk::*;
use serde_json::{Value, json};

impl DynAomiTool for GetZeroxSwapQuote {
    type App = ZeroxApp;
    type Args = GetZeroxSwapQuoteArgs;
    const NAME: &'static str = "get_zerox_swap_quote";
    const DESCRIPTION: &'static str = "Get a 0x permit2/price swap quote for price discovery.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = ZeroxClient::new()?;
        client.get_quote(
            &args.chain,
            &args.sell_token,
            &args.buy_token,
            args.amount,
            args.sender_address.as_deref(),
            args.slippage,
        )
    }
}

impl DynAomiTool for PlaceZeroxOrder {
    type App = ZeroxApp;
    type Args = PlaceZeroxOrderArgs;
    const NAME: &'static str = "place_zerox_order";
    const DESCRIPTION: &'static str = "Get executable tx data via 0x allowance-holder/quote. Returns transaction data (to, data, value) that the host should verify with `encode_and_simulate` and send with `send_transaction_to_wallet`.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = ZeroxClient::new()?;
        let quote = client.place_order(
            &args.chain,
            &args.sell_token,
            &args.buy_token,
            args.amount,
            &args.sender_address,
            args.slippage,
        )?;

        let tx = quote
            .get("transaction")
            .cloned()
            .ok_or_else(|| "0x response missing transaction payload".to_string())?;

        Ok(json!({
            "source": "0x",
            "quote": quote,
            "transaction": tx,
            "note": "Use the host's encode_and_simulate tool to verify this transaction, then use send_transaction_to_wallet to execute it.",
        }))
    }
}

// ============================================================================
// High Priority tools
// ============================================================================

impl DynAomiTool for GetZeroxSwapChains {
    type App = ZeroxApp;
    type Args = GetZeroxSwapChainsArgs;
    const NAME: &'static str = "get_zerox_swap_chains";
    const DESCRIPTION: &'static str =
        "List all chains supported by the 0x Swap API. Returns an array of { chainName, chainId }.";

    fn run(_app: &ZeroxApp, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = ZeroxClient::new()?;
        client.get_swap_chains()
    }
}

impl DynAomiTool for GetZeroxAllowanceHolderPrice {
    type App = ZeroxApp;
    type Args = GetZeroxAllowanceHolderPriceArgs;
    const NAME: &'static str = "get_zerox_allowance_holder_price";
    const DESCRIPTION: &'static str = "Get a 0x allowance-holder/price quote for price discovery. Matches the AllowanceHolder execution path so the price reflects actual execution costs.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = ZeroxClient::new()?;
        client.get_allowance_holder_price(
            &args.chain,
            &args.sell_token,
            &args.buy_token,
            args.amount,
            args.sender_address.as_deref(),
            args.slippage,
        )
    }
}

impl DynAomiTool for GetZeroxLiquiditySources {
    type App = ZeroxApp;
    type Args = GetZeroxLiquiditySourcesArgs;
    const NAME: &'static str = "get_zerox_liquidity_sources";
    const DESCRIPTION: &'static str =
        "List available DEXs and AMMs (liquidity sources) on a given chain.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = ZeroxClient::new()?;
        client.get_liquidity_sources(&args.chain)
    }
}

// ============================================================================
// Gasless tools
// ============================================================================

impl DynAomiTool for GetZeroxGaslessPrice {
    type App = ZeroxApp;
    type Args = GetZeroxGaslessPriceArgs;
    const NAME: &'static str = "get_zerox_gasless_price";
    const DESCRIPTION: &'static str = "Get a gasless swap price quote from 0x. The sell token must be an ERC-20 token (not native ETH/MATIC/etc.).";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = ZeroxClient::new()?;
        client.get_gasless_price(
            &args.chain,
            &args.sell_token,
            &args.buy_token,
            args.amount,
            args.sender_address.as_deref(),
            args.slippage,
        )
    }
}

impl DynAomiTool for GetZeroxGaslessQuote {
    type App = ZeroxApp;
    type Args = GetZeroxGaslessQuoteArgs;
    const NAME: &'static str = "get_zerox_gasless_quote";
    const DESCRIPTION: &'static str = "Get a gasless swap quote with EIP-712 typed data for signing. Returns approval (optional) and trade objects that the user must sign before submitting.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = ZeroxClient::new()?;
        client.get_gasless_quote(
            &args.chain,
            &args.sell_token,
            &args.buy_token,
            args.amount,
            &args.sender_address,
            args.slippage,
        )
    }
}

impl DynAomiTool for SubmitZeroxGaslessSwap {
    type App = ZeroxApp;
    type Args = SubmitZeroxGaslessSwapArgs;
    const NAME: &'static str = "submit_zerox_gasless_swap";
    const DESCRIPTION: &'static str = "Submit a signed gasless trade (and optional approval) to the 0x relayer. Returns a tradeHash for status polling.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = ZeroxClient::new()?;
        client.submit_gasless_swap(args.chain_id, &args.trade, args.approval.as_ref())
    }
}

impl DynAomiTool for GetZeroxGaslessStatus {
    type App = ZeroxApp;
    type Args = GetZeroxGaslessStatusArgs;
    const NAME: &'static str = "get_zerox_gasless_status";
    const DESCRIPTION: &'static str = "Poll the status of a gasless trade by tradeHash. Status progresses: pending -> succeeded -> confirmed.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = ZeroxClient::new()?;
        client.get_gasless_status(&args.trade_hash, args.chain_id)
    }
}

impl DynAomiTool for GetZeroxGaslessChains {
    type App = ZeroxApp;
    type Args = GetZeroxGaslessChainsArgs;
    const NAME: &'static str = "get_zerox_gasless_chains";
    const DESCRIPTION: &'static str = "List all chains that support gasless swaps via the 0x API.";

    fn run(_app: &ZeroxApp, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = ZeroxClient::new()?;
        client.get_gasless_chains()
    }
}
