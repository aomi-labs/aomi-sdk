mod common;

use aomi_sdk::{EnforcementPolicy, RouteStep, RouteTarget, RouteTrigger, ToolReturn, host};
use common::fixtures::{AsyncTool, SubmitOrder, SyncTool};
use serde_json::{Value, json};

#[test]
fn plain_tool_return_serializes_to_raw_value() {
    let tool_return = ToolReturn::value(json!({"ok": true}));
    let serialized = serde_json::to_value(&tool_return).unwrap();
    assert_eq!(serialized, json!({"ok": true}));

    let roundtrip = ToolReturn::from_value(serialized).unwrap();
    assert_eq!(roundtrip.value, json!({"ok": true}));
    assert!(roundtrip.routes.is_empty());
}

#[test]
fn routed_tool_return_serializes_to_envelope() {
    let tool_return = ToolReturn::with_routes(
        json!({"status": "awaiting_wallet"}),
        [
            RouteStep::on_return("commit_eip712", json!({"typed_data": {}}))
                .bind_as("clob_l1_signature"),
            RouteStep::on_bound_event(
                "submit_polymarket_order",
                json!({"market": "btc"}),
                "clob_l1_signature",
            ),
        ],
    );

    let serialized = serde_json::to_value(&tool_return).unwrap();
    assert_eq!(
        serialized,
        json!({
            "__aomi_tool_return": true,
            "__aomi_tool_value": {"status": "awaiting_wallet"},
            "__aomi_tool_routes": [
                {
                    "tool": "commit_eip712",
                    "args": {"typed_data": {}},
                    "trigger": {"type": "on_sync_return"},
                    "bind_as": "clob_l1_signature",
                },
                {
                    "tool": "submit_polymarket_order",
                    "args": {"market": "btc"},
                    "trigger": {
                        "type": "on_bound_event",
                        "alias": "clob_l1_signature",
                    },
                }
            ],
        })
    );

    let roundtrip = ToolReturn::from_value(serialized).unwrap();
    assert!(roundtrip.has_routes());
    assert_eq!(roundtrip.routes.len(), 2);
}

#[test]
fn svm_host_route_target_names_match_host_tools() {
    assert_eq!(
        <host::SvmStageIx as RouteTarget>::tool_name(),
        "svm_stage_ix"
    );
    assert_eq!(
        <host::SvmStageTx as RouteTarget>::tool_name(),
        "svm_stage_tx"
    );
    assert_eq!(
        <host::SvmSimulateIx as RouteTarget>::tool_name(),
        "svm_simulate_ix"
    );
    assert_eq!(
        <host::SvmSimulateTx as RouteTarget>::tool_name(),
        "svm_simulate_tx"
    );
    assert_eq!(
        <host::SvmCommitIx as RouteTarget>::tool_name(),
        "svm_commit_ix"
    );
    assert_eq!(
        <host::SvmCommitTx as RouteTarget>::tool_name(),
        "svm_commit_tx"
    );
    assert_eq!(<host::SvmSignTx as RouteTarget>::tool_name(), "svm_sign_tx");
    assert_eq!(
        <host::SvmSignData as RouteTarget>::tool_name(),
        "svm_sign_data"
    );
}

#[test]
fn svm_lane_1_stage_commit_route_plan_serializes() {
    // Lane 1 canonical chain — stage_ix → commit_ix. Mirrors what
    // Marinade's build_stake emits (wallet-mode broadcast).
    let plan = ToolReturn::route(json!({"status": "previewed"}))
        .next(|next| {
            next.add::<host::SvmStageIx>(json!({
                "instructions": [{"program_id": "Stake11...", "accounts": [], "data_base64": "..."}],
                "description": "stake 1 SOL on Marinade",
            }))
            .bind_as("ix_ids");
        })
        .after::<host::SvmCommitIx>(json!({
            "mode": "wallet",
            "version": "v0",
        }))
        .awaits("ix_ids")
        .build();

    let routes = serde_json::to_value(&plan).unwrap()["__aomi_tool_routes"]
        .as_array()
        .expect("routes present")
        .clone();
    let tools: Vec<&str> = routes
        .iter()
        .filter_map(|route| route.get("tool").and_then(Value::as_str))
        .collect();
    assert_eq!(tools, vec!["svm_stage_ix", "svm_commit_ix"]);
}

