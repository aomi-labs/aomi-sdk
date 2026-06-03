use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;

use crate::route::{
    Enforcement, EnforcementPolicy, EnforcementStep, RouteStep, RouteTrigger, ToolReturn,
};
use crate::types::DynAomiTool;

/// Type-level convenience for naming a routed target tool. The blanket impl
/// over [`DynAomiTool`] means an app's own tools auto-qualify; the
/// `add::<MyTool>(...)` / `after::<MyTool>(...)` builder methods simply
/// inline `MyTool::NAME` so callers don't have to repeat the string.
///
/// Stable host-provided tools can use marker types from [`host`]. Truly
/// dynamic tool names should still go through `add_named` / `after_named`.
pub trait RouteTarget {
    fn tool_name() -> &'static str;
}

impl<T> RouteTarget for T
where
    T: DynAomiTool,
{
    fn tool_name() -> &'static str {
        T::NAME
    }
}

/// Canonical marker targets for stable host-provided tools in the `common`
/// namespace. These are name-only wrappers for the route builder; they do not
/// model args or tool behavior.
pub mod host {
    use super::RouteTarget;

    macro_rules! host_target {
        // Accept any number of attributes (including `///` doc comments)
        // and forward them to the generated struct so docs.rs renders
        // them. Existing call sites without attributes still match.
        ($(#[$attr:meta])* $name:ident, $tool:literal) => {
            $(#[$attr])*
            #[derive(Debug, Clone, Copy, Default)]
            pub struct $name;

            impl RouteTarget for $name {
                fn tool_name() -> &'static str {
                    $tool
                }
            }
        };
    }

    host_target!(BraveSearch, "brave_search");
    host_target!(CommitTx, "commit_tx");
    host_target!(CommitTxs, "commit_txs");
    host_target!(EvmCommitMessage, "evm_commit_message");
    host_target!(StageTx, "stage_tx");
    host_target!(SimulateBatch, "simulate_batch");
    host_target!(ViewState, "view_state");
    host_target!(RunTx, "run_tx");
    host_target!(GetTimeAndOnchainContext, "get_time_and_onchain_context");
    host_target!(GetContract, "get_contract");
    host_target!(GetAccountInfo, "get_account_info");
    host_target!(SyncChain, "sync_chain");

    // ── SVM (Solana) primitives ──────────────────────────────────────────
    //
    // Route-target markers for the SVM verbs in the `svm-core` namespace
    // family (split into `svm-reads`, `svm-ix-broadcast`,
    // `svm-ix-sign`, `svm-tx-broadcast`, `svm-tx-sign`,
    // `svm-sign-data`, `svm-bundle` on the host side per ADR 0004
    // § Decision B). App `build_*` tools emit route plans that drive
    // these as continuations — mirroring how EVM apps use `EvmCommitMessage`
    // for the typed-data signing step.
    //
    // Three lanes from ADR 0003 § Decision A:
    //   - Lane 1 (ix list)        — `SvmStageIx`  → `svm_stage_ix`
    //   - Lane 2 (received tx)    — `SvmStageTx`  → `svm_stage_tx`
    //                               (host #38-pipeline-b2 landed iter
    //                               39 follow-up; ADR 0005 storage
    //                               primitive `SvmPending::Tx` ships
    //                               with it). Apps that receive a
    //                               base64 `VersionedTransaction` from
    //                               a venue (e.g. byreal `build-swap-tx`,
    //                               Jupiter `/swap`) stage it through
    //                               this marker and downstream verbs
    //                               consume the minted `tx_id`.
    //   - Lane 3 (off-chain msg)  — `SvmSignData` → `svm_sign_data`
    //
    // Two consumers per ADR 0003 § Decision B + ADR 0004 § C.2:
    //   - Stage / sim / commit (lane-symmetric per ADR 0003 § Decision A):
    //       `SvmStageIx`     → `svm_stage_ix`     (Lane 1 producer)
    //       `SvmStageTx`     → `svm_stage_tx`     (Lane 2 producer)
    //       `SvmSimulateIx`  → `svm_simulate_ix`  (Lane 1 sim consumer)
    //       `SvmSimulateTx`  → `svm_simulate_tx`  (Lane 2 sim consumer)
    //       `SvmCommitIx`    → `svm_commit_ix`    (Lane 1 commit; takes
    //                          `ix_ids` + assembly args + `mode`)
    //       `SvmCommitTx`    → `svm_commit_tx`    (Lane 2 commit; takes
    //                          `tx_id` + `mode`; blob authoritative)
    //                          Both commit tools carry a `mode`; wallet
    //                          is the currently shipped app-facing path.
    //   - Sign-only (app-broadcast pattern, ADR 0004 § C.2):
    //       `SvmSignTx`      → `svm_sign_tx`; byreal-style apps wrap
    //                          signing here, then POST signed bytes to
    //                          their own venue endpoint.
    //
    // Lane-symmetric note: stage / simulate split per tool name, NOT per
    // XOR arg. The lane lives in the tool the LLM picks; arg shapes are
    // non-discriminated. This mirrors how the host's `finalize_stage_tx`
    // already dispatches by `attribute.name` (call_consumer/svm.rs).
    //
    // Args contracts and bound-artifact shapes are documented at each
    // verb's host-side implementation in product-mono
    // `aomi/crates/tools/src/svm/`. Apps that need to inspect them
    // should follow the host-tool docstring as the source of truth.
    //
    // Naming convention: every SVM marker is `Svm*` (PascalCase) →
    // `svm_*` (snake_case). The host is the single source of truth for
    // the snake_case verb name.

    // Lane 1 producer — stage a `Vec<Instruction>`. Returns one
    // `pending_ix_id` per instruction; downstream simulate / commit
    // verbs consume the id list. ADR 0003 § Decision A.
    host_target!(SvmStageIx, "svm_stage_ix");

    // Lane 2 producer — stage a base64 `VersionedTransaction` blob
    // received from a venue (e.g. byreal `/dex/v2/build-swap-tx`,
    // Jupiter `/swap`, Raydium tx-API). The host decodes, validates
    // payer = connected wallet, then stages the blob under a fresh
    // `pending_tx_id`. Downstream verbs (`SvmSimulateTx`,
    // `SvmCommitTx`, `SvmSignTx`) consume that id. ADR 0003 §
    // Decision A + ADR 0005 storage primitive; host
    // #38-pipeline-b2 (iter 39 follow-up).
    //
    // Args contract:
    //   { "tx": "<base64 VersionedTransaction>",
    //     "description": "...",          // optional, surfaces in UI
    //     "kind": "..." }                // optional, free-form tag
    //
    // The direct inline form for `SvmSignTx`
    // (`{ "unsigned_tx": "<base64>", ... }`) remains supported, but
    // Lane 2 staging is the preferred path when the app also needs
    // simulate or route a single `tx_id` through multiple consumers.
    //
    // `preserve_blockhash: bool` defaults true for byte-stable
    // venue-validated flows like byreal preData/data byte-compare.
    host_target!(SvmStageTx, "svm_stage_tx");

    // Lane 1 simulate consumer — assembles the staged ix list into a
    // VersionedTransaction and simulates. Args contract:
    //   { "ix_ids": [<u32>, ...],
    //     "version": "legacy" | "v0",                 // optional
    //     "address_lookup_tables": ["<pubkey>", ...], // optional (v0 only)
    //     "compute_units": <u32>,                     // optional
    //     "priority_microlamports": <u64>,            // optional
    //     "mode": "litesvm" | "rpc",                  // optional, see ADR 0002
    //     "replace_recent_blockhash": <bool>,         // optional, default true
    //     "sig_verify": <bool>,                       // optional, default false
    //     "accounts": ["<pubkey>", ...] }             // optional address filter
    //
    // Rejects ids that resolve to `svm_stage_tx`-staged blobs with a
    // "use svm_simulate_tx" hint. Mirrors the host's lane symmetry
    // (split landed alongside this SDK bump).
    host_target!(SvmSimulateIx, "svm_simulate_ix");

    // Lane 2 simulate consumer — simulates a `svm_stage_tx`-staged
    // tx blob as-is. The blob's version / ALTs / blockhash / compute
    // budget are preserved; there are no assembly args, because the
    // blob's metadata is authoritative. Args contract:
    //   { "tx_id": <u32>,
    //     "mode": "litesvm" | "rpc",          // optional
    //     "replace_recent_blockhash": <bool>, // optional, default false
    //     "sig_verify": <bool>,               // optional, default false
    //     "accounts": ["<pubkey>", ...] }     // optional address filter
    //
    // Rejects ids that resolve to `svm_stage_ix`-staged instructions
    // with a "use svm_simulate_ix" hint.
    host_target!(SvmSimulateTx, "svm_simulate_tx");

    // Lane 1 commit consumer — assemble the staged ix list into one
    // VersionedTransaction and request wallet approval. Args contract:
    //   { "ix_ids": [<u32>, ...],
    //     "version": "legacy" | "v0",                 // optional
    //     "address_lookup_tables": ["<pubkey>", ...], // optional (v0 only)
    //     "compute_units": <u32>,                     // optional
    //     "priority_microlamports": <u64>,            // optional
    //     "mode": "wallet" | "internal-rpc" }         // optional, see below
    //
    // Mode (shared with `SvmCommitTx` Lane 2):
    //   - `wallet` (default): host wallet SDK signs + broadcasts via
    //     signAndSendTransaction. The only working path today.
    //   - `internal-rpc`: runtime signs and submits via its own RPC +
    //     confirm loop. Errors loud until host #38-pipeline-c lands
    //     the broadcast loop + `WalletCallback::Tx*` variants.
    //
    // Rejects ids that resolve to `svm_stage_tx`-staged blobs with a
    // "use svm_commit_tx" hint.
    host_target!(SvmCommitIx, "svm_commit_ix");

    // Lane 2 commit consumer — request wallet approval for a
    // `svm_stage_tx`-staged transaction blob. The blob's version / ALTs /
    // blockhash / compute budget are preserved; there are no assembly
    // args, because the blob's metadata is authoritative. Args contract:
    //   { "tx_id": <u32>,
    //     "mode": "wallet" | "internal-rpc" }         // optional
    //
    // Same `mode` semantics as `SvmCommitIx`. Rejects ids that resolve
    // to `svm_stage_ix`-staged instructions with a "use svm_commit_ix"
    // hint.
    host_target!(SvmCommitTx, "svm_commit_tx");

    // Sign-only verb for the **app-broadcast** pattern (ADR 0004 §
    // C.2). byreal-style apps wrap signing here, then forward the
    // signed bytes to their own venue endpoint (e.g. byreal
    // `/dex/v2/send-swap-tx`, Jupiter `/execute`, Raydium tx-API).
    // Args contract (transitional pre-#39-svm-apps-b):
    //   { "unsigned_tx": "<base64>", "description": "..." }
    // Args contract (Lane 2 staged, post-#39-svm-apps-b):
    //   { "tx_id": <u32>, "description": "..." }
    // Bound artifact (string): base64 signed tx bytes.
    //
    // Note: there is intentionally no `SvmSignTxs` (plural) — Solana
    // wallets sign one tx per user prompt. Apps that need multiple
    // signed txs should issue separate `SvmSignTx` route steps.
    host_target!(SvmSignTx, "svm_sign_tx");

    // Lane 3 producer + consumer — off-chain message signing for
    // commit-reveal flows, Squads proposal payloads, wallet-attested
    // intents. Host-side renamed from `svm_commit_message` in iter 39
    // (ADR 0003 OQ #3 closed). Args contract:
    //   { "message_base64": "...", "description": "...",
    //     "domain": {...}, "kind": "..." }
    host_target!(SvmSignData, "svm_sign_data");
}

#[derive(Debug, Clone)]
struct DeferredRouteStep {
    step: RouteStep,
    /// Aliases the after-step waits on. `.awaits(a)` pushes one; `.awaits_all([..])`
    /// extends. At `try_build` time: 1 alias → `OnBoundEvent`, 2+ → `OnAllBoundEvents`.
    awaited_aliases: Vec<String>,
}

pub struct RouteBuilder {
    value: Value,
    next_steps: Vec<RouteStep>,
    after_step: Option<DeferredRouteStep>,
    errors: Vec<String>,
}

impl RouteBuilder {
    pub(super) fn new(value: Value) -> Self {
        Self {
            value,
            next_steps: Vec::new(),
            after_step: None,
            errors: Vec::new(),
        }
    }

