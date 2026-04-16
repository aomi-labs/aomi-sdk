#[path = "../client.rs"]
mod client;
#[path = "../tool.rs"]
mod tool;

use alloy::signers::{Signer, SignerSync, local::PrivateKeySigner};
use alloy_dyn_abi::eip712::TypedData;
use aomi_sdk::{DynAomiApp, DynAomiTool, DynAsyncSink, DynToolCallCtx, DynToolDispatch, DynToolMetadata};
use polymarket_client_sdk::clob::types::request::BalanceAllowanceRequest;
use polymarket_client_sdk::clob::types::{AssetType, SignatureType};
use polymarket_client_sdk::clob::{Client as SdkClobClient, Config as SdkClobConfig};
use polymarket_client_sdk::{POLYGON, PRIVATE_KEY_VAR};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

impl DynAomiApp for client::PolymarketRewardsApp {
    fn name(&self) -> &'static str {
        "polymarket-rewards-harness"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn preamble(&self) -> &'static str {
        ""
    }

    fn tools(&self) -> Vec<DynToolMetadata> {
        Vec::new()
    }

    fn start_tool(
        &self,
        _name: &str,
        _args_json: &str,
        _ctx_json: &str,
        _sink: DynAsyncSink,
    ) -> DynToolDispatch {
        panic!("rewards_harness does not support dynamic dispatch")
    }
}