#[test]
fn svm_lane_2_stage_commit_route_plan_serializes() {
    // Lane 2 canonical chain — stage_tx → commit_tx. Future shape for
    // venue blob → wallet sign once apps adopt Lane 2 commit.
    let plan = ToolReturn::route(json!({"status": "previewed"}))
        .next(|next| {
            next.add::<host::SvmStageTx>(json!({
                "tx": "AgAB...base64...",
                "description": "swap via byreal RFQ",
            }))
            .bind_as("tx_id");
        })
        .after::<host::SvmCommitTx>(json!({"mode": "wallet"}))
        .awaits("tx_id")
        .build();

    let routes = serde_json::to_value(&plan).unwrap()["__aomi_tool_routes"]
        .as_array()
        .expect("routes present")
        .clone();
    let tools: Vec<&str> = routes
        .iter()
        .filter_map(|route| route.get("tool").and_then(Value::as_str))
        .collect();
    assert_eq!(tools, vec!["svm_stage_tx", "svm_commit_tx"]);

    // Lane 2 commit takes only `tx_id` + `mode` — assert no
    // accidental Lane 1 args leaked into the after step's payload.
    let commit_args = routes[1].get("args").expect("args present");
    let args_obj = commit_args.as_object().expect("args object");
    assert!(!args_obj.contains_key("ix_ids"));
    assert!(!args_obj.contains_key("version"));
    assert!(!args_obj.contains_key("address_lookup_tables"));
    assert!(!args_obj.contains_key("compute_units"));
}

#[test]
fn svm_lane_1_stage_simulate_route_plan_serializes() {
    // Lane 1 canonical chain — stage ix list → simulate via
    // `SvmSimulateIx({ ix_ids })`. Mirrors what a future Marinade
    // build_stake that wants pre-commit simulation would emit.
    let plan = ToolReturn::route(json!({"status": "previewed"}))
        .next(|next| {
            next.add::<host::SvmStageIx>(json!({
                "instructions": [{"program_id": "Stake11...", "accounts": [], "data_base64": "..."}],
                "description": "stake 1 SOL on Marinade",
            }))
            .bind_as("ix_ids");
        })
        .after::<host::SvmSimulateIx>(json!({"mode": "rpc"}))
        .awaits("ix_ids")
        .build();

    let routes = serde_json::to_value(&plan).unwrap()["__aomi_tool_routes"]
        .as_array()
        .expect("routes present")
        .clone();
    let tools: Vec<&str> = routes
        .iter()
        .filter_map(|route| route.get("tool").and_then(Value::as_str))
        .collect();
    assert_eq!(tools, vec!["svm_stage_ix", "svm_simulate_ix"]);

    // Lane symmetry: alias bound by stage_ix is `ix_ids`, simulate_ix
    // consumes the same alias — no XOR-arg ambiguity at any point.
    let stage = &routes[0];
    assert_eq!(stage.get("bind_as").and_then(Value::as_str), Some("ix_ids"));
    let simulate_trigger = routes[1].get("trigger").expect("trigger present");
    assert_eq!(
        simulate_trigger.get("alias").and_then(Value::as_str),
        Some("ix_ids")
    );
}