    pub fn next(mut self, f: impl FnOnce(&mut NextRoutesBuilder<'_>)) -> Self {
        let mut next = NextRoutesBuilder { route: &mut self };
        f(&mut next);
        self
    }

    pub fn after<T>(self, args: impl Serialize) -> AfterStepBuilder
    where
        T: RouteTarget,
    {
        self.after_named(T::tool_name(), args)
    }

    pub fn after_named(
        mut self,
        tool: impl Into<String>,
        args: impl Serialize,
    ) -> AfterStepBuilder {
        if self.after_step.is_some() {
            self.errors
                .push("RouteBuilder v1 supports at most one deferred `after` step".to_string());
        } else {
            self.after_step = Some(DeferredRouteStep {
                step: RouteStep {
                    tool: tool.into(),
                    args: serde_json::to_value(args).unwrap_or(Value::Null),
                    trigger: RouteTrigger::OnBoundEvent {
                        alias: String::new(),
                    },
                    bind_as: None,
                    prompt: None,
                    enforcement: None,
                },
                awaited_aliases: Vec::new(),
            });
        }

        AfterStepBuilder { route: self }
    }

    pub fn try_build(mut self) -> Result<ToolReturn, String> {
        let mut aliases = BTreeSet::new();
        let enforced_producer_count = self
            .next_steps
            .iter()
            .filter(|step| step.enforcement.is_some())
            .count();
        if enforced_producer_count > 1 {
            self.errors.push(
                "RouteBuilder v1 supports at most one enforced producer in `next(...)`".to_string(),
            );
        }
        for step in &self.next_steps {
            if let Some(alias) = step.bind_as.as_deref() {
                record_route_alias(&mut aliases, &mut self.errors, alias);
            }
            if let Some(enforcement) = step.enforcement.as_ref() {
                for alias in step_enforcement_aliases(enforcement) {
                    record_route_alias(&mut aliases, &mut self.errors, alias);
                }
            }
        }

        for step in &self.next_steps {
            if let Some(alias) = step.bind_as.as_deref() {
                if !matches!(step.trigger, RouteTrigger::OnSyncReturn) {
                    self.errors.push(format!(
                        "bound artifact alias `{alias}` must be attached to an immediate `next` step"
                    ));
                }
                if !step.args.is_object() {
                    self.errors.push(format!(
                        "bound artifact producer `{}` must use object args in RouteBuilder v1",
                        step.tool
                    ));
                }
            }
            if step.enforcement.is_some() && !step.args.is_object() {
                self.errors.push(format!(
                    "enforced producer `{}` must use object args in RouteBuilder v1",
                    step.tool
                ));
            }
        }

        if let Some(after) = self.after_step.as_mut() {
            if after.awaited_aliases.is_empty() {
                self.errors
                    .push("deferred `after(...)` step is missing `.awaits(\"alias\")`".to_string());
                return if self.errors.is_empty() {
                    Ok(ToolReturn::with_routes(self.value, self.next_steps))
                } else {
                    Err(self.errors.join("\n"))
                };
            }

            if !after.step.args.is_object() {
                self.errors.push(format!(
                    "deferred route step `{}` must use object args so the awaited alias can be injected",
                    after.step.tool
                ));
            }
            let mut seen = BTreeSet::new();
            for awaited in &after.awaited_aliases {
                if awaited.trim().is_empty() {
                    self.errors
                        .push("deferred route awaits alias must not be empty".to_string());
                } else if !aliases.contains(awaited) {
                    self.errors.push(format!(
                        "deferred route awaits unknown alias `{awaited}`; produce it in `next(...)` or the attached enforcement first"
                    ));
                } else if !seen.insert(awaited.clone()) {
                    self.errors.push(format!(
                        "deferred route awaits alias `{awaited}` more than once"
                    ));
                }
            }
            after.step.trigger = if after.awaited_aliases.len() == 1 {
                RouteTrigger::OnBoundEvent {
                    alias: after.awaited_aliases.remove(0),
                }
            } else {
                RouteTrigger::OnAllBoundEvents {
                    aliases: std::mem::take(&mut after.awaited_aliases),
                }
            };
        }

        if !self.errors.is_empty() {
            return Err(self.errors.join("\n"));
        }

        let mut routes = self.next_steps;
        if let Some(after) = self.after_step {
            routes.push(after.step);
        }
        Ok(ToolReturn::with_routes(self.value, routes))
    }

    pub fn build(self) -> ToolReturn {
        self.try_build()
            .unwrap_or_else(|err| panic!("invalid RouteBuilder plan: {err}"))
    }
}

fn step_enforcement_aliases(enforcement: &Enforcement) -> impl Iterator<Item = &str> {
    enforcement_aliases(enforcement).into_iter()
}

fn record_route_alias(aliases: &mut BTreeSet<String>, errors: &mut Vec<String>, alias: &str) {
    if alias.trim().is_empty() {
        errors.push("bound alias must not be empty".to_string());
    } else if !aliases.insert(alias.to_string()) {
        errors.push(format!("duplicate bound alias `{alias}` in route plan"));
    }
}

fn enforcement_aliases(enforcement: &Enforcement) -> Vec<&str> {
    enforcement
        .steps
        .iter()
        .filter_map(EnforcementStep::bound_alias)
        .collect()
}

pub struct NextRoutesBuilder<'a> {
    route: &'a mut RouteBuilder,
}

impl<'a> NextRoutesBuilder<'a> {
    pub fn add<T>(&mut self, args: impl Serialize) -> NextStepBuilder<'_>
    where
        T: RouteTarget,
    {
        self.push_step(T::tool_name(), args)
    }