fn main() {
    if let Err(error) = run_main() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run_main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or_else(usage)?;
    let parsed = parse_flags(args.collect::<Vec<_>>())?;

    match command.as_str() {
        "preview" => {
            let result = build_preview_result(
                required(&parsed, "condition-id")?,
                parse_f64(required(&parsed, "capital")?, "capital")?,
            )?;
            print_json(&result)?;
        }
        "start" => {
            let address = required(&parsed, "address")?.to_string();
            let condition_id = required(&parsed, "condition-id")?.to_string();
            let capital = parse_f64(required(&parsed, "capital")?, "capital")?;
            let state_path = state_file(&parsed)?;

            let preview = build_preview_result(&condition_id, capital)?;
            let submit_template = preview
                .get("submit_args_template")
                .cloned()
                .ok_or_else(|| "build_quote_plan did not return submit_args_template".to_string())?;

            let mut submit_args = submit_template;
            let submit_obj = submit_args
                .as_object_mut()
                .ok_or_else(|| "submit_args_template was not an object".to_string())?;
            submit_obj.insert("address".to_string(), Value::String(address));
            if let Some(question) = preview.get("market_question").and_then(Value::as_str) {
                submit_obj.insert(
                    "market_question".to_string(),
                    Value::String(question.to_string()),
                );
            }

            let result = run_submit(submit_args)?;
            persist_next_state(&state_path, &result)?;
            print_json(&json!({
                "preview": preview,
                "result": result,
                "state_file": state_path,
            }))?;
        }
        "resume" => {
            let state_path = state_file(&parsed)?;
            let mut submit_args = load_state(&state_path)?;

            if let Some(signature) = parsed.values.get("clob-l1-signature") {
                let obj = submit_args
                    .as_object_mut()
                    .ok_or_else(|| "state file did not contain an object".to_string())?;
                obj.insert(
                    "clob_l1_signature".to_string(),
                    Value::String(signature.clone()),
                );
            }
            if let Some(signature) = parsed.values.get("yes-bid-signature") {
                let obj = submit_args
                    .as_object_mut()
                    .ok_or_else(|| "state file did not contain an object".to_string())?;
                obj.insert(
                    "yes_bid_signature".to_string(),
                    Value::String(signature.clone()),
                );
            }
            if let Some(signature) = parsed.values.get("no-bid-signature") {
                let obj = submit_args
                    .as_object_mut()
                    .ok_or_else(|| "state file did not contain an object".to_string())?;
                obj.insert(
                    "no_bid_signature".to_string(),
                    Value::String(signature.clone()),
                );
            }
            if parsed.flags.contains("simulation-confirmed") {
                let obj = submit_args
                    .as_object_mut()
                    .ok_or_else(|| "state file did not contain an object".to_string())?;
                obj.insert("simulation_confirmed".to_string(), Value::Bool(true));
            }

            let result = run_submit(submit_args)?;
            persist_next_state(&state_path, &result)?;
            print_json(&json!({
                "result": result,
                "state_file": state_path,
            }))?;
        }
        "preflight" => {
            let signer = load_env_signer()?;
            let address = format!("{:#x}", signer.address());
            let config = SdkClobConfig::builder().use_server_time(true).build();
            let client = SdkClobClient::new("https://clob.polymarket.com", config)
                .map_err(|e| format!("failed to create SDK client: {e}"))?;
            let authed = client::TOKIO_RT
                .block_on(client.authentication_builder(&signer).authenticate())
                .map_err(|e| format!("failed to authenticate SDK client: {e}"))?;
            let balance_allowance = client::TOKIO_RT
                .block_on(authed.balance_allowance(
                    BalanceAllowanceRequest::builder()
                        .asset_type(AssetType::Collateral)
                        .signature_type(SignatureType::Eoa)
                        .build(),
                ))
                .map_err(|e| format!("failed to fetch balance/allowance: {e}"))?;
            print_json(&json!({
                "address": address,
                "balance": balance_allowance.balance.to_string(),
                "allowances": balance_allowance.allowances,
            }))?;
        }
        "open-orders" => {
            let signer = load_env_signer()?;
            let creds = derive_creds_for_signer(&signer)?;
            let asset_id = parsed.values.get("asset-id").map(String::as_str);
            let result = tool::GetQuotePlanStatus::run(
                &client::PolymarketRewardsApp,
                client::GetQuotePlanStatusArgs {
                    address: creds.address.clone(),
                    api_key: creds.api_key,
                    api_secret: creds.api_secret,
                    passphrase: creds.passphrase,
                    signature_type: creds.signature_type,
                    funder: creds.funder,
                    asset_id: asset_id.map(ToOwned::to_owned),
                    include_earnings: Some(false),
                },
                tool_ctx("get_quote_plan_status"),
            )?;
            print_json(&result)?;
        }
        "cancel-open" => {
            let signer = load_env_signer()?;
            let creds = derive_creds_for_signer(&signer)?;
            let condition_id = parsed.values.get("condition-id").cloned();
            let asset_id = parsed.values.get("asset-id").cloned();
            let all = parsed.flags.contains("all");
            if !all && condition_id.is_none() && asset_id.is_none() {
                return Err("cancel-open requires --condition-id, --asset-id, or --all".to_string());
            }

            let result = if all {
                let client = client::PolymarketRewardsClient::new()?;
                let open_orders = client.fetch_open_orders(&creds, None)?;
                let order_ids = open_orders
                    .iter()
                    .filter_map(|order| {
                        order
                            .get("id")
                            .or_else(|| order.get("order_id"))
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    })
                    .collect::<Vec<_>>();
                tool::WithdrawQuoteLiquidity::run(
                    &client::PolymarketRewardsApp,
                    client::WithdrawQuoteLiquidityArgs {
                        confirmation: Some("confirm".to_string()),
                        address: creds.address.clone(),
                        api_key: creds.api_key,
                        api_secret: creds.api_secret,
                        passphrase: creds.passphrase,
                        signature_type: creds.signature_type,
                        funder: creds.funder,
                        order_ids: Some(order_ids),
                        condition_id: None,
                        asset_id: None,
                        simulate: Some(false),
                    },
                    tool_ctx("withdraw_quote_liquidity"),
                )?
            } else {
                tool::WithdrawQuoteLiquidity::run(
                    &client::PolymarketRewardsApp,
                    client::WithdrawQuoteLiquidityArgs {
                        confirmation: Some("confirm".to_string()),
                        address: creds.address.clone(),
                        api_key: creds.api_key,
                        api_secret: creds.api_secret,
                        passphrase: creds.passphrase,
                        signature_type: creds.signature_type,
                        funder: creds.funder,
                        order_ids: None,
                        condition_id,
                        asset_id,
                        simulate: Some(false),
                    },
                    tool_ctx("withdraw_quote_liquidity"),
                )?
            };
            print_json(&result)?;
        }
        "self-test-live" => {
            let requested_address = parsed.values.get("address").cloned();
            let condition_id = required(&parsed, "condition-id")?.to_string();
            let capital = parse_f64(required(&parsed, "capital")?, "capital")?;
            let state_path = state_file(&parsed)?;
            let cancel_after_submit = !parsed.flags.contains("no-cancel");

            let private_key = std::env::var("POLYMARKET_PRIVATE_KEY")
                .or_else(|_| std::env::var("POLYMARKET_TEST_PRIVATE_KEY"))
                .map_err(|_| {
                    "missing POLYMARKET_PRIVATE_KEY or POLYMARKET_TEST_PRIVATE_KEY in environment"
                        .to_string()
                })?;
            let signer = PrivateKeySigner::from_str(private_key.trim())
                .map_err(|e| format!("invalid POLYMARKET_PRIVATE_KEY: {e}"))?;
            let signer_address = format!("{:#x}", signer.address());

            if let Some(address) = requested_address.as_deref()
                && !address.eq_ignore_ascii_case(&signer_address)
            {
                return Err(format!(
                    "--address {address} does not match POLYMARKET_PRIVATE_KEY address {signer_address}"
                ));
            }

            let preview = build_preview_result(&condition_id, capital)?;
            let submit_template = preview
                .get("submit_args_template")
                .cloned()
                .ok_or_else(|| "build_quote_plan did not return submit_args_template".to_string())?;

            let mut submit_args = submit_template;
            let submit_obj = submit_args
                .as_object_mut()
                .ok_or_else(|| "submit_args_template was not an object".to_string())?;
            submit_obj.insert(
                "address".to_string(),
                Value::String(signer_address.clone()),
            );
            if let Some(question) = preview.get("market_question").and_then(Value::as_str) {
                submit_obj.insert(
                    "market_question".to_string(),
                    Value::String(question.to_string()),
                );
            }

            let stage1 = run_submit(submit_args)?;
            persist_next_state(&state_path, &stage1)?;
            let stage1_args = load_state(&state_path)?;
            let clob_auth = stage1_args
                .get("clob_auth")
                .cloned()
                .ok_or_else(|| "stage1 state missing clob_auth".to_string())?;
            let clob_auth_signature =
                sign_typed_data(&signer, &client::build_reward_clob_auth_typed_data(
                    &serde_json::from_value(clob_auth)
                        .map_err(|e| format!("invalid clob_auth state: {e}"))?,
                ))?;

            let mut stage2_args = stage1_args;
            let stage2_obj = stage2_args
                .as_object_mut()
                .ok_or_else(|| "stage1 state was not an object".to_string())?;
            stage2_obj.insert(
                "clob_l1_signature".to_string(),
                Value::String(clob_auth_signature),
            );

            let stage2 = run_submit(stage2_args)?;
            persist_next_state(&state_path, &stage2)?;
            let stage2_args = load_state(&state_path)?;
            let prepared_yes_bid_order: client::PreparedRewardOrder = serde_json::from_value(
                stage2_args
                    .get("prepared_yes_bid_order")
                    .cloned()
                    .ok_or_else(|| "stage2 state missing prepared_yes_bid_order".to_string())?,
            )
            .map_err(|e| format!("invalid prepared_yes_bid_order state: {e}"))?;
            let prepared_no_bid_order: client::PreparedRewardOrder = serde_json::from_value(
                stage2_args
                    .get("prepared_no_bid_order")
                    .cloned()
                    .ok_or_else(|| "stage2 state missing prepared_no_bid_order".to_string())?,
            )
            .map_err(|e| format!("invalid prepared_no_bid_order state: {e}"))?;

            let yes_bid_signature = sign_typed_data(
                &signer,
                &client::build_reward_order_typed_data(&prepared_yes_bid_order),
            )?;
            let no_bid_signature = sign_typed_data(
                &signer,
                &client::build_reward_order_typed_data(&prepared_no_bid_order),
            )?;

            let mut stage3_args = stage2_args;
            let stage3_obj = stage3_args
                .as_object_mut()
                .ok_or_else(|| "stage2 state was not an object".to_string())?;
            stage3_obj.insert(
                "yes_bid_signature".to_string(),
                Value::String(yes_bid_signature),
            );
            stage3_obj.insert(
                "no_bid_signature".to_string(),
                Value::String(no_bid_signature),
            );

            let stage3 = run_submit(stage3_args)?;
            persist_next_state(&state_path, &stage3)?;
            let mut final_args = load_state(&state_path)?;
            let final_obj = final_args
                .as_object_mut()
                .ok_or_else(|| "stage3 state was not an object".to_string())?;
            final_obj.insert("simulation_confirmed".to_string(), Value::Bool(true));

            let submit_live = run_submit(final_args)?;

            let mut canceled = None;
            if cancel_after_submit
                && submit_live
                    .get("stage")
                    .and_then(Value::as_str)
                    .is_some_and(|stage| stage == "submitted")
            {
                if let Some(order_ids) = extract_order_ids(&submit_live) {
                    let submit_state = submit_live
                        .get("submit_args_template")
                        .cloned()
                        .or_else(|| load_state(&state_path).ok())
                        .ok_or_else(|| "unable to recover submission state for cancellation".to_string())?;
                    let address = submit_state
                        .get("address")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "submission state missing address".to_string())?;
                    let clob_auth: client::ClobAuthContext = serde_json::from_value(
                        submit_state
                            .get("clob_auth")
                            .cloned()
                            .ok_or_else(|| "submission state missing clob_auth".to_string())?,
                    )
                    .map_err(|e| format!("invalid submission clob_auth state: {e}"))?;
                    let clob_l1_signature = submit_state
                        .get("clob_l1_signature")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "submission state missing clob_l1_signature".to_string())?;
                    let client = client::PolymarketRewardsClient::new()?;
                    let api = client.derive_api_key(&client::ClobL1Auth {
                        address: address.to_string(),
                        signature: clob_l1_signature.to_string(),
                        timestamp: clob_auth.timestamp,
                        nonce: Some(clob_auth.nonce),
                    })?;
                    let cancel = tool::WithdrawQuoteLiquidity::run(
                        &client::PolymarketRewardsApp,
                        client::WithdrawQuoteLiquidityArgs {
                            confirmation: Some("confirm".to_string()),
                            address: address.to_string(),
                            api_key: api.api_key,
                            api_secret: api.api_secret,
                            passphrase: api.passphrase,
                            signature_type: None,
                            funder: None,
                            order_ids: Some(order_ids),
                            condition_id: None,
                            asset_id: None,
                            simulate: Some(false),
                        },
                        tool_ctx("withdraw_quote_liquidity"),
                    )?;
                    canceled = Some(cancel);
                }
            }

            print_json(&json!({
                "signer_address": signer_address,
                "preview_market": preview.get("market_question").cloned(),
                "stage1": summarize_stage(&stage1),
                "stage2": summarize_stage(&stage2),
                "stage3": summarize_stage(&stage3),
                "submit_live": summarize_stage(&submit_live),
                "cancel_result": canceled.as_ref().map(summarize_stage),
                "state_file": state_path,
            }))?;
        }
        _ => return Err(usage()),
    }

    Ok(())
}