#[test]
fn svm_lane_2_stage_simulate_route_plan_serializes() {
    // Lane 2 canonical chain — venue-built tx blob → stage_tx →
    // simulate_tx. Mirrors what a future byreal `build_swap` would
    // emit once it routes through Lane 2 instead of the transitional
    // inline `SvmSignTx` path.
    let plan = ToolReturn::route(json!({"status": "previewed"}))
        .next(|next| {
            next.add::<host::SvmStageTx>(json!({
                "tx": "AgAB...base64...VersionedTransaction",
                "description": "swap 1 USDC for 0.005 SOL via byreal RFQ",
                "kind": "byreal.swap",
            }))
            .bind_as("tx_id");
        })
        .after::<host::SvmSimulateTx>(json!({"mode": "rpc"}))
        .awaits("tx_id")
        .build();

    let routes = serde_json::to_value(&plan).unwrap()["__aomi_tool_routes"]
        .as_array()
        .expect("routes present")
        .clone();
    let tools: Vec<&str> = routes
        .iter()
        .filter_map(|route| route.get("tool").and_then(Value::as_str))
        .collect();
    assert_eq!(tools, vec!["svm_stage_tx", "svm_simulate_tx"]);

    let stage = &routes[0];
    assert_eq!(stage.get("bind_as").and_then(Value::as_str), Some("tx_id"));
    let simulate_trigger = routes[1].get("trigger").expect("trigger present");
    assert_eq!(
        simulate_trigger.get("alias").and_then(Value::as_str),
        Some("tx_id")
    );
}

#[test]
fn route_builder_serializes_bound_artifact_plan() {
    let tool_return = ToolReturn::route(json!({"status": "awaiting_wallet"}))
        .next(|next| {
            next.add::<host::CommitEip712>(json!({"typed_data": {}}))
                .bind_as("clob_l1_signature")
                .note("sign this first");
        })
        .after::<SubmitOrder>(json!({"market": "btc"}))
        .awaits("clob_l1_signature")
        .note("continue submit")
        .build();

    assert_eq!(
        serde_json::to_value(&tool_return).unwrap(),
        json!({
            "__aomi_tool_return": true,
            "__aomi_tool_value": {"status": "awaiting_wallet"},
            "__aomi_tool_routes": [
                {
                    "tool": "commit_eip712",
                    "args": {"typed_data": {}},
                    "trigger": {"type": "on_sync_return"},
                    "bind_as": "clob_l1_signature",
                    "prompt": "sign this first",
                },
                {
                    "tool": "submit_order",
                    "args": {"market": "btc"},
                    "trigger": {
                        "type": "on_bound_event",
                        "alias": "clob_l1_signature",
                    },
                    "prompt": "continue submit",
                }
            ]
        })
    );
}

#[test]
fn route_builder_serializes_solana_sign_plan() {
    let tool_return = ToolReturn::route(json!({"status": "awaiting_wallet"}))
        .next(|next| {
            next.add::<host::SvmSignTx>(json!({
                "unsigned_tx": "AgAB...base64...",
                "description": "Swap 1 USDC for 0.005 SOL via byreal RFQ",
            }))
            .bind_as("signed_tx")
            .note("sign this Solana swap");
        })
        .after::<SubmitOrder>(json!({"venue": "byreal-rfq"}))
        .awaits("signed_tx")
        .note("submit signed tx to venue")
        .build();

    assert_eq!(
        serde_json::to_value(&tool_return).unwrap(),
        json!({
            "__aomi_tool_return": true,
            "__aomi_tool_value": {"status": "awaiting_wallet"},
            "__aomi_tool_routes": [
                {
                    "tool": "svm_sign_tx",
                    "args": {
                        "unsigned_tx": "AgAB...base64...",
                        "description": "Swap 1 USDC for 0.005 SOL via byreal RFQ",
                    },
                    "trigger": {"type": "on_sync_return"},
                    "bind_as": "signed_tx",
                    "prompt": "sign this Solana swap",
                },
                {
                    "tool": "submit_order",
                    "args": {"venue": "byreal-rfq"},
                    "trigger": {
                        "type": "on_bound_event",
                        "alias": "signed_tx",
                    },
                    "prompt": "submit signed tx to venue",
                }
            ]
        })
    );
}

