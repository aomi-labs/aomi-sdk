//! The model-authored campaign plan and its validated, executable form.

use std::collections::BTreeSet;
use std::str::FromStr;

use alloy_primitives::{Address, B256, Bytes, U256};
use aomi_sdk::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::abi::{Actors, ContractAbi};
use crate::replay::{ReplayBatch, ReplayCall, ReplayStatus};

/// Compiler output for the contract under test.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FuzzContract {
    pub name: String,
    /// Compiler-produced JSON ABI array, encoded as a JSON string.
    pub abi: String,
    /// 0x-prefixed deployed (runtime) bytecode, copied verbatim.
    pub deployed_bytecode: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct FuzzActor {
    /// Unique lowercase identifier, e.g. `alice` or `attacker`.
    pub name: String,
    /// Initial native balance in atomic units, decimal or 0x-hex.
    #[serde(default = "FuzzActor::default_balance")]
    pub balance: String,
}

impl FuzzActor {
    pub const MAX_NAME: usize = 32;

    fn default_balance() -> String {
        "10000000000000000000".into()
    }

    pub fn validate(&self) -> Result<(), String> {
        let valid = !self.name.is_empty()
            && self.name.len() <= Self::MAX_NAME
            && self
                .name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if valid {
            Ok(())
        } else {
            Err(format!(
                "actor name `{}` must be 1..={} lowercase letters, digits or underscores",
                self.name,
                Self::MAX_NAME
            ))
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct FuzzCall {
    /// Human-readable trace label, not a Solidity function name.
    pub label: String,
    /// Actor name declared in this plan.
    pub actor: String,
    /// Exact canonical ABI signature, for example `withdraw(uint256)`.
    pub function: String,
    /// ABI arguments as strings. Arrays and tuples are JSON text; `$name`
    /// is replaced by that actor's address.
    #[serde(default)]
    pub arguments: Vec<String>,
    /// Native value in atomic units, decimal or 0x-hex.
    #[serde(default = "FuzzCall::zero")]
    pub value: String,
}

impl FuzzCall {
    fn zero() -> String {
        "0".into()
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExpectedStatus {
    /// Security assertion: an unexpected success is a violation.
    Reverted,
    /// Coverage pressure only; never a finding on its own, because the
    /// plan cannot express the preconditions that make success required.
    Succeeded,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FuzzProperty {
    pub name: String,
    /// Actor whose matching action this property constrains.
    pub actor: String,
    /// Exact canonical signature of an action in this plan.
    pub function: String,
    #[serde(default)]
    pub expected_status: Option<ExpectedStatus>,
    /// Maximum native balance increase for `actor` over the whole trace, in
    /// atomic units.
    #[serde(default)]
    pub max_actor_profit: Option<String>,
    /// Read-only view/pure calls evaluated after every trace.
    #[serde(default)]
    pub probes: Vec<FuzzCall>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FuzzPlan {
    pub hypothesis: String,
    pub actors: Vec<FuzzActor>,
    /// Writable action templates. Every writable entrypoint needs one.
    pub actions: Vec<FuzzCall>,
    pub invariants: Vec<FuzzProperty>,
    /// Longest call sequence explored.
    #[serde(default = "FuzzPlan::default_depth")]
    pub max_depth: usize,
    /// Exploration budget in executed traces. Confirming a violation
    /// (minimization and reproduction) may run past it.
    #[serde(default = "FuzzPlan::default_executions")]
    pub max_executions: usize,
}

impl FuzzPlan {
    /// The contract plus every actor must fit the host's watch list.
    pub const MAX_ACTORS: usize = ReplayBatch::MAX_WATCH - 1;
    pub const MAX_ACTIONS: usize = 24;
    pub const MAX_INVARIANTS: usize = 24;
    pub const MAX_DEPTH: usize = ReplayBatch::MAX_CALLS;
    pub const MAX_EXECUTIONS: usize = 256;

    fn default_depth() -> usize {
        4
    }

    fn default_executions() -> usize {
        32
    }

    /// Validate against the contract and resolve every call to calldata.
    pub fn prepare(
        &self,
        abi: &ContractAbi,
        artifact: B256,
        target: Address,
    ) -> Result<PreparedPlan, String> {
        let bounded = |name: &str, len: usize, max: usize| {
            if (1..=max).contains(&len) {
                Ok(())
            } else {
                Err(format!("{name} must be 1..={max}, got {len}"))
            }
        };
        if self.hypothesis.trim().is_empty() {
            return Err("hypothesis is required".into());
        }
        bounded("actors", self.actors.len(), Self::MAX_ACTORS)?;
        bounded("actions", self.actions.len(), Self::MAX_ACTIONS)?;
        bounded("invariants", self.invariants.len(), Self::MAX_INVARIANTS)?;
        bounded("max_depth", self.max_depth, Self::MAX_DEPTH)?;
        bounded("max_executions", self.max_executions, Self::MAX_EXECUTIONS)?;

        let actors = Actors::derive(artifact, &self.actors)?;
        let prepare = |call: &FuzzCall, read_only: bool| -> Result<PreparedCall, String> {
            let function = abi.function(&call.function)?;
            if ContractAbi::read_only(function) != read_only {
                return Err(if read_only {
                    format!("probe `{}` must be a view or pure function", call.function)
                } else {
                    format!("action `{}` must be a writable function", call.function)
                });
            }
            Ok(PreparedCall {
                input: call.clone(),
                from: actors.get(&call.actor)?.address,
                to: target,
                calldata: abi.encode(&actors, &call.function, &call.arguments)?,
                value: U256::from_str(&call.value)
                    .map_err(|_| format!("`{}` has an invalid value", call.label))?,
            })
        };

        let actions = self
            .actions
            .iter()
            .map(|call| prepare(call, false))
            .collect::<Result<Vec<_>, _>>()?;
        let planned: BTreeSet<_> = self.actions.iter().map(|a| a.function.as_str()).collect();
        let missing: Vec<_> = abi
            .writable()
            .into_iter()
            .filter(|signature| !planned.contains(signature.as_str()))
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "plan needs at least one action for every writable entrypoint; missing: {}",
                missing.join(", ")
            ));
        }

        let mut properties = Vec::with_capacity(self.invariants.len());
        let mut probes: Vec<PreparedCall> = Vec::new();
        for property in &self.invariants {
            actors.get(&property.actor)?;
            if !self
                .actions
                .iter()
                .any(|a| a.actor == property.actor && a.function == property.function)
            {
                return Err(format!(
                    "property `{}` must match an action by actor and function",
                    property.name
                ));
            }
            if property.expected_status.is_none() && property.max_actor_profit.is_none() {
                return Err(format!(
                    "property `{}` needs expected_status or max_actor_profit",
                    property.name
                ));
            }
            for probe in &property.probes {
                let prepared = prepare(probe, true)?;
                if !probes.iter().any(|known| known.same_call(&prepared)) {
                    probes.push(prepared);
                }
            }
            properties.push(PreparedProperty {
                name: property.name.clone(),
                actor: property.actor.clone(),
                function: property.function.clone(),
                expected: property.expected_status,
                max_profit: property
                    .max_actor_profit
                    .as_deref()
                    .map(U256::from_str)
                    .transpose()
                    .map_err(|_| {
                        format!(
                            "property `{}` has an invalid max_actor_profit",
                            property.name
                        )
                    })?,
                actor_address: actors.get(&property.actor)?.address,
            });
        }
        if probes.len() > ReplayBatch::MAX_PROBES {
            return Err(format!(
                "plan declares {} distinct probes; at most {} fit one trace",
                probes.len(),
                ReplayBatch::MAX_PROBES
            ));
        }
        Ok(PreparedPlan {
            actors,
            actions,
            properties,
            probes,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedCall {
    pub input: FuzzCall,
    pub from: Address,
    pub to: Address,
    pub calldata: Bytes,
    pub value: U256,
}

impl PreparedCall {
    fn same_call(&self, other: &Self) -> bool {
        (self.from, &self.calldata, self.value) == (other.from, &other.calldata, other.value)
    }

    pub fn replay(&self) -> ReplayCall {
        ReplayCall {
            label: self.input.label.clone(),
            from: format!("{:#x}", self.from),
            to: format!("{:#x}", self.to),
            data: self.calldata.to_string(),
            value: self.value.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedProperty {
    pub name: String,
    pub actor: String,
    pub function: String,
    pub expected: Option<ExpectedStatus>,
    pub max_profit: Option<U256>,
    pub actor_address: Address,
}

impl PreparedProperty {
    /// Whether this step, within a trace whose actor gained `profit`,
    /// breaks the property. Only a conclusive status can violate.
    pub fn violated_by(&self, call: &FuzzCall, status: ReplayStatus, profit: U256) -> bool {
        if call.actor != self.actor || call.function != self.function || !status.conclusive() {
            return false;
        }
        let unexpected_success =
            self.expected == Some(ExpectedStatus::Reverted) && status.succeeded();
        let excess_profit = self.max_profit.is_some_and(|max| profit > max);
        unexpected_success || excess_profit
    }
}

pub(crate) struct PreparedPlan {
    pub actors: Actors,
    pub actions: Vec<PreparedCall>,
    pub properties: Vec<PreparedProperty>,
    /// Distinct probes across every property, evaluated after each trace.
    pub probes: Vec<PreparedCall>,
}