fn build_preview_result(condition_id: &str, capital: f64) -> Result<Value, String> {
    let resolve = tool::ResolveRewardDeployment::run(
        &client::PolymarketRewardsApp,
        client::ResolveRewardDeploymentArgs {
            plan_id: condition_id.to_string(),
            ranked_condition_ids: None,
            capital_usd: Some(capital),
        },
        tool_ctx("resolve_reward_deployment"),
    )?;

    let yes_token_id = resolve
        .get("yes_token_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "resolve_reward_deployment missing yes_token_id".to_string())?;
    let no_token_id = resolve
        .get("no_token_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "resolve_reward_deployment missing no_token_id".to_string())?;
    let deployment = resolve
        .get("deployment")
        .and_then(Value::as_object)
        .ok_or_else(|| "resolve_reward_deployment missing deployment".to_string())?;
    let reward_params = resolve
        .get("reward_params")
        .and_then(Value::as_object)
        .ok_or_else(|| "resolve_reward_deployment missing reward_params".to_string())?;

    let build = tool::BuildQuotePlan::run(
        &client::PolymarketRewardsApp,
        client::BuildQuotePlanArgs {
            condition_id: condition_id.to_string(),
            yes_token_id: yes_token_id.to_string(),
            no_token_id: no_token_id.to_string(),
            order_size_usd: capital,
            yes_bid_price: read_f64(deployment, "yes_bid_price")?,
            yes_ask_price: read_f64(deployment, "yes_ask_price")?,
            time_in_force: Some("GTC".to_string()),
            execution_mode: Some(client::QuoteExecutionMode::TwoLegBidOnly),
            rewards_max_spread: Some(read_f64(reward_params, "rewards_max_spread")?),
            rewards_min_size: Some(read_f64(reward_params, "rewards_min_size")?),
        },
        tool_ctx("build_quote_plan"),
    )?;

    Ok(json!({
        "market_question": resolve.get("question").cloned(),
        "resolve_reward_deployment": resolve,
        "build_quote_plan": build.clone(),
        "submit_args_template": build.get("submit_args_template").cloned(),
    }))
}

fn run_submit(args: Value) -> Result<Value, String> {
    let parsed: client::SubmitRewardQuoteArgs =
        serde_json::from_value(args).map_err(|e| format!("invalid submit args: {e}"))?;
    tool::SubmitRewardQuote::run(
        &client::PolymarketRewardsApp,
        parsed,
        tool_ctx("submit_reward_quote"),
    )
}

fn persist_next_state(path: &PathBuf, result: &Value) -> Result<(), String> {
    if let Some(template) = result.get("submit_args_template") {
        let bytes = serde_json::to_vec_pretty(template)
            .map_err(|e| format!("failed to serialize state file: {e}"))?;
        fs::write(path, bytes).map_err(|e| format!("failed to write state file: {e}"))?;
    }
    Ok(())
}

fn load_state(path: &PathBuf) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|e| format!("failed to read state file: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("failed to parse state file: {e}"))
}

fn print_json(value: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| format!("failed to render json: {e}"))?;
    println!("{text}");
    Ok(())
}

fn sign_typed_data(signer: &PrivateKeySigner, typed_data: &Value) -> Result<String, String> {
    let typed: TypedData = serde_json::from_value(typed_data.clone())
        .map_err(|e| format!("invalid typed data payload: {e}"))?;
    let hash = typed
        .eip712_signing_hash()
        .map_err(|e| format!("failed to hash typed data: {e}"))?;
    signer
        .sign_hash_sync(&hash)
        .map(|sig| sig.to_string())
        .map_err(|e| format!("failed to sign typed data: {e}"))
}

fn load_env_signer() -> Result<PrivateKeySigner, String> {
    let private_key = std::env::var(PRIVATE_KEY_VAR)
        .or_else(|_| std::env::var("POLYMARKET_TEST_PRIVATE_KEY"))
        .map_err(|_| {
            format!("missing {PRIVATE_KEY_VAR} or POLYMARKET_TEST_PRIVATE_KEY in environment")
        })?;
    PrivateKeySigner::from_str(private_key.trim())
        .map_err(|e| format!("invalid private key env var: {e}"))
        .map(|signer| signer.with_chain_id(Some(POLYGON)))
}

fn derive_creds_for_signer(signer: &PrivateKeySigner) -> Result<client::ClobCredentials, String> {
    let address = format!("{:#x}", signer.address());
    let clob_auth = client::build_reward_clob_auth_context(&address);
    let signature = sign_typed_data(
        signer,
        &client::build_reward_clob_auth_typed_data(&clob_auth),
    )?;
    let client = client::PolymarketRewardsClient::new()?;
    let api = client.create_or_derive_api_credentials(&client::ClobL1Auth {
        address: address.clone(),
        signature,
        timestamp: clob_auth.timestamp,
        nonce: Some(clob_auth.nonce),
    })?;

    Ok(client::ClobCredentials {
        address,
        api_key: api.api_key,
        api_secret: api.api_secret,
        passphrase: api.passphrase,
        signature_type: Some("eoa".to_string()),
        funder: None,
    })
}

fn summarize_stage(result: &Value) -> Value {
    json!({
        "stage": result.get("stage").cloned(),
        "mode": result.get("mode").cloned(),
        "status": result.get("status").cloned(),
        "error": result.get("error").cloned(),
        "pending_sign_request_count": result.get("pending_sign_request_count").cloned(),
        "order_count": result.get("order_count").cloned(),
    })
}

fn extract_order_ids(result: &Value) -> Option<Vec<String>> {
    let order_results = result.get("order_results")?.as_object()?;
    let mut ids = Vec::new();
    for value in order_results.values() {
        if let Some(obj) = value.as_object() {
            for key in ["orderID", "orderId", "id"] {
                if let Some(id) = obj.get(key).and_then(Value::as_str)
                    && !id.trim().is_empty()
                {
                    ids.push(id.to_string());
                    break;
                }
            }
        }
    }
    if ids.is_empty() { None } else { Some(ids) }
}

fn tool_ctx(tool_name: &str) -> DynToolCallCtx {
    DynToolCallCtx {
        session_id: "local-rewards-harness".to_string(),
        tool_name: tool_name.to_string(),
        call_id: format!("{tool_name}-local"),
        state_attributes: Map::new(),
    }
}

fn read_f64(map: &Map<String, Value>, key: &str) -> Result<f64, String> {
    map.get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("missing numeric field `{key}`"))
}