#[test]
fn route_builder_bind_as_works_for_any_tool() {
    let tool_return = ToolReturn::route(json!({"status": "ok"}))
        .next(|next| {
            next.add::<SyncTool>(json!({"x": 1})).bind_as("tool_result");
        })
        .after::<SubmitOrder>(json!({}))
        .awaits("tool_result")
        .build();

    assert_eq!(
        tool_return.routes[0].bind_as.as_deref(),
        Some("tool_result")
    );
}

#[test]
fn route_builder_async_tool_can_bind_as() {
    let tool_return = ToolReturn::route(json!({"status": "ok"}))
        .next(|next| {
            next.add::<AsyncTool>(json!({"x": 1})).bind_as("from_async");
        })
        .after::<SubmitOrder>(json!({}))
        .awaits("from_async")
        .build();

    assert_eq!(tool_return.routes[0].bind_as.as_deref(), Some("from_async"));
}

#[test]
fn route_builder_enforcement_can_satisfy_awaited_alias() {
    let tool_return = ToolReturn::route(json!({"status": "ok"}))
        .next(|next| {
            next.add::<host::StageTx>(json!({"to": "0x1", "data": {"raw": "0x"}}))
                .enforce(EnforcementPolicy::Stop, |enforce| {
                    enforce.add::<host::SimulateBatch>(json!({}));
                    enforce
                        .add::<host::CommitTxs>(json!({"aa_preference": "auto"}))
                        .bind_as("transaction_hash");
                });
        })
        .after::<SubmitOrder>(json!({"quote_id": "quote-1"}))
        .awaits("transaction_hash")
        .build();

    assert_eq!(tool_return.routes[0].bind_as, None);
    assert!(tool_return.routes[0].enforcement.is_some());
    assert!(matches!(
        &tool_return.routes[1].trigger,
        RouteTrigger::OnBoundEvent { alias } if alias == "transaction_hash"
    ));
}

#[test]
fn route_builder_rejects_invalid_aliases() {
    assert_err_contains(
        ToolReturn::route(json!({"status": "ok"}))
            .after::<SubmitOrder>(json!({}))
            .awaits("missing_alias")
            .try_build(),
        "awaits unknown alias `missing_alias`",
    );

    assert_err_contains(
        ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<SyncTool>(json!({"x": 1})).bind_as("   ");
            })
            .try_build(),
        "bound alias must not be empty",
    );

    assert_err_contains(
        ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<SyncTool>(json!({"x": 1})).bind_as("artifact");
            })
            .after::<SubmitOrder>(json!({}))
            .awaits(" ")
            .try_build(),
        "awaits alias must not be empty",
    );
}

#[test]
fn route_builder_rejects_duplicate_aliases() {
    let err = ToolReturn::route(json!({"status": "ok"}))
        .next(|next| {
            next.add::<host::CommitEip712>(json!({"typed_data": {}}))
                .bind_as("dup");
            next.add::<SyncTool>(json!({"x": 1})).bind_as("dup");
        })
        .after::<SubmitOrder>(json!({}))
        .awaits("dup")
        .try_build()
        .expect_err("duplicate aliases should fail");

    assert!(err.contains("duplicate bound alias `dup`"));
}

#[test]
fn route_builder_rejects_invalid_enforced_producers() {
    assert_err_contains(
        ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<host::StageTx>(json!({"to": "0x1"}))
                    .enforce(EnforcementPolicy::Stop, |_| {});
                next.add::<host::StageTx>(json!({"to": "0x2"}))
                    .enforce(EnforcementPolicy::Stop, |_| {});
            })
            .try_build(),
        "at most one enforced producer",
    );

    assert_err_contains(
        ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<host::StageTx>(json!(["not", "an", "object"]))
                    .enforce(EnforcementPolicy::Stop, |_| {});
            })
            .try_build(),
        "must use object args",
    );
}