    pub fn add_named(
        &mut self,
        tool: impl Into<String>,
        args: impl Serialize,
    ) -> NextStepBuilder<'_> {
        self.push_step(tool, args)
    }

    fn push_step(&mut self, tool: impl Into<String>, args: impl Serialize) -> NextStepBuilder<'_> {
        let index = self.route.next_steps.len();
        self.route.next_steps.push(RouteStep::on_return(
            tool.into(),
            serde_json::to_value(args).unwrap_or(Value::Null),
        ));
        NextStepBuilder {
            route: self.route,
            index,
        }
    }
}

pub struct NextStepBuilder<'a> {
    route: &'a mut RouteBuilder,
    index: usize,
}

impl<'a> NextStepBuilder<'a> {
    /// Publish this step's terminal result Value under the given alias.
    /// Continuations declared via `after(...).awaits(alias)` or
    /// `after(...).awaits_all([..])` consume it.
    ///
    /// Aliases must be unique within a route plan, but the *tool name* does not
    /// have to be — a single plan may have multiple `evm_commit_message` / `stage_tx`
    /// / `svm_sign_tx` steps each binding to a distinct alias. The runtime
    /// consumes aliases in FIFO order per tool name, so list the steps in the
    /// order you expect the LLM/user to drive them (use `.note(...)` to
    /// reinforce the order in the suggested-action prompt).
    pub fn bind_as(self, alias: impl Into<String>) -> Self {
        self.route.next_steps[self.index].bind_as = Some(alias.into());
        self
    }

