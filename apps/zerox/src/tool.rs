use crate::client::*;
use aomi_sdk::*;
use serde::Serialize;
use serde_json::Value;

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[0x] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("0x".to_string()));
            Value::Object(map)
        }
        other => serde_json::json!({ "source": "0x", "data": other }),
    })
}

impl DynAomiTool for GetZeroxSwapQuote {
    type App = ZeroxApp;
    type Args = GetZeroxSwapQuoteArgs;
    const NAME: &'static str = "get_zerox_swap_quote";
    const DESCRIPTION: &'static str = "Get a 0x permit2/price swap quote for price discovery.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(ZeroxClient::new(args.api_key.as_deref())?.get_quote(
            &args.chain,
            &args.sell_token,
            &args.buy_token,
            args.amount,
            args.sender_address.as_deref(),
            args.slippage,
        )?)
    }
}

impl DynAomiTool for PlaceZeroxOrder {
    type App = ZeroxApp;
    type Args = PlaceZeroxOrderArgs;
    const NAME: &'static str = "place_zerox_order";
    const DESCRIPTION: &'static str = "Get executable tx data via 0x allowance-holder/quote. Returns a raw transaction payload (to, data, value) that the host should stage with `stage_tx` using `data.raw`, verify with `simulate_batch`, then finalize with `commit_tx`.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let quote = ZeroxClient::new(args.api_key.as_deref())?.place_order(
            &args.chain,
            &args.sell_token,
            &args.buy_token,
            args.amount,
            &args.sender_address,
            args.slippage,
        )?;

        let tx = serde_json::to_value(
            quote
                .transaction
                .as_ref()
                .ok_or_else(|| "0x response missing transaction payload".to_string())?,
        )
        .map_err(|e| format!("failed to encode 0x transaction payload: {e}"))?;

        ok(serde_json::json!({
            "quote": quote,
            "transaction": tx,
            "note": "Stage this raw 0x transaction with stage_tx using data.raw, verify the staged pending_tx_id with simulate_batch, then call commit_tx. Do not re-encode 0x calldata.",
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

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(ZeroxClient::new(args.api_key.as_deref())?.get_swap_chains()?)
    }
}

impl DynAomiTool for GetZeroxAllowanceHolderPrice {
    type App = ZeroxApp;
    type Args = GetZeroxAllowanceHolderPriceArgs;
    const NAME: &'static str = "get_zerox_allowance_holder_price";
    const DESCRIPTION: &'static str = "Get a 0x allowance-holder/price quote for price discovery. Matches the AllowanceHolder execution path so the price reflects actual execution costs.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(ZeroxClient::new(args.api_key.as_deref())?.get_allowance_holder_price(
            &args.chain,
            &args.sell_token,
            &args.buy_token,
            args.amount,
            args.sender_address.as_deref(),
            args.slippage,
        )?)
    }
}

impl DynAomiTool for GetZeroxLiquiditySources {
    type App = ZeroxApp;
    type Args = GetZeroxLiquiditySourcesArgs;
    const NAME: &'static str = "get_zerox_liquidity_sources";
    const DESCRIPTION: &'static str =
        "List available DEXs and AMMs (liquidity sources) on a given chain.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(ZeroxClient::new(args.api_key.as_deref())?.get_liquidity_sources(&args.chain)?)
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
        ok(ZeroxClient::new(args.api_key.as_deref())?.get_gasless_price(
            &args.chain,
            &args.sell_token,
            &args.buy_token,
            args.amount,
            args.sender_address.as_deref(),
            args.slippage,
        )?)
    }
}

impl DynAomiTool for GetZeroxGaslessQuote {
    type App = ZeroxApp;
    type Args = GetZeroxGaslessQuoteArgs;
    const NAME: &'static str = "get_zerox_gasless_quote";
    const DESCRIPTION: &'static str = "Get a gasless swap quote with EIP-712 typed data for signing. Returns approval (optional) and trade objects that the user must sign before submitting.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(ZeroxClient::new(args.api_key.as_deref())?.get_gasless_quote(
            &args.chain,
            &args.sell_token,
            &args.buy_token,
            args.amount,
            &args.sender_address,
            args.slippage,
        )?)
    }
}

impl DynAomiTool for SubmitZeroxGaslessSwap {
    type App = ZeroxApp;
    type Args = SubmitZeroxGaslessSwapArgs;
    const NAME: &'static str = "submit_zerox_gasless_swap";
    const DESCRIPTION: &'static str = "Submit a signed gasless trade (and optional approval) to the 0x relayer. Returns a tradeHash for status polling.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(ZeroxClient::new(args.api_key.as_deref())?.submit_gasless_swap(
            args.chain_id,
            &args.trade,
            args.approval.as_ref(),
        )?)
    }
}

impl DynAomiTool for GetZeroxGaslessStatus {
    type App = ZeroxApp;
    type Args = GetZeroxGaslessStatusArgs;
    const NAME: &'static str = "get_zerox_gasless_status";
    const DESCRIPTION: &'static str = "Poll the status of a gasless trade by tradeHash. Status progresses: pending -> succeeded -> confirmed.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(ZeroxClient::new(args.api_key.as_deref())?
            .get_gasless_status(&args.trade_hash, args.chain_id)?)
    }
}

impl DynAomiTool for GetZeroxGaslessChains {
    type App = ZeroxApp;
    type Args = GetZeroxGaslessChainsArgs;
    const NAME: &'static str = "get_zerox_gasless_chains";
    const DESCRIPTION: &'static str = "List all chains that support gasless swaps via the 0x API.";

    fn run(_app: &ZeroxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(ZeroxClient::new(args.api_key.as_deref())?.get_gasless_chains()?)
    }
}