#[test]
fn route_builder_allows_repeated_tool_with_distinct_binds() {
    let tool_return = ToolReturn::route(json!({"status": "awaiting_wallet"}))
        .next(|next| {
            next.add::<host::CommitEip712>(json!({"typed_data": {"approval": true}}))
                .bind_as("approval")
                .note("Sign the Permit2 approval first.");
            next.add::<host::CommitEip712>(json!({"typed_data": {"trade": true}}))
                .bind_as("trade")
                .note("Then sign the gasless trade.");
        })
        .after::<SubmitOrder>(json!({"chain_id": 1}))
        .awaits_all(["approval", "trade"])
        .build();

    assert_eq!(tool_return.routes.len(), 3);
    assert_eq!(tool_return.routes[0].tool, "commit_eip712");
    assert_eq!(tool_return.routes[0].bind_as.as_deref(), Some("approval"));
    assert_eq!(tool_return.routes[1].tool, "commit_eip712");
    assert_eq!(tool_return.routes[1].bind_as.as_deref(), Some("trade"));
    assert_all_bound(&tool_return, &["approval", "trade"]);
}

#[test]
fn route_builder_awaits_called_twice_upgrades_to_multi_alias() {
    let tool_return = ToolReturn::route(json!({"status": "awaiting_wallet"}))
        .next(|next| {
            next.add::<host::CommitEip712>(json!({"typed_data": {"a": true}}))
                .bind_as("approval");
            next.add::<host::CommitEip712>(json!({"typed_data": {"t": true}}))
                .bind_as("trade");
        })
        .after::<SubmitOrder>(json!({"chain_id": 1}))
        .awaits("approval")
        .awaits("trade")
        .build();

    assert_all_bound(&tool_return, &["approval", "trade"]);
}

#[test]
fn route_builder_single_awaits_still_uses_on_bound_event() {
    let tool_return = ToolReturn::route(json!({"status": "ok"}))
        .next(|next| {
            next.add::<host::CommitEip712>(json!({"typed_data": {}}))
                .bind_as("signature");
        })
        .after::<SubmitOrder>(json!({}))
        .awaits("signature")
        .build();

    match &tool_return.routes[1].trigger {
        RouteTrigger::OnBoundEvent { alias } => assert_eq!(alias, "signature"),
        other => panic!("expected OnBoundEvent, got {other:?}"),
    }
}

#[test]
fn route_builder_rejects_invalid_awaits_all_aliases() {
    for (awaited, expected) in [
        (["approval", "missing"], "awaits unknown alias `missing`"),
        (["approval", "approval"], "more than once"),
    ] {
        assert_err_contains(
            ToolReturn::route(json!({"status": "ok"}))
                .next(|next| {
                    next.add::<host::CommitEip712>(json!({"typed_data": {}}))
                        .bind_as("approval");
                })
                .after::<SubmitOrder>(json!({}))
                .awaits_all(awaited)
                .try_build(),
            expected,
        );
    }
}

fn assert_err_contains(result: Result<ToolReturn, String>, expected: &str) {
    let err = result.expect_err("route builder should fail");
    assert!(err.contains(expected), "expected `{expected}`, got `{err}`");
}

fn assert_all_bound(tool_return: &ToolReturn, expected: &[&str]) {
    match &tool_return.routes[2].trigger {
        RouteTrigger::OnAllBoundEvents { aliases } => {
            assert_eq!(
                aliases,
                &expected
                    .iter()
                    .map(|alias| alias.to_string())
                    .collect::<Vec<_>>()
            );
        }
        other => panic!("expected OnAllBoundEvents, got {other:?}"),
    }
}

