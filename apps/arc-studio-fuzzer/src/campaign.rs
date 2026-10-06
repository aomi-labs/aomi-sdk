//! One session's fuzz campaign: a bounded breadth-first search over call
//! sequences, run on the host's `evm-sim` world. The app owns the search
//! (frontier, state deduplication, property checks, minimization and the
//! two-reproduction rule); the host only executes the batches it emits.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::str::FromStr;

use alloy_primitives::{Address, B256, Bytes, U256, keccak256};
use aomi_sdk::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::abi::ContractAbi;
use crate::plan::{FuzzCall, FuzzContract, FuzzPlan, PreparedPlan};
use crate::replay::{
    BASELINE, CompactTrace, ReplayBalance, ReplayBatch, ReplayProbe, ReplayStatus, ReplayTrace,
};

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FuzzStartArgs {
    /// Arc chain of the open simulation world: 5042 (mainnet) or 5042002 (testnet).
    pub chain_id: u64,
    #[serde(flatten)]
    pub plan: FuzzPlan,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FuzzRoundArgs {
    /// Every compact trace from the last `sim_replay_traces` call, unchanged.
    pub replay: Vec<CompactTrace>,
    /// Optional revised plan. Actors must stay identical.
    #[serde(default)]
    pub plan: Option<FuzzPlan>,
}

impl FuzzContract {
    /// The runtime bytecode, checked against the EIP-170 size limit.
    pub fn runtime(&self) -> Result<Bytes, String> {
        let code = Bytes::from_str(self.deployed_bytecode.trim())
            .map_err(|_| "deployed_bytecode must be 0x-prefixed hex".to_string())?;
        if code.is_empty() || code.len() > FuzzCampaign::MAX_RUNTIME_BYTES {
            return Err(format!(
                "deployed_bytecode must be 1..={} bytes, got {}",
                FuzzCampaign::MAX_RUNTIME_BYTES,
                code.len()
            ));
        }
        Ok(code)
    }
}

#[derive(Debug, Clone)]
enum Work {
    Explore(Vec<usize>),
    Minimize(Vec<usize>),
    Reproduce(Vec<usize>),
}

impl Work {
    fn trace(&self) -> &[usize] {
        match self {
            Self::Explore(trace) | Self::Minimize(trace) | Self::Reproduce(trace) => trace,
        }
    }
}

/// A violating trace waiting to be shrunk and reproduced.
#[derive(Debug, Clone)]
struct Suspect {
    property: usize,
    trace: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Minimizing,
    Reproducing,
}

#[derive(Debug, Clone)]
struct Confirmation {
    property: usize,
    best: Vec<usize>,
    stage: Stage,
}

#[derive(Debug, Clone, Serialize)]
struct FindingStep {
    label: String,
    actor: String,
    function: String,
    status: ReplayStatus,
}

#[derive(Debug, Clone, Serialize)]
struct Finding {
    property: String,
    minimized_trace: Vec<FuzzCall>,
    steps: Vec<FindingStep>,
    balances: Vec<ReplayBalance>,
    probes: Vec<ReplayProbe>,
    reproductions: usize,
}

#[derive(Debug, Clone, Serialize)]
struct Unconfirmed {
    property: String,
    trace: Vec<FuzzCall>,
    reason: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum CampaignStatus {
    Exploring,
    Confirming,
    Complete,
}

#[derive(Debug, Serialize)]
struct Setup {
    set_code: Value,
    set_balances: Vec<Value>,
    snapshot: Value,
}

// Every batch the campaign emits must fit one sim_replay_traces call.
const _: () = assert!(
    FuzzCampaign::EXPLORE_BATCH <= ReplayBatch::MAX_TRACES
        && FuzzPlan::MAX_DEPTH <= ReplayBatch::MAX_TRACES
);

pub(crate) struct FuzzCampaign {
    id: B256,
    chain_id: u64,
    contract: FuzzContract,
    abi: ContractAbi,
    artifact: B256,
    target: Address,
    hypothesis: String,
    max_depth: usize,
    max_executions: usize,
    plan: PreparedPlan,
    writable: BTreeSet<String>,
    frontier: VecDeque<Vec<usize>>,
    pending: HashMap<String, Work>,
    /// State fingerprints already expanded: the search corpus.
    expanded: BTreeSet<B256>,
    covered: BTreeSet<String>,
    behavior: BTreeSet<String>,
    suspects: VecDeque<Suspect>,
    confirming: Option<Confirmation>,
    findings: Vec<Finding>,
    unconfirmed: Vec<Unconfirmed>,
    round: u32,
    executions: usize,
    inconclusive: usize,
    serial: u64,
}

impl FuzzCampaign {
    pub const ARC_CHAINS: [u64; 2] = [5042, 5_042_002];
    pub const MAX_RUNTIME_BYTES: usize = 24_576;
    /// Traces per exploration batch. Every trace transits the model twice,
    /// so this stays well under the host's own limit.
    const EXPLORE_BATCH: usize = 8;
    const REPRODUCTIONS: usize = 2;
    const LIMITATIONS: [&'static str; 3] = [
        "Runtime bytecode installation does not execute constructors; constructor-initialised storage is absent.",
        "Coverage measures writable entrypoints and distinct observed states, not opcode coverage.",
        "A completed budget is evidence for the tested properties, not proof of contract safety.",
    ];

    pub fn new(contract: FuzzContract, args: FuzzStartArgs) -> Result<Self, String> {
        if !Self::ARC_CHAINS.contains(&args.chain_id) {
            return Err(format!(
                "Arc fuzzing supports chain {:?}, got {}",
                Self::ARC_CHAINS,
                args.chain_id
            ));
        }
        let abi = ContractAbi::parse(&contract.abi)?;
        let runtime = contract.runtime()?;
        let artifact = keccak256([contract.abi.as_bytes(), runtime.as_ref()].concat());
        let target = Address::from_slice(&artifact[12..]);
        let plan = args.plan.prepare(&abi, artifact, target)?;
        let mut campaign = Self {
            id: keccak256([artifact.as_slice(), args.plan.hypothesis.as_bytes()].concat()),
            chain_id: args.chain_id,
            writable: abi.writable(),
            contract,
            abi,
            artifact,
            target,
            hypothesis: args.plan.hypothesis,
            max_depth: args.plan.max_depth,
            max_executions: args.plan.max_executions,
            plan,
            frontier: VecDeque::new(),
            pending: HashMap::new(),
            expanded: BTreeSet::new(),
            covered: BTreeSet::new(),
            behavior: BTreeSet::new(),
            suspects: VecDeque::new(),
            confirming: None,
            findings: Vec::new(),
            unconfirmed: Vec::new(),
            round: 0,
            executions: 0,
            inconclusive: 0,
            serial: 0,
        };
        campaign.frontier = (0..campaign.plan.actions.len()).map(|i| vec![i]).collect();
        Ok(campaign)
    }

    /// Setup operations for the host, then the first batch.
    pub fn start(&mut self) -> Value {
        let setup = Setup {
            set_code: json!({
                "chain_id": self.chain_id,
                "address": format!("{:#x}", self.target),
                "runtime_bytecode": self.contract.deployed_bytecode.trim(),
            }),
            set_balances: self
                .plan
                .actors
                .iter()
                .map(|(name, actor)| {
                    json!({
                        "chain_id": self.chain_id,
                        "actor": name,
                        "address": format!("{:#x}", actor.address),
                        "balance": actor.balance.to_string(),
                    })
                })
                .collect(),
            snapshot: json!({ "label": BASELINE }),
        };
        json!({
            "campaign_id": self.id,
            "setup": setup,
            "next_batch": self.next_batch(),
            "report": self.report(),
        })
    }

    pub fn round(&mut self, args: FuzzRoundArgs) -> Result<Value, String> {
        self.ingest(args.replay)?;
        self.round += 1;
        // A revision re-indexes actions, which would orphan an open
        // confirmation; defer it instead of failing after ingestion.
        let revision = match args.plan {
            None => None,
            Some(_) if self.confirming.is_some() || !self.suspects.is_empty() => Some(
                "deferred: a violation is being confirmed; resend the revised plan after it completes",
            ),
            Some(plan) => {
                self.revise(plan)?;
                Some("applied")
            }
        };
        Ok(json!({
            "revision": revision,
            "next_batch": self.next_batch(),
            "report": self.report(),
        }))
    }

    pub fn report(&self) -> Value {
        json!({
            "campaign_id": self.id,
            "hypothesis": self.hypothesis,
            "status": self.status(),
            "round": self.round,
            "executions": self.executions,
            "max_executions": self.max_executions,
            "inconclusive_traces": self.inconclusive,
            "corpus_size": self.expanded.len(),
            "frontier_size": self.frontier.len(),
            "covered_writes": self.covered,
            "uncovered_writes": self.writable.difference(&self.covered).collect::<Vec<_>>(),
            "behavior_coverage": self.behavior,
            "findings": self.findings,
            "unconfirmed": self.unconfirmed,
            "limitations": Self::LIMITATIONS,
        })
    }

    fn status(&self) -> CampaignStatus {
        if self.confirming.is_some() || !self.suspects.is_empty() {
            CampaignStatus::Confirming
        } else if !self.pending.is_empty() {
            CampaignStatus::Exploring
        } else {
            CampaignStatus::Complete
        }
    }

    fn revise(&mut self, plan: FuzzPlan) -> Result<(), String> {
        let prepared = plan.prepare(&self.abi, self.artifact, self.target)?;
        if prepared.actors != self.plan.actors {
            return Err("a revised plan cannot change actors, addresses, or balances".into());
        }
        // Carry the search over: queued traces keep their place when every
        // action they use survives, and new actions start at depth one.
        // Re-sending an unchanged plan is therefore a no-op.
        let remap: Vec<Option<usize>> = self
            .plan
            .actions
            .iter()
            .map(|old| {
                prepared
                    .actions
                    .iter()
                    .position(|new| new.input == old.input)
            })
            .collect();
        let added = (0..prepared.actions.len())
            .filter(|new| !remap.contains(&Some(*new)))
            .map(|new| vec![new]);
        self.frontier = self
            .frontier
            .drain(..)
            .filter_map(|trace| trace.iter().map(|old| remap[*old]).collect())
            .chain(added)
            .collect();
        self.hypothesis = plan.hypothesis;
        self.max_depth = plan.max_depth;
        self.max_executions = plan.max_executions;
        self.plan = prepared;
        Ok(())
    }

    fn confirmed(&self, property: usize) -> bool {
        let name = &self.plan.properties[property].name;
        self.findings
            .iter()
            .any(|finding| &finding.property == name)
    }

    fn next_batch(&mut self) -> Option<ReplayBatch> {
        if self.confirming.is_none() {
            while let Some(suspect) = self.suspects.pop_front() {
                if !self.confirmed(suspect.property) {
                    self.confirming = Some(Confirmation {
                        property: suspect.property,
                        best: suspect.trace,
                        stage: Stage::Minimizing,
                    });
                    break;
                }
            }
        }
        let work: Vec<Work> = match self.confirming.as_mut() {
            Some(confirmation) => {
                if confirmation.best.len() <= 1 {
                    confirmation.stage = Stage::Reproducing;
                }
                match confirmation.stage {
                    Stage::Reproducing => {
                        vec![Work::Reproduce(confirmation.best.clone()); Self::REPRODUCTIONS]
                    }
                    Stage::Minimizing => (0..confirmation.best.len())
                        .map(|drop| {
                            let mut shorter = confirmation.best.clone();
                            shorter.remove(drop);
                            Work::Minimize(shorter)
                        })
                        .collect(),
                }
            }
            None => {
                let watch = self.plan.actors.len() + 1;
                let worst = self.max_depth + self.plan.probes.len() + 2 * watch;
                let capacity = (ReplayBatch::MAX_EXECUTIONS / worst)
                    .clamp(1, Self::EXPLORE_BATCH)
                    .min(self.max_executions.saturating_sub(self.executions));
                (0..capacity)
                    .map_while(|_| self.frontier.pop_front())
                    .map(Work::Explore)
                    .collect()
            }
        };
        if work.is_empty() {
            return None;
        }
        let probes: Vec<_> = self
            .plan
            .probes
            .iter()
            .map(|probe| probe.replay())
            .collect();
        let traces = work
            .into_iter()
            .map(|work| {
                self.serial += 1;
                let id = format!("{}-{}", self.round + 1, self.serial);
                let trace = ReplayTrace {
                    id: id.clone(),
                    calls: work
                        .trace()
                        .iter()
                        .map(|index| self.plan.actions[*index].replay())
                        .collect(),
                    probes: probes.clone(),
                };
                self.pending.insert(id, work);
                trace
            })
            .collect();
        Some(ReplayBatch {
            snapshot: BASELINE,
            chain_id: self.chain_id,
            watch: self
                .plan
                .actors
                .iter()
                .map(|(_, actor)| actor.address)
                .chain([self.target])
                .map(|address| format!("{address:#x}"))
                .collect(),
            traces,
        })
    }

    fn ingest(&mut self, replay: Vec<CompactTrace>) -> Result<(), String> {
        if self.pending.is_empty() {
            return Err("campaign has no pending replay batch".into());
        }
        if replay.len() != self.pending.len() {
            return Err(format!(
                "expected {} replayed traces, got {}; pass every compact trace unchanged",
                self.pending.len(),
                replay.len()
            ));
        }
        let ids: BTreeSet<&str> = replay.iter().map(|r| r.id.as_str()).collect();
        if ids.len() != replay.len() {
            return Err("replay repeats a trace id".into());
        }
        if let Some(unknown) = replay.iter().find(|r| !self.pending.contains_key(&r.id)) {
            return Err(format!("unknown replay trace id `{}`", unknown.id));
        }
        let observations: Vec<(Work, CompactTrace)> = replay
            .into_iter()
            .map(|report| {
                (
                    self.pending.remove(&report.id).expect("checked above"),
                    report,
                )
            })
            .collect();
        self.executions += observations.len();

        let mut evaluated = Vec::with_capacity(observations.len());
        for (work, report) in observations {
            let trace = work.trace();
            let conclusive = report.conclusive(trace.len());
            if !conclusive {
                self.inconclusive += 1;
            }
            let violations = if conclusive {
                self.violations(trace, &report)
            } else {
                Vec::new()
            };
            evaluated.push((work, report, conclusive, violations));
        }

        if let Some(confirmation) = self.confirming.take() {
            self.confirming = match confirmation.stage {
                Stage::Minimizing => {
                    let shorter = evaluated
                        .iter()
                        .find(|(_, _, _, violations)| violations.contains(&confirmation.property))
                        .map(|(work, ..)| work.trace().to_vec());
                    Some(match shorter {
                        Some(best) => Confirmation {
                            best,
                            ..confirmation
                        },
                        None => Confirmation {
                            stage: Stage::Reproducing,
                            ..confirmation
                        },
                    })
                }
                Stage::Reproducing => {
                    let reproduced = evaluated
                        .iter()
                        .all(|(_, _, _, violations)| violations.contains(&confirmation.property));
                    let calls = confirmation
                        .best
                        .iter()
                        .map(|i| self.plan.actions[*i].input.clone())
                        .collect::<Vec<_>>();
                    let property = self.plan.properties[confirmation.property].name.clone();
                    if reproduced {
                        let (_, report, _, _) = &evaluated[0];
                        self.findings.push(Finding {
                            property,
                            steps: calls
                                .iter()
                                .zip(&report.statuses)
                                .map(|(call, status)| FindingStep {
                                    label: call.label.clone(),
                                    actor: call.actor.clone(),
                                    function: call.function.clone(),
                                    status: *status,
                                })
                                .collect(),
                            minimized_trace: calls,
                            balances: report.balances.clone(),
                            probes: report.probes.clone(),
                            reproductions: Self::REPRODUCTIONS,
                        });
                    } else {
                        self.unconfirmed.push(Unconfirmed {
                            property,
                            trace: calls,
                            reason: if evaluated.iter().all(|(_, _, conclusive, _)| *conclusive) {
                                "violation did not reproduce from the baseline"
                            } else {
                                "host engine error during reproduction"
                            },
                        });
                    }
                    None
                }
            };
            return Ok(());
        }

        for (work, report, conclusive, violations) in evaluated {
            let Work::Explore(trace) = work else { continue };
            for (index, status) in trace.iter().zip(&report.statuses) {
                let call = &self.plan.actions[*index].input;
                self.covered.insert(call.function.clone());
                self.behavior.insert(format!(
                    "{}:{}:{}",
                    call.actor,
                    call.function,
                    status.as_str()
                ));
            }
            if !conclusive {
                continue;
            }
            for property in violations {
                if !self.suspects.iter().any(|s| s.property == property) {
                    self.suspects.push_back(Suspect {
                        property,
                        trace: trace.clone(),
                    });
                }
            }
            if self.suspects.iter().any(|s| s.trace == trace) {
                continue;
            }
            let fingerprint = self.fingerprint(&trace, &report);
            if self.expanded.insert(fingerprint) && trace.len() < self.max_depth {
                for action in 0..self.plan.actions.len() {
                    let mut next = trace.clone();
                    next.push(action);
                    self.frontier.push_back(next);
                }
            }
        }
        Ok(())
    }

    /// Properties (not yet confirmed) this conclusive trace violates.
    fn violations(&self, trace: &[usize], report: &CompactTrace) -> Vec<usize> {
        let mut out = BTreeSet::new();
        for (index, property) in self.plan.properties.iter().enumerate() {
            if self.confirmed(index) {
                continue;
            }
            let actor = format!("{:#x}", property.actor_address);
            let profit = report
                .balances
                .iter()
                .find(|balance| balance.address.eq_ignore_ascii_case(&actor))
                .and_then(|balance| {
                    let before = U256::from_str(&balance.before).ok()?;
                    let after = U256::from_str(&balance.after).ok()?;
                    Some(after.saturating_sub(before))
                })
                .unwrap_or_default();
            let violated = trace.iter().zip(&report.statuses).any(|(action, status)| {
                property.violated_by(&self.plan.actions[*action].input, *status, profit)
            });
            if violated {
                out.insert(index);
            }
        }
        out.into_iter().collect()
    }

    /// Identity of the state a trace reached: changed balances and probe
    /// outputs. Without probes, storage-only effects are invisible, so the
    /// call sequence and its statuses are included to avoid over-pruning.
    fn fingerprint(&self, trace: &[usize], report: &CompactTrace) -> B256 {
        let mut balances: Vec<_> = report
            .balances
            .iter()
            .map(|b| (b.address.to_ascii_lowercase(), &b.after))
            .collect();
        balances.sort();
        let sequence = self
            .plan
            .probes
            .is_empty()
            .then_some((trace, &report.statuses));
        keccak256(
            serde_json::to_vec(&(balances, &report.probes, sequence))
                .expect("fingerprint inputs serialize"),
        )
    }
}
