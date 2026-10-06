use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use aomi_sdk::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::abi::ContractAbi;
use crate::campaign::FuzzCampaign;
use crate::plan::FuzzContract;

#[derive(Clone, Default)]
pub(crate) struct ArcStudioFuzzerApp {
    sessions: Arc<Mutex<Sessions>>,
}

impl ArcStudioFuzzerApp {
    pub(crate) fn sessions(&self) -> MutexGuard<'_, Sessions> {
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub(crate) struct Session {
    pub contract: FuzzContract,
    pub campaign: Option<FuzzCampaign>,
    touched: Instant,
}

/// Per-chat-session state. Bounded: a session the model never closes is
/// evicted once newer sessions need the room.
#[derive(Default)]
pub(crate) struct Sessions(HashMap<String, Session>);

impl Sessions {
    const MAX: usize = 64;

    /// Start (or restart) a session with a new compiler artifact.
    pub fn open(&mut self, id: String, contract: FuzzContract) {
        if !self.0.contains_key(&id) && self.0.len() >= Self::MAX {
            let oldest = self
                .0
                .iter()
                .min_by_key(|(_, session)| session.touched)
                .map(|(id, _)| id.clone());
            if let Some(oldest) = oldest {
                self.0.remove(&oldest);
            }
        }
        self.0.insert(
            id,
            Session {
                contract,
                campaign: None,
                touched: Instant::now(),
            },
        );
    }

    pub fn get_mut(&mut self, id: &str) -> Result<&mut Session, String> {
        let session = self.0.get_mut(id).ok_or_else(|| {
            "no compiler artifact for this session; call arc_map_attack_surface first".to_string()
        })?;
        session.touched = Instant::now();
        Ok(session)
    }

    pub fn close(&mut self, id: &str) -> bool {
        self.0.remove(id).is_some()
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MapAttackSurfaceArgs {
    /// Complete Solidity source. Include imported code when available.
    pub source_code: String,
    /// The compiler artifact the app keeps for this fuzz session.
    pub contract: FuzzContract,
}

#[derive(Debug, Serialize)]
struct EntryPoint {
    signature: String,
    mutability: alloy_json_abi::StateMutability,
    outputs: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AttackSurface {
    contract: String,
    entry_points: Vec<EntryPoint>,
    state_observers: Vec<String>,
    events: Vec<String>,
    custom_errors: Vec<String>,
    attack_hypothesis_seeds: BTreeSet<&'static str>,
    agent_next_action: &'static str,
}

impl AttackSurface {
    /// Function-name fragments and the attack family they suggest.
    const NAME_SEEDS: [(&'static [&'static str], &'static str); 4] = [
        (
            &["withdraw", "redeem", "claim", "transfer", "sweep", "drain"],
            "Unauthorized or over-sized value extraction across user roles",
        ),
        (
            &["deposit", "mint", "stake", "borrow", "repay"],
            "Cross-call accounting and ordering around asset/share creation",
        ),
        (
            &[
                "owner",
                "admin",
                "role",
                "pause",
                "upgrade",
                "initialize",
                "set",
            ],
            "Privilege escalation, unsafe initialization, or role confusion",
        ),
        (
            &["execute", "call", "hook", "callback", "flash"],
            "Arbitrary call, callback ordering, or reentrancy reachability",
        ),
    ];

    /// Source fragments and the attack family they suggest.
    const SOURCE_SEEDS: [(&'static str, &'static str); 9] = [
        (
            "delegatecall",
            "Delegatecall target/control and storage-context takeover",
        ),
        (
            "selfdestruct",
            "Destruction reachability and forced value movement",
        ),
        (
            "tx.origin",
            "tx.origin authorization confusion through an intermediary",
        ),
        (
            "ecrecover",
            "Signature replay, malleability, nonce, and domain separation",
        ),
        (
            "block.timestamp",
            "Timestamp-dependent state transitions at boundary values",
        ),
        (
            ".call{",
            "External callback/reentrancy before state finalization",
        ),
        (
            "assembly",
            "Assembly memory/storage assumptions and arbitrary slot access",
        ),
        (
            "initializer",
            "Initialization ordering and uninitialized takeover",
        ),
        (
            "upgrade",
            "Upgrade authorization and implementation-slot takeover",
        ),
    ];

    const PAYABLE_SEED: &'static str =
        "Native-USDC accounting: vary msg.value across callers and ordering";

    pub fn map(args: &MapAttackSurfaceArgs) -> Result<Self, String> {
        if args.contract.name.trim().is_empty() {
            return Err("contract name is required".into());
        }
        if args.source_code.trim().is_empty() {
            return Err("complete source_code is required for agentic analysis".into());
        }
        args.contract.runtime()?;
        let abi = ContractAbi::parse(&args.contract.abi)?;

        let mut seeds = BTreeSet::new();
        let mut entry_points = Vec::new();
        let mut observers = Vec::new();
        for function in abi.functions() {
            let signature = function.signature();
            if ContractAbi::read_only(function) {
                observers.push(signature.clone());
            } else {
                let name = function.name.to_ascii_lowercase();
                seeds.extend(
                    Self::NAME_SEEDS
                        .iter()
                        .filter(|(parts, _)| parts.iter().any(|part| name.contains(part)))
                        .map(|(_, seed)| *seed),
                );
            }
            if function.state_mutability == alloy_json_abi::StateMutability::Payable {
                seeds.insert(Self::PAYABLE_SEED);
            }
            entry_points.push(EntryPoint {
                signature,
                mutability: function.state_mutability,
                outputs: function
                    .outputs
                    .iter()
                    .map(|output| output.selector_type().into_owned())
                    .collect(),
            });
        }
        let source = args.source_code.to_ascii_lowercase();
        seeds.extend(
            Self::SOURCE_SEEDS
                .iter()
                .filter(|(needle, _)| source.contains(needle))
                .map(|(_, seed)| *seed),
        );

        Ok(Self {
            contract: args.contract.name.clone(),
            entry_points,
            state_observers: observers,
            events: abi
                .inner()
                .events()
                .map(|event| event.name.clone())
                .collect(),
            custom_errors: abi
                .inner()
                .errors()
                .map(|error| error.name.clone())
                .collect(),
            attack_hypothesis_seeds: seeds,
            agent_next_action: "Turn the threat model into executable properties and ABI-level action templates, then call sim_open, arc_fuzz_start, and arc_fuzz_round until the campaign completes. These seeds are prompts, not findings.",
        })
    }
}