fn parse_f64(value: &str, label: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|e| format!("invalid {label} `{value}`: {e}"))
}

fn required<'a>(parsed: &'a ParsedArgs, key: &str) -> Result<&'a str, String> {
    parsed
        .values
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("missing --{key}\n\n{}", usage()))
}

fn state_file(parsed: &ParsedArgs) -> Result<PathBuf, String> {
    Ok(PathBuf::from(
        parsed
            .values
            .get("state-file")
            .cloned()
            .unwrap_or_else(|| "/tmp/polymarket-rewards-state.json".to_string()),
    ))
}

#[derive(Default)]
struct ParsedArgs {
    values: std::collections::BTreeMap<String, String>,
    flags: std::collections::BTreeSet<String>,
}

fn parse_flags(raw: Vec<String>) -> Result<ParsedArgs, String> {
    let mut parsed = ParsedArgs::default();
    let mut i = 0;
    while i < raw.len() {
        let arg = &raw[i];
        if !arg.starts_with("--") {
            return Err(format!("unexpected argument `{arg}`\n\n{}", usage()));
        }
        let key = arg.trim_start_matches("--").to_string();
        if i + 1 < raw.len() && !raw[i + 1].starts_with("--") {
            parsed.values.insert(key, raw[i + 1].clone());
            i += 2;
        } else {
            parsed.flags.insert(key);
            i += 1;
        }
    }
    Ok(parsed)
}

