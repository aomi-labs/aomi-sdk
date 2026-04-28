use crate::client::*;
use aomi_sdk::*;
use serde_json::Value;

impl DynAomiTool for GetOneInchQuote {
    type App = OneInchApp;
    type Args = GetOneInchQuoteArgs;
    const NAME: &'static str = "get_oneinch_quote";
    const DESCRIPTION: &'static str = "Get a 1inch swap quote for price discovery (no transaction data). Returns optimal routing across DEXs.";

    fn run(_app: &OneInchApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OneInchClient::new(args.api_key.as_deref())?;
        let chain_id = args.chain_id.unwrap_or(1);
        client.get_quote(
            chain_id,
            &args.src,
            &args.dst,
            &args.amount,
            args.protocols.as_deref(),
        )
    }
}

impl DynAomiTool for GetOneInchSwap {
    type App = OneInchApp;
    type Args = GetOneInchSwapArgs;
    const NAME: &'static str = "get_oneinch_swap";
    const DESCRIPTION: &'static str = "Get a 1inch swap quote with executable transaction calldata. Returns a raw tx object (to, data, value, gas) that the host should stage with `stage_tx` using `data.raw`, verify with `simulate_batch`, then finalize with `commit_tx`.";

    fn run(_app: &OneInchApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OneInchClient::new(args.api_key.as_deref())?;
        let chain_id = args.chain_id.unwrap_or(1);
        client.get_swap(
            chain_id,
            &args.src,
            &args.dst,
            &args.amount,
            &args.from,
            args.slippage,
            args.protocols.as_deref(),
        )
    }
}

impl DynAomiTool for GetOneInchApproveTransaction {
    type App = OneInchApp;
    type Args = GetOneInchApproveTransactionArgs;
    const NAME: &'static str = "get_oneinch_approve_transaction";
    const DESCRIPTION: &'static str = "Get transaction data to approve the 1inch router to spend a token. Returns a raw approval tx object (to, data, value). Omit amount for unlimited approval. Stage it directly with `stage_tx` using `data.raw`; do not re-encode calldata.";

    fn run(_app: &OneInchApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OneInchClient::new(args.api_key.as_deref())?;
        let chain_id = args.chain_id.unwrap_or(1);
        client.get_approve_transaction(chain_id, &args.token_address, args.amount.as_deref())
    }
}

impl DynAomiTool for GetOneInchAllowance {
    type App = OneInchApp;
    type Args = GetOneInchAllowanceArgs;
    const NAME: &'static str = "get_oneinch_allowance";
    const DESCRIPTION: &'static str = "Check the current allowance the 1inch router has for a token from a given wallet. Returns the allowance amount.";

    fn run(_app: &OneInchApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OneInchClient::new(args.api_key.as_deref())?;
        let chain_id = args.chain_id.unwrap_or(1);
        client.get_allowance(chain_id, &args.token_address, &args.wallet_address)
    }
}

impl DynAomiTool for GetOneInchLiquiditySources {
    type App = OneInchApp;
    type Args = GetOneInchLiquiditySourcesArgs;
    const NAME: &'static str = "get_oneinch_liquidity_sources";
    const DESCRIPTION: &'static str =
        "List available DEXs and AMMs (liquidity sources) on a given chain for 1inch routing.";

    fn run(_app: &OneInchApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OneInchClient::new(args.api_key.as_deref())?;
        let chain_id = args.chain_id.unwrap_or(1);
        client.get_liquidity_sources(chain_id)
    }
}

impl DynAomiTool for GetOneInchTokens {
    type App = OneInchApp;
    type Args = GetOneInchTokensArgs;
    const NAME: &'static str = "get_oneinch_tokens";
    const DESCRIPTION: &'static str = "List all supported tokens on a given chain. Returns token addresses, symbols, decimals, and logos.";

    fn run(_app: &OneInchApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OneInchClient::new(args.api_key.as_deref())?;
        let chain_id = args.chain_id.unwrap_or(1);
        client.get_tokens(chain_id)
    }
}
