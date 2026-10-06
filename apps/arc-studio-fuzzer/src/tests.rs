//! Campaign behaviour against a fake host that executes replay batches
//! with the semantics of the Arc Studio demo's `UnsafeVault`: anyone can
//! withdraw from the pooled deposits.

use std::collections::BTreeMap;
use std::str::FromStr;

use alloy_primitives::{Address, U256, keccak256};
use serde_json::{Value, json};

use crate::abi::{Actors, ContractAbi};
use crate::campaign::{FuzzCampaign, FuzzRoundArgs, FuzzStartArgs};
use crate::client::{AttackSurface, MapAttackSurfaceArgs, Sessions};
use crate::plan::{ExpectedStatus, FuzzActor, FuzzCall, FuzzContract, FuzzPlan, FuzzProperty};
use crate::replay::{CompactTrace, ReplayBalance, ReplayBatch, ReplayStatus};

const ARC: u64 = 5042;

fn vault() -> FuzzContract {
    FuzzContract {
        name: "UnsafeVault".into(),
        abi: json!([
            {"type":"function","name":"deposit","stateMutability":"payable","inputs":[],"outputs":[]},
            {"type":"function","name":"withdraw","stateMutability":"nonpayable","inputs":[{"name":"amount","type":"uint256"}],"outputs":[]},
            {"type":"function","name":"balances","stateMutability":"view","inputs":[{"name":"who","type":"address"}],"outputs":[{"type":"uint256"}]}
        ])
        .to_string(),
        deployed_bytecode: "0x6080".into(),
    }
}

fn call(label: &str, actor: &str, function: &str, arguments: &[&str], value: &str) -> FuzzCall {
    FuzzCall {
        label: label.into(),
        actor: actor.into(),
        function: function.into(),
        arguments: arguments.iter().map(|a| a.to_string()).collect(),
        value: value.into(),
    }
}

fn actor(name: &str) -> FuzzActor {
    FuzzActor {
        name: name.into(),
        balance: "10000000000000000000".into(),
    }
}

fn must_revert(name: &str, actor: &str, function: &str) -> FuzzProperty {
    FuzzProperty {
        name: name.into(),
        actor: actor.into(),
        function: function.into(),
        expected_status: Some(ExpectedStatus::Reverted),
        max_actor_profit: None,
        probes: vec![],
    }
}

fn vault_plan() -> FuzzPlan {
    FuzzPlan {
        hypothesis: "bob can withdraw alice's deposit".into(),
        actors: vec![actor("alice"), actor("bob")],
        actions: vec![
            call("alice deposits", "alice", "deposit()", &[], "100"),
            call("bob withdraws", "bob", "withdraw(uint256)", &["100"], "0"),
        ],
        invariants: vec![must_revert(
            "unauthorized_withdraw_must_revert",
            "bob",
            "withdraw(uint256)",
        )],
        max_depth: 3,
        max_executions: 32,
    }
}

fn start(plan: FuzzPlan) -> (FuzzCampaign, Value) {
    let mut campaign = FuzzCampaign::new(
        vault(),
        FuzzStartArgs {
            chain_id: ARC,
            plan,
        },
    )
    .unwrap();
    let result = campaign.start();
    (campaign, result)
}

fn batch(value: &Value) -> Option<ReplayBatch> {
    (!value.is_null())
        .then(|| ReplayBatch {
            snapshot: crate::replay::BASELINE,
            chain_id: value["chain_id"].as_u64().unwrap(),
            watch: serde_json::from_value(value["watch"].clone()).unwrap(),
            traces: Vec::new(),
        })
        .map(|_| ReplayBatch {
            traces: serde_json::from_value::<Vec<Value>>(value["traces"].clone())
                .unwrap()
                .into_iter()
                .map(|trace| crate::replay::ReplayTrace {
                    id: trace["id"].as_str().unwrap().into(),
                    calls: serde_json::from_value::<Vec<Value>>(trace["calls"].clone())
                        .unwrap()
                        .iter()
                        .map(replay_call)
                        .collect(),
                    probes: serde_json::from_value::<Vec<Value>>(trace["probes"].clone())
                        .unwrap()
                        .iter()
                        .map(replay_call)
                        .collect(),
                })
                .collect(),
            snapshot: crate::replay::BASELINE,
            chain_id: value["chain_id"].as_u64().unwrap(),
            watch: serde_json::from_value(value["watch"].clone()).unwrap(),
        })
}