fn usage() -> String {
    [
        "Usage:",
        "  cargo run --bin rewards_harness -- preview --condition-id <0x...> --capital <usdc>",
        "  cargo run --bin rewards_harness -- start --address <0x...> --condition-id <0x...> --capital <usdc> [--state-file /tmp/rewards-state.json]",
        "  cargo run --bin rewards_harness -- resume --state-file /tmp/rewards-state.json [--clob-l1-signature 0x...] [--yes-bid-signature 0x...] [--no-bid-signature 0x...] [--simulation-confirmed]",
        "  cargo run --bin rewards_harness -- preflight",
        "  cargo run --bin rewards_harness -- open-orders [--asset-id <token-id>]",
        "  cargo run --bin rewards_harness -- cancel-open --condition-id <0x...>",
        "  cargo run --bin rewards_harness -- cancel-open --asset-id <token-id>",
        "  cargo run --bin rewards_harness -- cancel-open --all",
        "  cargo run --bin rewards_harness -- self-test-live --condition-id <0x...> --capital <usdc> [--address <0x...>] [--state-file /tmp/rewards-state.json] [--no-cancel]",
        "",
        "The harness reuses the same rewards plugin code path without the bot.",
        "`self-test-live` signs locally from POLYMARKET_PRIVATE_KEY and cancels the orders after a successful live submit unless --no-cancel is passed.",
    ]
    .join("\n")
}