    pub fn note(self, note: impl Into<String>) -> Self {
        self.route.next_steps[self.index].prompt = Some(note.into());
        self
    }

    pub fn enforce(
        self,
        on_failure: EnforcementPolicy,
        f: impl FnOnce(&mut EnforcementBuilder<'_>),
    ) -> Self {
        let mut steps = Vec::new();
        let mut builder = EnforcementBuilder { steps: &mut steps };
        f(&mut builder);
        self.route.next_steps[self.index].enforcement = Some(Enforcement { steps, on_failure });
        self
    }
}

pub struct EnforcementBuilder<'a> {
    steps: &'a mut Vec<EnforcementStep>,
}

impl<'a> EnforcementBuilder<'a> {
    pub fn add<T>(&mut self, args: impl Serialize) -> EnforcementStepBuilder<'_>
    where
        T: RouteTarget,
    {
        self.add_named(T::tool_name(), args)
    }

    pub fn add_named(
        &mut self,
        tool: impl Into<String>,
        args: impl Serialize,
    ) -> EnforcementStepBuilder<'_> {
        let index = self.steps.len();
        self.steps.push(EnforcementStep {
            tool: tool.into(),
            args: serde_json::to_value(args).unwrap_or(Value::Null),
            bind_as: None,
        });
        EnforcementStepBuilder {
            steps: self.steps,
            index,
        }
    }
}

