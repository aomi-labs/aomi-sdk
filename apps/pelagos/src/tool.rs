use crate::client::*;
use aomi_sdk::*;
use serde_json::{Value, json};

pub(crate) struct PelagosHealth;

impl DynAomiTool for PelagosHealth {
    type App = PelagosApp;
    type Args = HealthArgs;
    const NAME: &'static str = "pelagos_health";
    const DESCRIPTION: &'static str = "Check whether a Pelagos appchain node is reachable and healthy. Call this first to confirm the target is up before issuing any other tools.";

    fn run(_app: &PelagosApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let c = client_from(args.base_url.as_deref())?;
        let payload = c.health()?;
        Ok(json!({
            "base_url": c.base_url,
            "healthy": true,
            "response": payload,
        }))
    }
}

pub(crate) struct PelagosGetBalance;

impl DynAomiTool for PelagosGetBalance {
    type App = PelagosApp;
    type Args = GetBalanceArgs;
    const NAME: &'static str = "pelagos_get_balance";
    const DESCRIPTION: &'static str = "Query the on-chain token balance for a user account on a Pelagos appchain. Use this before suggesting a transfer to confirm the sender has sufficient funds.";

    fn run(_app: &PelagosApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let c = client_from(args.base_url.as_deref())?;
        let result = c.get_balance(&args.user, &args.token)?;
        Ok(json!({
            "base_url": c.base_url,
            "user": args.user,
            "token": args.token,
            "balance": result,
        }))
    }
}

pub(crate) struct PelagosTxStatus;

impl DynAomiTool for PelagosTxStatus {
    type App = PelagosApp;
    type Args = TxHashArgs;
    const NAME: &'static str = "pelagos_tx_status";
    const DESCRIPTION: &'static str = "Look up the lifecycle state of a Pelagos appchain transaction by hash. States progress: pending (in tx pool) -> batched (pulled by consensus) -> processed or failed. Use this after `pelagos_send` to track settlement.";

    fn run(_app: &PelagosApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let c = client_from(args.base_url.as_deref())?;
        let status = c.tx_status(&args.tx_hash)?;
        Ok(json!({
            "base_url": c.base_url,
            "tx_hash": args.tx_hash,
            "status": status,
        }))
    }
}

pub(crate) struct PelagosTxReceipt;

impl DynAomiTool for PelagosTxReceipt {
    type App = PelagosApp;
    type Args = TxHashArgs;
    const NAME: &'static str = "pelagos_tx_receipt";
    const DESCRIPTION: &'static str = "Fetch the finalized execution receipt for a settled Pelagos appchain transaction. Only available once the transaction has been processed; returns null for pending transactions.";

    fn run(_app: &PelagosApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let c = client_from(args.base_url.as_deref())?;
        let receipt = c.tx_receipt(&args.tx_hash)?;
        Ok(json!({
            "base_url": c.base_url,
            "tx_hash": args.tx_hash,
            "receipt": receipt,
        }))
    }
}

pub(crate) struct PelagosSend;

impl DynAomiTool for PelagosSend {
    type App = PelagosApp;
    type Args = SendArgs;
    const NAME: &'static str = "pelagos_send";
    const DESCRIPTION: &'static str = "Submit a token transfer transaction to a Pelagos appchain. Requires the user to explicitly confirm (`confirm: true`) before the transaction is submitted. After submission, use `pelagos_tx_status` to track settlement and `pelagos_tx_receipt` to confirm the outcome. Do NOT claim success until the receipt shows the transaction processed.";

    fn run(_app: &PelagosApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        if !args.confirm {
            return Err(
                "pelagos_send requires confirm=true; ask the user to confirm the transfer before proceeding"
                    .to_string(),
            );
        }

        if args.hash.is_empty() {
            return Err("hash field must not be empty".to_string());
        }
        if args.sender.is_empty() || args.receiver.is_empty() {
            return Err("sender and receiver must not be empty".to_string());
        }
        if args.token.is_empty() {
            return Err("token must not be empty".to_string());
        }

        let tx = json!({
            "sender": args.sender,
            "receiver": args.receiver,
            "value": args.value,
            "token": args.token,
            "hash": args.hash,
        });

        let c = client_from(args.base_url.as_deref())?;
        let result = c.send_transaction(&tx)?;

        Ok(json!({
            "base_url": c.base_url,
            "submitted": true,
            "transaction": tx,
            "result": result,
            "next_steps": [
                "Call pelagos_tx_status with the tx_hash to track settlement.",
                "Call pelagos_tx_receipt once status is processed to confirm the outcome.",
            ],
        }))
    }
}

pub(crate) struct PelagosRpc;

impl DynAomiTool for PelagosRpc {
    type App = PelagosApp;
    type Args = CallArgs;
    const NAME: &'static str = "pelagos_rpc";
    const DESCRIPTION: &'static str = "Call any appchain-specific JSON-RPC method that is not covered by the standard Pelagos tools. Use `pelagos_get_balance`, `pelagos_tx_status`, `pelagos_tx_receipt`, and `pelagos_send` for their respective standard methods. For state-changing custom methods, set `confirm: true` after explicit user approval.";

    fn run(_app: &PelagosApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        if is_dedicated_method(&args.method) {
            return Err(format!(
                "'{}' has a dedicated Pelagos tool - use that instead of pelagos_rpc",
                args.method
            ));
        }

        let params: Value = serde_json::from_str(&args.params_json)
            .map_err(|e| format!("params_json is not valid JSON: {e}"))?;

        if !params.is_array() {
            return Err(
                "params_json must be a JSON array, e.g. `[]` or `[{\"key\":\"val\"}]`".to_string(),
            );
        }

        if looks_mutating(&args.method) && !args.confirm.unwrap_or(false) {
            return Err(format!(
                "'{}' looks like a state-changing method; rerun with confirm=true after user confirmation",
                args.method
            ));
        }

        let c = client_from(args.base_url.as_deref())?;
        let result = c.call(&args.method, params.clone())?;

        Ok(json!({
            "base_url": c.base_url,
            "method": args.method,
            "params": params,
            "result": result,
        }))
    }
}