// ===========================================================================
// 3-node SVM pipeline tests
//
// The high-level `RouteBuilder` API (`.next` + `.after`) ships exactly two
// trigger classes: one or more on-sync-return steps plus one
// on-bound-event after-step. That's enough for the 2-node patterns most
// apps emit today (Marinade's `stage_ix → commit_ix`, byreal's `sign_tx
// → submit_*`).
//
// Future SVM apps that want a stage → simulate → commit preview flow
// need the LOWER-LEVEL `RouteStep::on_return_to` / `RouteStep::on_bound_event`
// constructors to wire a fully sequential 3-node chain. The tests below pin
// that shape using the host markers from the SDK's `host` module, so:
//
//   - SDK route-machinery regressions (e.g. trigger serialization, alias
//     handling, RouteTarget::tool_name drift) surface here without needing
//     a real app to wire them up.
//   - Future helper that wraps this pattern (e.g.
//     `build_svm_stage_sim_commit_routes`) has a reference test to mirror.
//   - The exact host verb names (`svm_stage_tx`, `svm_simulate_tx`,
//     `svm_commit_tx`) are pinned against the markers; a rename in the host
//     contract breaks here before it breaks a live wallet dispatch.
// ===========================================================================

/// Lane 2 (venue tx blob) 3-node chain:
///
///   parent ──sync return──▶ svm_stage_tx          binds  tx_id
///                            │
///                            └─bound event(tx_id)─▶ svm_simulate_tx   binds sim_result
///                                                    │
///                                                    └─bound event(sim_result)─▶ svm_commit_tx
///
/// Each step's trigger references the *previous* step's binding alias,
/// not the stage's `tx_id`. That's what makes the chain sequential vs
/// a fan-out (both sim and commit awaiting the same `tx_id`). The runtime
/// injects the bound value into the awaiting step's args under the
/// alias's name, so `svm_simulate_tx` and `svm_commit_tx` see
/// `tx_id` / `sim_result` materialised at dispatch time without the app
/// having to thread them through the static args template.
#[test]
fn lane_2_stage_sim_commit_chain_binds_sequentially() {
    let parent_value = json!({
        "action_kind": "swap",
        "preview": { "in": "1000", "out": "950" },
    });

    // Step 1 — stage the venue-supplied tx blob. Synthetic base64 — this
    // layer doesn't decode it; the host's pipeline finalizer does.
    let mut stage = RouteStep::on_return_to::<host::SvmStageTx>(json!({
        "tx": "AQID",
        "description": "venue swap blob",
        "preserve_blockhash": true,
    }));
    stage.bind_as = Some("tx_id".to_string());

    // Step 2 — simulate the staged tx. The runtime injects `tx_id`
    // from the bound alias; the app leaves it out of the static args.
    let mut sim = RouteStep::on_bound_event(
        host::SvmSimulateTx::tool_name(),
        json!({ "mode": "litesvm" }),
        "tx_id",
    );
    sim.bind_as = Some("sim_result".to_string());

    // Step 3 — commit. Awaits `sim_result` so the chain is strictly
    // sequential (commit doesn't fire until sim returns). The runtime
    // still has `tx_id` in the artifact store for commit to use; it's
    // already bound from step 1.
    let commit = RouteStep::on_bound_event(
        host::SvmCommitTx::tool_name(),
        json!({ "mode": "wallet" }),
        "sim_result",
    );

    let tool_return = ToolReturn::with_routes(
        parent_value.clone(),
        [stage.clone(), sim.clone(), commit.clone()],
    );

    // Round-trip the envelope. Catches serializer drift on
    // `__aomi_tool_routes` and the per-step shape.
    let env = serde_json::to_value(&tool_return).expect("envelope serializes");
    assert_eq!(env["__aomi_tool_return"], json!(true));
    assert_eq!(env["__aomi_tool_value"], parent_value);

    let routes = env["__aomi_tool_routes"]
        .as_array()
        .expect("routes serialize to array");
    assert_eq!(routes.len(), 3, "3-node chain has exactly 3 steps");

    // ── Step 1: stage_tx ─────────────────────────────────────────────
    let s = &routes[0];
    assert_eq!(s["tool"], json!(host::SvmStageTx::tool_name()));
    assert_eq!(
        s["tool"],
        json!("svm_stage_tx"),
        "marker drift — `host::SvmStageTx` no longer maps to `svm_stage_tx`"
    );
    assert_eq!(s["trigger"], json!({ "type": "on_sync_return" }));
    assert_eq!(
        s["bind_as"],
        json!("tx_id"),
        "stage must bind `tx_id` — downstream sim awaits this alias"
    );
    assert_eq!(s["args"]["tx"], json!("AQID"));
    assert_eq!(s["args"]["preserve_blockhash"], json!(true));

    // ── Step 2: simulate_tx ──────────────────────────────────────────
    let s = &routes[1];
    assert_eq!(s["tool"], json!(host::SvmSimulateTx::tool_name()));
    assert_eq!(s["tool"], json!("svm_simulate_tx"));
    assert_eq!(
        s["trigger"],
        json!({ "type": "on_bound_event", "alias": "tx_id" }),
        "sim must wait on `tx_id` — fires only after stage binds"
    );
    assert_eq!(
        s["bind_as"],
        json!("sim_result"),
        "sim must bind `sim_result` — downstream commit awaits this"
    );
    // `tx_id` is NOT in the static args — the runtime injects it from
    // the bound alias at dispatch time. The static args carry only the
    // mode override the app cares about.
    assert!(
        s["args"].get("tx_id").is_none(),
        "sim's static args must NOT pre-populate tx_id; runtime injects it from the bind"
    );
    assert_eq!(s["args"]["mode"], json!("litesvm"));

    // ── Step 3: commit_tx ────────────────────────────────────────────
    let s = &routes[2];
    assert_eq!(s["tool"], json!(host::SvmCommitTx::tool_name()));
    assert_eq!(s["tool"], json!("svm_commit_tx"));
    assert_eq!(
        s["trigger"],
        json!({ "type": "on_bound_event", "alias": "sim_result" }),
        "commit must wait on `sim_result` — preview before broadcast"
    );
    assert_eq!(
        s["args"]["mode"],
        json!("wallet"),
        "commit mode must be `wallet` (only working path until host #38-pipeline-c)"
    );

    // Round-trip back into the ToolReturn — verifies the envelope
    // parses what it serializes.
    let back = ToolReturn::from_value(env).expect("envelope round-trips");
    assert_eq!(back.routes.len(), 3);
    assert_eq!(back.routes[0].bind_as.as_deref(), Some("tx_id"));
    assert_eq!(back.routes[1].bind_as.as_deref(), Some("sim_result"));
    match &back.routes[1].trigger {
        RouteTrigger::OnBoundEvent { alias } => assert_eq!(alias, "tx_id"),
        other => panic!("sim trigger should be OnBoundEvent(tx_id), got {other:?}"),
    }
    match &back.routes[2].trigger {
        RouteTrigger::OnBoundEvent { alias } => assert_eq!(alias, "sim_result"),
        other => panic!("commit trigger should be OnBoundEvent(sim_result), got {other:?}"),
    }
}