pub struct EnforcementStepBuilder<'a> {
    steps: &'a mut Vec<EnforcementStep>,
    index: usize,
}

impl<'a> EnforcementStepBuilder<'a> {
    /// Publish this enforced step's terminal result Value under the given alias.
    pub fn bind_as(self, alias: impl Into<String>) -> Self {
        self.steps[self.index].bind_as = Some(alias.into());
        self
    }
}

pub struct AfterStepBuilder {
    route: RouteBuilder,
}

impl AfterStepBuilder {
    /// Wait for the named artifact alias produced earlier in this route plan.
    /// Calling this more than once accumulates aliases — the after-step then
    /// fires only when **all** awaited aliases are bound, with each bound
    /// value injected into its args under the alias's name. Equivalent to
    /// `.awaits_all([..])`.
    pub fn awaits(mut self, alias: impl Into<String>) -> Self {
        if let Some(after) = self.route.after_step.as_mut() {
            after.awaited_aliases.push(alias.into());
        }
        self
    }

    /// Wait for **all** of the named aliases. The after-step fires only when
    /// every alias in `aliases` has been bound; each bound value is injected
    /// into the after-step args under its alias name.
    ///
    /// Use this for combined-signing flows (e.g. 0x gasless: Permit2
    /// approval + Settler metaTransaction trade) where the submit tool needs
    /// every signature in one call.
    pub fn awaits_all<I, S>(mut self, aliases: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        if let Some(after) = self.route.after_step.as_mut() {
            after
                .awaited_aliases
                .extend(aliases.into_iter().map(Into::into));
        }
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        if let Some(after) = self.route.after_step.as_mut() {
            after.step.prompt = Some(note.into());
        }
        self
    }

    pub fn next(mut self, f: impl FnOnce(&mut NextRoutesBuilder<'_>)) -> RouteBuilder {
        let mut next = NextRoutesBuilder {
            route: &mut self.route,
        };
        f(&mut next);
        self.route
    }

    pub fn build(self) -> ToolReturn {
        self.route.build()
    }

    pub fn try_build(self) -> Result<ToolReturn, String> {
        self.route.try_build()
    }
}
