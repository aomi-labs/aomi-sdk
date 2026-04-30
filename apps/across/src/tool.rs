use crate::client::*;
use aomi_sdk::*;
use serde::Serialize;
use serde_json::Value;

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[across] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("across".to_string()));
            Value::Object(map)
        }
        other => serde_json::json!({ "source": "across", "data": other }),
    })
}

impl DynAomiTool for GetAcrossBridgeQuote {
    type App = AcrossApp;
    type Args = GetAcrossBridgeQuoteArgs;
    const NAME: &'static str = "get_across_bridge_quote";
    const DESCRIPTION: &'static str = "Get a bridge fee quote from Across Protocol. Returns suggested fees, estimated fill time, and fee breakdown for a cross-chain transfer.";

    fn run(_app: &AcrossApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(AcrossClient::new()?.get_suggested_fees(
            &args.input_token,
            &args.output_token,
            args.origin_chain_id,
            args.destination_chain_id,
            &args.amount,
            args.recipient.as_deref(),
            args.message.as_deref(),
        )?)
    }
}

impl DynAomiTool for GetAcrossBridgeLimits {
    type App = AcrossApp;
    type Args = GetAcrossBridgeLimitsArgs;
    const NAME: &'static str = "get_across_bridge_limits";
    const DESCRIPTION: &'static str =
        "Get minimum and maximum transfer limits for a specific token route on Across Protocol.";

    fn run(_app: &AcrossApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(AcrossClient::new()?.get_limits(
            &args.input_token,
            &args.output_token,
            args.origin_chain_id,
            args.destination_chain_id,
        )?)
    }
}

impl DynAomiTool for GetAcrossDepositStatus {
    type App = AcrossApp;
    type Args = GetAcrossDepositStatusArgs;
    const NAME: &'static str = "get_across_deposit_status";
    const DESCRIPTION: &'static str = "Track the status of a bridge deposit on Across Protocol. Returns fill status and corresponding fill transaction hash if filled.";

    fn run(_app: &AcrossApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(AcrossClient::new()?.get_deposit_status(args.origin_chain_id, args.deposit_id)?)
    }
}

impl DynAomiTool for GetAcrossAvailableRoutes {
    type App = AcrossApp;
    type Args = GetAcrossAvailableRoutesArgs;
    const NAME: &'static str = "get_across_available_routes";
    const DESCRIPTION: &'static str = "List available bridge routes on Across Protocol. Optionally filter by origin/destination chain ID or token address.";

    fn run(_app: &AcrossApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(AcrossClient::new()?.get_available_routes(
            args.origin_chain_id,
            args.destination_chain_id,
            args.origin_token.as_deref(),
            args.destination_token.as_deref(),
        )?)
    }
}

impl DynAomiTool for GetAcrossTokenPrice {
    type App = AcrossApp;
    type Args = GetAcrossTokenPriceArgs;
    const NAME: &'static str = "get_across_token_price";
    const DESCRIPTION: &'static str = "Get token price from Across Protocol's coingecko endpoint. Provide either an L1 or L2 token address.";

    fn run(_app: &AcrossApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        if args.l1_token.is_none() && args.l2_token.is_none() {
            return Err("[across] token price failed: at least one of l1_token or l2_token must be provided".to_string());
        }
        ok(AcrossClient::new()?
            .get_coingecko_price(args.l1_token.as_deref(), args.l2_token.as_deref())?)
    }
}