/// Lane 1 (composed-from-instructions) 3-node chain — symmetric to the
/// Lane 2 case above. The host markers differ (`SvmStageIx` /
/// `SvmSimulateIx` / `SvmCommitIx` instead of the `*Tx` triple) and the
/// stage args shape is `instructions: [...]` rather than `tx: "<b64>"`,
/// but the binding/awaits chain is identical. Pinning both lanes here
/// makes future host renames (or SDK marker drift) surface in one place.
#[test]
fn lane_1_stage_sim_commit_chain_binds_sequentially() {
    let parent_value = json!({ "action_kind": "transfer", "amount": "1000000" });

    let mut stage = RouteStep::on_return_to::<host::SvmStageIx>(json!({
        "instructions": [
            {
                "program_id": "11111111111111111111111111111111",
                "accounts": [],
                "data_base64": "",
                "description": "synthetic ix"
            }
        ],
        "description": "Lane 1 transfer",
    }));
    stage.bind_as = Some("ix_ids".to_string());

    let mut sim = RouteStep::on_bound_event(
        host::SvmSimulateIx::tool_name(),
        json!({ "version": "legacy" }),
        "ix_ids",
    );
    sim.bind_as = Some("sim_result".to_string());

    let commit = RouteStep::on_bound_event(
        host::SvmCommitIx::tool_name(),
        json!({ "mode": "wallet", "version": "legacy" }),
        "sim_result",
    );

    let tool_return = ToolReturn::with_routes(parent_value, [stage, sim, commit]);
    let env = serde_json::to_value(&tool_return).expect("envelope serializes");
    let routes = env["__aomi_tool_routes"].as_array().expect("routes");

    assert_eq!(routes.len(), 3);
    assert_eq!(routes[0]["tool"], json!("svm_stage_ix"));
    assert_eq!(routes[0]["bind_as"], json!("ix_ids"));
    assert_eq!(routes[1]["tool"], json!("svm_simulate_ix"));
    assert_eq!(
        routes[1]["trigger"],
        json!({ "type": "on_bound_event", "alias": "ix_ids" })
    );
    assert_eq!(routes[1]["bind_as"], json!("sim_result"));
    assert_eq!(routes[2]["tool"], json!("svm_commit_ix"));
    assert_eq!(
        routes[2]["trigger"],
        json!({ "type": "on_bound_event", "alias": "sim_result" })
    );

    // Marker drift check: every host::Svm* tool_name returned from the
    // SDK must agree with the host's snake_case verb name. If a future
    // refactor renames either side without updating the other, this is
    // where it breaks.
    assert_eq!(host::SvmStageIx::tool_name(), "svm_stage_ix");
    assert_eq!(host::SvmSimulateIx::tool_name(), "svm_simulate_ix");
    assert_eq!(host::SvmCommitIx::tool_name(), "svm_commit_ix");
    assert_eq!(host::SvmStageTx::tool_name(), "svm_stage_tx");
    assert_eq!(host::SvmSimulateTx::tool_name(), "svm_simulate_tx");
    assert_eq!(host::SvmCommitTx::tool_name(), "svm_commit_tx");
}