fn replay_call(value: &Value) -> crate::replay::ReplayCall {
    crate::replay::ReplayCall {
        label: value["label"].as_str().unwrap().into(),
        from: value["from"].as_str().unwrap().into(),
        to: value["to"].as_str().unwrap().into(),
        data: value["data"].as_str().unwrap().into(),
        value: value["value"].as_str().unwrap().into(),
    }
}

/// Execute a batch the way `sim_replay_traces(compact=true)` would for an
/// `UnsafeVault` whose withdraw only checks the pooled balance.
fn execute(batch: &ReplayBatch, engine_error_on: Option<&str>) -> Vec<CompactTrace> {
    let deposit = &keccak256("deposit()")[..4];
    batch
        .traces
        .iter()
        .map(|trace| {
            let start = |address: &String| {
                if address == &trace.calls[0].to {
                    U256::ZERO
                } else {
                    U256::from(10_000_000_000_000_000_000u128)
                }
            };
            let mut balances: BTreeMap<String, U256> =
                batch.watch.iter().map(|a| (a.clone(), start(a))).collect();
            let before = balances.clone();
            let mut pool = U256::ZERO;
            let mut statuses = Vec::new();
            for call in &trace.calls {
                if engine_error_on == Some(call.label.as_str()) {
                    statuses.push(ReplayStatus::EngineError);
                    break;
                }
                let data = alloy_primitives::hex::decode(&call.data).unwrap();
                let value = U256::from_str(&call.value).unwrap();
                if &data[..4] == deposit {
                    pool += value;
                    *balances.get_mut(&call.from).unwrap() -= value;
                    *balances.get_mut(&call.to).unwrap() += value;
                    statuses.push(ReplayStatus::Succeeded);
                } else {
                    let amount = U256::from_be_slice(&data[4..36]);
                    if pool >= amount {
                        pool -= amount;
                        *balances.get_mut(&call.from).unwrap() += amount;
                        *balances.get_mut(&call.to).unwrap() -= amount;
                        statuses.push(ReplayStatus::Succeeded);
                    } else {
                        statuses.push(ReplayStatus::Reverted);
                    }
                }
            }
            CompactTrace {
                id: trace.id.clone(),
                statuses,
                probes: vec![],
                balances: balances
                    .iter()
                    .filter(|(address, after)| before[*address] != **after)
                    .map(|(address, after)| ReplayBalance {
                        address: address.clone(),
                        before: before[address].to_string(),
                        after: after.to_string(),
                    })
                    .collect(),
            }
        })
        .collect()
}

/// Drive the campaign to completion; returns the final report.
fn run_to_completion(campaign: &mut FuzzCampaign, first: &Value) -> Value {
    let mut next = first.clone();
    for _ in 0..64 {
        let Some(batch) = batch(&next) else {
            return campaign.report();
        };
        let result = campaign
            .round(FuzzRoundArgs {
                replay: execute(&batch, None),
                plan: None,
            })
            .unwrap();
        next = result["next_batch"].clone();
    }
    panic!("campaign did not complete");
}

