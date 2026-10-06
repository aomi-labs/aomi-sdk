use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::campaign::{FuzzCampaign, FuzzRoundArgs, FuzzStartArgs};
use crate::client::{ArcStudioFuzzerApp, AttackSurface, MapAttackSurfaceArgs};

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EmptyArgs {}

pub(crate) struct MapAttackSurface;

impl DynAomiTool for MapAttackSurface {
    type App = ArcStudioFuzzerApp;
    type Args = MapAttackSurfaceArgs;
    const NAME: &'static str = "arc_map_attack_surface";
    const DESCRIPTION: &'static str = "Map a compiled Arc contract's callable surface and source-level risk signals before attacking it. Call this first with the complete source and compiler artifact; the app keeps the artifact for this session. Returns exact signatures, read-only observers, and hypothesis seeds. It does not choose a fuzz corpus and never claims vulnerabilities.";

    fn run(app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let surface = AttackSurface::map(&args)?;
        app.sessions().open(ctx.session_id, args.contract);
        serde_json::to_value(surface).map_err(|e| e.to_string())
    }
}

pub(crate) struct FuzzStart;

impl DynAomiTool for FuzzStart {
    type App = ArcStudioFuzzerApp;
    type Args = FuzzStartArgs;
    const NAME: &'static str = "arc_fuzz_start";
    const DESCRIPTION: &'static str = "Create this session's fuzz campaign from an executable plan: chain_id plus hypothesis, actors, actions, invariants, max_depth, and max_executions at the top level. Validates the plan against the kept artifact and returns evm-sim setup operations (sim_set_code, sim_set_balance, sim_snapshot) and the first replay batch.";

    fn run(app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let mut sessions = app.sessions();
        let session = sessions.get_mut(&ctx.session_id)?;
        let mut campaign = FuzzCampaign::new(session.contract.clone(), args)?;
        let result = campaign.start();
        session.campaign = Some(campaign);
        Ok(result)
    }
}

pub(crate) struct FuzzRound;

impl DynAomiTool for FuzzRound {
    type App = ArcStudioFuzzerApp;
    type Args = FuzzRoundArgs;
    const NAME: &'static str = "arc_fuzz_round";
    const DESCRIPTION: &'static str = "Pass every compact trace from the last sim_replay_traces call unchanged. The app checks properties, deduplicates states, extends the breadth-first frontier, minimizes and reproduces violations, and returns the next replay batch (null when the campaign is complete). Optionally include a revised plan with the same actors.";

    fn run(app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        app.sessions()
            .get_mut(&ctx.session_id)?
            .campaign
            .as_mut()
            .ok_or("no fuzz campaign for this session; call arc_fuzz_start first")?
            .round(args)
    }
}

pub(crate) struct FuzzReport;

impl DynAomiTool for FuzzReport {
    type App = ArcStudioFuzzerApp;
    type Args = EmptyArgs;
    const NAME: &'static str = "arc_fuzz_report";
    const DESCRIPTION: &'static str = "Read the current campaign report: status, coverage, confirmed findings with their minimized reproductions, and unconfirmed violations. Does not change the search or the simulation world.";

    fn run(app: &Self::App, _args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        Ok(app
            .sessions()
            .get_mut(&ctx.session_id)?
            .campaign
            .as_ref()
            .ok_or("no fuzz campaign for this session")?
            .report())
    }
}

pub(crate) struct FuzzClose;

impl DynAomiTool for FuzzClose {
    type App = ArcStudioFuzzerApp;
    type Args = EmptyArgs;
    const NAME: &'static str = "arc_fuzz_close";
    const DESCRIPTION: &'static str = "Delete this session's artifact and campaign state. Call sim_close separately to release the simulation world.";

    fn run(app: &Self::App, _args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let closed = app.sessions().close(&ctx.session_id);
        Ok(json!({ "status": "closed", "had_session": closed }))
    }
}
