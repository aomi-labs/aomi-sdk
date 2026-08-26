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
    // Route-target markers for the SVM verbs (host namespaces after the
    // 2026-07 lane recut: `svm-reads`, `svm-write-ix`, `svm-write-tx`,
    // plus the `svm-core` meta/union). App `build_*` tools emit route
    // plans that drive these as continuations — mirroring how EVM apps
    // use `EvmCommitMessage` for the typed-data signing step.
    //
    // Three lanes from ADR 0003 § Decision A:
    //   - Lane 1 (ix list)        — `SvmStageIx`  → `svm_stage_ix`
    //   - Lane 2 (received tx)    — `SvmStageTx`  → `svm_stage_tx`.
    //                               Apps that receive a base64
    //                               `VersionedTransaction` from a venue
    //                               (byreal `build-swap-tx`, Jupiter
    //                               `/swap`, Raydium tx-API) stage it
    //                               through this marker and downstream
    //                               verbs consume the minted `tx_id`.
    //   - Lane 3 (off-chain msg)  — `SvmSignData` → `svm_sign_data`
    //
    // Commit means one thing: "execute this staged transaction under
    // kernel policy". There is NO mode arg and NO sign-only verb. Two
    // orthogonal decisions route the call, neither made by the app's
    // tools or the model:
    //
    //   - WHO SIGNS — kernel policy on the user's wallet
    //     (`public_keys.signing_mode`): human-sync routes to the
    //     connected wallet, autonomous signs server-side via the
    //     delegated grant, denied rejects. Apps never see this axis.
    //   - WHO SUBMITS — the `Broadcaster` config
    //     (`"wallet" | "venue" | "aomi"`): the app manifest's
    //     `broadcast = { default, allowed }` declares the operator
    //     policy; a stage call may pin `broadcaster` explicitly for a
    //     flow with a hard venue constraint (RFQ fills). `"venue"`
    //     makes commit emit a sign-only request whose signed bytes
    //     return to the app's own `submit_*` tool — the pattern that
    //     used to be the separate `svm_sign_tx` verb.
    //
    // Lane-symmetric note: stage / simulate / commit split per tool
    // name, NOT per XOR arg. The lane lives in the tool the LLM picks.
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
    // `SvmCommitTx`) consume that id.
    //
    // Args contract:
    //   { "tx": "<base64 VersionedTransaction>",
    //     "description": "...",           // optional, surfaces in UI
    //     "kind": "...",                   // optional, free-form tag
    //     "preserve_blockhash": <bool>,   // optional, default true
    //     "broadcaster": "wallet" | "venue" | "aomi" }  // optional
    //
    // `broadcaster` stamps WHO SUBMITS on the staged artifact. Omitted →
    // the app manifest's `broadcast.default` (falling back to the host
    // default). Pin `"venue"` explicitly for flows with a hard venue
    // constraint (RFQ fills); the host validates against the manifest's
    // `broadcast.allowed`.
    //
    // `preserve_blockhash: bool` defaults true for byte-stable
    // venue-validated flows like byreal preData/data byte-compare;
    // venue-broadcast blobs must keep it true.
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
    // VersionedTransaction and execute it under kernel policy. Args
    // contract:
    //   { "ix_ids": [<u32>, ...],
    //     "version": "legacy" | "v0",                 // optional
    //     "address_lookup_tables": ["<pubkey>", ...], // optional (v0 only)
    //     "compute_units": <u32>,                     // optional
    //     "priority_microlamports": <u64>,            // optional
    //     "broadcaster": "wallet" | "venue" | "aomi" } // optional
    //
    // No mode arg. Lane 1 assembles at commit time, so `broadcaster`
    // rides the call instead of a staged blob — forward what the app's
    // build tool returned; it is not a model choice. Rejects ids that
    // resolve to `svm_stage_tx`-staged blobs with a "use svm_commit_tx"
    // hint.
    host_target!(SvmCommitIx, "svm_commit_ix");

    // Lane 2 commit consumer — execute a `svm_stage_tx`-staged
    // transaction blob under kernel policy. The blob's version / ALTs /
    // blockhash / compute budget / broadcaster are preserved; there are
    // no assembly or mode args, because the staged metadata is
    // authoritative. Args contract:
    //   { "tx_id": <u32> }
    //
    // Routing (host-side, see product-mono `svm/tx/commit.rs`):
    //   - staged `broadcaster: "wallet"` → FE wallet signs AND submits
    //     (`signAndSendTransaction`) — the classic attended flow.
    //   - staged `broadcaster: "venue"` → sign-only request; the signed
    //     bytes bind to the route alias and return to the app's
    //     `submit_*` continuation; the venue broadcasts. On an
    //     autonomous-armed wallet the kernel signs server-side and the
    //     bytes bind without any FE round-trip — same route plan,
    //     unattended-capable.
    //   - staged `broadcaster: "aomi"` → runtime broadcast loop
    //     (BroadcastEngine, host #38-pipeline-c).
    //
    // Bound artifact for the venue cell (string): base64 signed tx
    // bytes — bind it with `.bind_as("signed_tx")` and await it in the
    // app's submit continuation. Note: Solana wallets sign one tx per
    // user prompt; apps needing multiple signed txs should issue
    // separate stage + commit step pairs, each binding a distinct
    // alias. Rejects ids that resolve to `svm_stage_ix`-staged
    // instructions with a "use svm_commit_ix" hint.
    host_target!(SvmCommitTx, "svm_commit_tx");

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
    /// / `svm_commit_tx` steps each binding to a distinct alias. The runtime
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