#[test]
fn start_emits_setup_and_a_host_shaped_batch() {
    let (_, result) = start(vault_plan());
    assert_eq!(
        result["setup"]["snapshot"]["label"],
        crate::replay::BASELINE
    );
    assert_eq!(result["setup"]["set_code"]["runtime_bytecode"], "0x6080");
    assert_eq!(result["setup"]["set_balances"].as_array().unwrap().len(), 2);
    let next = &result["next_batch"];
    assert_eq!(
        next["watch"].as_array().unwrap().len(),
        3,
        "actors + contract"
    );
    assert_eq!(
        next["traces"].as_array().unwrap().len(),
        2,
        "one per action"
    );
    let call = &next["traces"][0]["calls"][0];
    let keys: Vec<_> = call.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys,
        ["data", "from", "label", "to", "value"],
        "no gas, no extras"
    );
    assert!(next.get("watch_addresses").is_none());
    assert_eq!(result["report"]["status"], "exploring");
}

#[test]
fn breadth_first_search_minimizes_and_reproduces_the_vault_drain() {
    let (mut campaign, result) = start(vault_plan());
    let report = run_to_completion(&mut campaign, &result["next_batch"]);

    assert_eq!(report["status"], "complete");
    let findings = report["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1, "{report:#}");
    let finding = &findings[0];
    assert_eq!(finding["property"], "unauthorized_withdraw_must_revert");
    assert_eq!(finding["reproductions"], 2);
    let labels: Vec<_> = finding["minimized_trace"]
        .as_array()
        .unwrap()
        .iter()
        .map(|call| call["label"].as_str().unwrap())
        .collect();
    assert_eq!(
        labels,
        ["alice deposits", "bob withdraws"],
        "minimal ordered reproduction"
    );
    assert_eq!(
        finding["steps"][1]["status"], "succeeded",
        "the unauthorized withdraw succeeded"
    );
    assert!(report["unconfirmed"].as_array().unwrap().is_empty());
    assert_eq!(
        report["uncovered_writes"].as_array().unwrap().len(),
        0,
        "every writable entrypoint was exercised"
    );
}

#[test]
fn an_engine_error_is_inconclusive_not_a_revert() {
    let (mut campaign, result) = start(vault_plan());
    let batch = batch(&result["next_batch"]).unwrap();
    let out = campaign
        .round(FuzzRoundArgs {
            replay: execute(&batch, Some("bob withdraws")),
            plan: None,
        })
        .unwrap();
    assert_eq!(out["report"]["inconclusive_traces"], 1);
    assert!(
        !out["report"]["behavior_coverage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b == "bob:withdraw(uint256):reverted"),
        "an engine error must not count as the expected revert"
    );
    assert!(
        out["report"]["behavior_coverage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b == "bob:withdraw(uint256):engine_error")
    );
}

#[test]
fn storage_only_states_are_not_pruned_together() {
    // Two actions that both succeed without moving value. A fingerprint of
    // statuses + balances alone would expand only the first.
    let contract = FuzzContract {
        name: "Config".into(),
        abi: json!([
            {"type":"function","name":"setOwner","stateMutability":"nonpayable","inputs":[{"type":"address"}],"outputs":[]},
            {"type":"function","name":"pause","stateMutability":"nonpayable","inputs":[],"outputs":[]}
        ])
        .to_string(),
        deployed_bytecode: "0x00".into(),
    };
    let plan = FuzzPlan {
        hypothesis: "config takeover".into(),
        actors: vec![actor("attacker")],
        actions: vec![
            call(
                "take ownership",
                "attacker",
                "setOwner(address)",
                &["$attacker"],
                "0",
            ),
            call("pause", "attacker", "pause()", &[], "0"),
        ],
        invariants: vec![FuzzProperty {
            max_actor_profit: Some("0".into()),
            expected_status: None,
            ..must_revert("no profit", "attacker", "pause()")
        }],
        max_depth: 2,
        max_executions: 32,
    };
    let mut campaign = FuzzCampaign::new(
        contract,
        FuzzStartArgs {
            chain_id: ARC,
            plan,
        },
    )
    .unwrap();
    let result = campaign.start();
    let batch = batch(&result["next_batch"]).unwrap();
    let replay = batch
        .traces
        .iter()
        .map(|trace| CompactTrace {
            id: trace.id.clone(),
            statuses: vec![ReplayStatus::Succeeded],
            probes: vec![],
            balances: vec![],
        })
        .collect();
    let out = campaign
        .round(FuzzRoundArgs { replay, plan: None })
        .unwrap();
    assert_eq!(out["report"]["corpus_size"], 2);
    assert_eq!(
        out["next_batch"]["traces"].as_array().unwrap().len(),
        4,
        "both depth-1 states are extended"
    );
}

#[test]
fn every_violation_in_a_batch_is_kept() {
    let mut plan = vault_plan();
    plan.actions.push(call(
        "alice withdraws",
        "alice",
        "withdraw(uint256)",
        &["0"],
        "0",
    ));
    plan.invariants.push(must_revert(
        "alice_withdraw_must_revert",
        "alice",
        "withdraw(uint256)",
    ));
    let (mut campaign, result) = start(plan);
    let report = run_to_completion(&mut campaign, &result["next_batch"]);
    let properties: Vec<_> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["property"].as_str().unwrap().to_string())
        .collect();
    assert!(
        properties.contains(&"alice_withdraw_must_revert".to_string()),
        "{report:#}"
    );
    assert!(properties.contains(&"unauthorized_withdraw_must_revert".to_string()));
}

#[test]
fn a_flaky_violation_is_reported_as_unconfirmed() {
    let (mut campaign, result) = start(vault_plan());
    let mut next = result["next_batch"].clone();
    // Run honestly until the campaign asks for reproductions, then fail them.
    loop {
        let batch = batch(&next).expect("campaign reaches reproduction");
        let reproducing = campaign.report()["status"] == "confirming"
            && batch.traces.len() == 2
            && batch.traces[0].calls == batch.traces[1].calls;
        let mut replay = execute(&batch, None);
        if reproducing {
            for trace in &mut replay {
                trace.statuses = vec![ReplayStatus::Succeeded, ReplayStatus::Reverted];
                trace.balances.clear();
            }
        }
        let out = campaign
            .round(FuzzRoundArgs { replay, plan: None })
            .unwrap();
        if reproducing {
            assert!(out["report"]["findings"].as_array().unwrap().is_empty());
            let unconfirmed = &out["report"]["unconfirmed"][0];
            assert_eq!(unconfirmed["property"], "unauthorized_withdraw_must_revert");
            assert_eq!(
                unconfirmed["reason"],
                "violation did not reproduce from the baseline"
            );
            return;
        }
        next = out["next_batch"].clone();
    }
}

#[test]
fn round_rejects_mismatched_replays_without_losing_the_batch() {
    let (mut campaign, result) = start(vault_plan());
    let batch = batch(&result["next_batch"]).unwrap();
    let mut replay = execute(&batch, None);

    let err = campaign
        .round(FuzzRoundArgs {
            replay: replay[..1].to_vec(),
            plan: None,
        })
        .unwrap_err();
    assert!(err.contains("expected 2 replayed traces"), "{err}");
    replay[1].id = replay[0].id.clone();
    let err = campaign
        .round(FuzzRoundArgs {
            replay: replay.clone(),
            plan: None,
        })
        .unwrap_err();
    assert!(err.contains("repeats a trace id"), "{err}");

    let replay = execute(&batch, None);
    campaign
        .round(FuzzRoundArgs { replay, plan: None })
        .expect("the pending batch survived the rejected attempts");
}

#[test]
fn a_revision_is_deferred_while_confirming_and_cannot_change_actors() {
    let (mut campaign, result) = start(vault_plan());
    let mut next = result["next_batch"].clone();
    loop {
        let batch = batch(&next).unwrap();
        let out = campaign
            .round(FuzzRoundArgs {
                replay: execute(&batch, None),
                plan: Some(vault_plan()),
            })
            .unwrap();
        if out["report"]["status"] == "confirming" {
            assert!(out["revision"].as_str().unwrap().starts_with("deferred"));
            break;
        }
        assert_eq!(out["revision"], "applied");
        next = out["next_batch"].clone();
    }

    let (mut campaign, result) = start(vault_plan());
    let batch = batch(&result["next_batch"]).unwrap();
    let mut changed = vault_plan();
    changed.actors[0].balance = "1".into();
    let err = campaign
        .round(FuzzRoundArgs {
            replay: execute(&batch, None),
            plan: Some(changed),
        })
        .unwrap_err();
    assert!(err.contains("cannot change actors"), "{err}");
}

#[test]
fn a_revision_keeps_the_frontier_and_seeds_only_new_actions() {
    let (mut campaign, result) = start(vault_plan());
    let batch = batch(&result["next_batch"]).unwrap();
    let mut revised = vault_plan();
    revised
        .actions
        .push(call("bob deposits", "bob", "deposit()", &[], "1"));
    let out = campaign
        .round(FuzzRoundArgs {
            replay: execute(&batch, None),
            plan: Some(revised),
        })
        .unwrap();
    assert_eq!(out["revision"], "applied");
    let labels: Vec<Vec<String>> = out["next_batch"]["traces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|trace| {
            trace["calls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["label"].as_str().unwrap().to_string())
                .collect()
        })
        .collect();
    assert!(
        labels.iter().any(|trace| trace.len() == 2),
        "depth-2 extensions queued before the revision survive: {labels:?}"
    );
    assert!(
        labels.iter().any(|trace| trace == &["bob deposits"]),
        "the new action starts at depth one: {labels:?}"
    );
}

#[test]
fn plans_are_validated_against_the_contract() {
    let reject = |plan: FuzzPlan, needle: &str| {
        let err = FuzzCampaign::new(
            vault(),
            FuzzStartArgs {
                chain_id: ARC,
                plan,
            },
        )
        .err()
        .expect("plan is rejected");
        assert!(err.contains(needle), "{err}");
    };
    let mut plan = vault_plan();
    plan.actions.pop();
    plan.invariants.clear();
    plan.invariants.push(must_revert("p", "alice", "deposit()"));
    reject(plan, "missing: withdraw(uint256)");

    let mut plan = vault_plan();
    plan.invariants[0].probes = vec![call("read", "alice", "withdraw(uint256)", &["1"], "0")];
    reject(plan, "must be a view or pure function");

    let mut plan = vault_plan();
    plan.invariants[0].probes = (0..=ReplayBatch::MAX_PROBES)
        .map(|i| {
            call(
                "read",
                "alice",
                "balances(address)",
                &[&format!("0x{i:040x}")],
                "0",
            )
        })
        .collect();
    reject(plan, "distinct probes");

    let mut plan = vault_plan();
    plan.actors.push(actor("Bad Name"));
    reject(plan, "lowercase");

    let err = FuzzCampaign::new(
        vault(),
        FuzzStartArgs {
            chain_id: 1,
            plan: vault_plan(),
        },
    )
    .err()
    .unwrap();
    assert!(err.contains("Arc fuzzing supports chain"), "{err}");
}

#[test]
fn arguments_encode_actor_references_tuples_and_arrays() {
    let abi = ContractAbi::parse(
        &json!([{"type":"function","name":"f","stateMutability":"nonpayable","inputs":[
            {"type":"address"},
            {"type":"uint256[]"},
            {"type":"tuple","components":[{"type":"bool"},{"type":"bytes2"}]}
        ],"outputs":[]}])
        .to_string(),
    )
    .unwrap();
    let actors = Actors::derive(Default::default(), &[actor("alice")]).unwrap();
    let alice = actors.get("alice").unwrap().address;
    let data = abi
        .encode(
            &actors,
            "f(address,uint256[],(bool,bytes2))",
            &[
                "$alice".into(),
                "[\"1\",\"2\"]".into(),
                "[true,\"0xabcd\"]".into(),
            ],
        )
        .unwrap();
    assert_eq!(Address::from_slice(&data[16..36]), alice);
    assert!(
        abi.encode(&actors, "f(address,uint256[],(bool,bytes2))", &[])
            .is_err()
    );
    assert!(
        abi.encode(
            &actors,
            "f(address,uint256[],(bool,bytes2))",
            &["$mallory".into(), "[]".into(), "[true,\"0xabcd\"]".into()]
        )
        .unwrap_err()
        .contains("unknown actor")
    );
}

#[test]
fn attack_surface_maps_signatures_observers_and_seeds() {
    let surface = AttackSurface::map(&MapAttackSurfaceArgs {
        source_code: "contract UnsafeVault { function withdraw(uint256 a) external { payable(msg.sender).call{value: a}(\"\"); } }".into(),
        contract: vault(),
    })
    .unwrap();
    let surface = serde_json::to_value(surface).unwrap();
    let entry = |signature: &str| {
        surface["entry_points"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["signature"] == signature)
            .cloned()
            .unwrap_or_else(|| panic!("no entry point {signature}"))
    };
    assert_eq!(entry("deposit()")["mutability"], "payable");
    assert_eq!(entry("withdraw(uint256)")["mutability"], "nonpayable");
    assert_eq!(surface["state_observers"], json!(["balances(address)"]));
    let seeds = surface["attack_hypothesis_seeds"].as_array().unwrap();
    for needle in ["reentrancy", "Native-USDC", "value extraction"] {
        assert!(
            seeds.iter().any(|s| s.as_str().unwrap().contains(needle)),
            "missing {needle} seed: {seeds:?}"
        );
    }
}

#[test]
fn sessions_are_bounded_and_evict_the_least_recent() {
    let mut sessions = Sessions::default();
    for i in 0..=64 {
        sessions.open(format!("s{i}"), vault());
    }
    assert!(
        sessions.get_mut("s0").is_err(),
        "the oldest session was evicted"
    );
    assert!(sessions.get_mut("s64").is_ok());
    assert!(sessions.close("s64"));
    assert!(!sessions.close("s64"));
}

#[test]
fn preamble_assigns_attack_selection_to_the_agent_and_matches_the_host_contract() {
    let preamble = super::PREAMBLE;
    for needle in [
        "You choose the actors",
        "A revert is evidence about a precondition",
        "Do not say a contract is safe",
        "copy `snapshot`, `chain_id`, `watch`, and `traces` verbatim",
        "engine_error",
    ] {
        assert!(preamble.contains(needle), "preamble lacks `{needle}`");
    }
    assert!(!preamble.contains("watch_addresses"));
}

#[test]
fn start_and_round_args_deserialize_from_model_json() {
    let start: FuzzStartArgs = serde_json::from_value(json!({
        "chain_id": 5042,
        "hypothesis": "h",
        "actors": [{"name": "alice"}],
        "actions": [{"label": "d", "actor": "alice", "function": "deposit()", "value": "1"}],
        "invariants": [{"name": "p", "actor": "alice", "function": "deposit()", "expected_status": "reverted"}]
    }))
    .expect("flat start args parse");
    assert_eq!(start.plan.max_depth, 4, "defaults apply");
    assert!(
        serde_json::from_value::<FuzzStartArgs>(json!({"chain_id": 5042, "plan": {}})).is_err(),
        "a nested plan is rejected"
    );
    let round: FuzzRoundArgs = serde_json::from_value(json!({
        "replay": [{
            "id": "1-1",
            "statuses": ["succeeded", "engine_error"],
            "probes": [{"label": "read", "status": "succeeded", "return_data": "0x01"}],
            "balances": [{"address": "0x01", "before": "1", "after": "2"}]
        }]
    }))
    .expect("compact host output parses");
    assert_eq!(round.replay[0].statuses[1], ReplayStatus::EngineError);
}
