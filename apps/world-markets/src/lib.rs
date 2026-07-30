//! `world-markets` — reference app for the **app-scoped skill** construct.
//!
//! A compact prediction-market trading app whose model-facing contract lives
//! in a structured skill block instead of one flat preamble: named markdown
//! sections (instructions / workflows / action_rules / safety), a guard
//! table the host's compiled interpreter enforces on the staging surface,
//! and — unlike host skills — activation purely by app binding. Nothing here
//! is reachable through `activate_skills` or visible from any other app.
//!
//! The market catalog is deliberately self-contained (no external API): the
//! app exists to exercise the construct end-to-end — sectioned instructions
//! baked into the composed preamble, `world_build_trade` emitting router
//! calldata, and the guard table vetting the staged transaction (allowed
//! contract, allowed selector, chain scope, notional caps).

use aomi_sdk::*;

mod client;
mod tool;

const PREAMBLE: &str = "You are the World Markets trading agent. Your full \
operating contract — workflows, action rules, and safety limits — is in the \
Application Skill sections below.";

dyn_aomi_app!(
    app = client::WorldMarketsApp,
    name = "world-markets",
    version = "0.3.0",
    preamble = PREAMBLE,
    tools = [
        tool::ListWorldMarkets,
        tool::GetWorldMarket,
        tool::PreviewWorldTrade,
        tool::BuildWorldTrade,
    ],
    namespaces = ["evm-core"],
    skill = {
        id: "world-markets/trading",
        sections: {
            instructions: "skill/instructions.md",
            workflows: "skill/workflows.md",
            action_rules: "skill/action-rules.md",
            safety: "skill/safety.md",
        },
        guard: "skill/guard.json",
    },
);