/// Negative test — if a 3-node chain accidentally awaits the *stage*
/// alias on the commit step (a common copy-paste bug when extending a
/// 2-node template into 3 nodes), both sim AND commit fire as soon as
/// stage binds. They become parallel siblings, not a sequential chain
/// — sim's result is never gated. This test pins the buggy shape so
/// that if someone "fixes" the sequential test above by relaxing it
/// to this pattern, this negative test breaks too and forces a
/// conscious decision.
#[test]
#[allow(non_snake_case)]
fn fan_out_chain_is_NOT_sequential() {
    let mut stage = RouteStep::on_return_to::<host::SvmStageTx>(json!({ "tx": "AQID" }));
    stage.bind_as = Some("tx_id".to_string());

    // BOTH sim and commit awaiting the stage alias = fan-out, not chain.
    let sim = RouteStep::on_bound_event(host::SvmSimulateTx::tool_name(), json!({}), "tx_id");
    let commit = RouteStep::on_bound_event(
        host::SvmCommitTx::tool_name(),
        json!({ "mode": "wallet" }),
        "tx_id",
    );

    let tool_return = ToolReturn::with_routes(json!({}), [stage, sim, commit]);

    // Both downstream steps await the SAME alias — runtime fires them
    // both when `tx_id` binds. That's a fan-out (commit doesn't wait
    // on sim), which is observably wrong for a "preview-before-sign"
    // shape. The test asserts the wrong shape so that the *correct*
    // sequential chain above stays the documented pattern.
    match &tool_return.routes[1].trigger {
        RouteTrigger::OnBoundEvent { alias } => assert_eq!(alias, "tx_id"),
        other => panic!("sim trigger: {other:?}"),
    }
    match &tool_return.routes[2].trigger {
        RouteTrigger::OnBoundEvent { alias } => {
            assert_eq!(
                alias, "tx_id",
                "this is the BUGGY shape on purpose — commit must await sim's binding \
                 (`sim_result`), not the stage's `tx_id`, to make the chain sequential"
            );
        }
        other => panic!("commit trigger: {other:?}"),
    }
}
