//! Curated tool layer for Dune Analytics. Hand-written from the generated
//! client in `aomi_ext::dune` — see ext/specs/dune.yaml for the full surface.
//!
//! Intent: run SQL queries on Dune (saved or raw), wait for completion, fetch
//! results. The 47 mechanical tools from `aomi-build gen-tool` were collapsed
//! into 5 user-centric ones, all centred on the SQL execute → poll → fetch
//! workflow:
//!
//!   * `dune_run_query`           — composite execute → poll → fetch (saved query)
//!   * `dune_run_sql`             — composite execute → poll → fetch (raw SQL)
//!   * `dune_get_latest_results`  — fetch the latest cached results, no execution
//!   * `dune_get_execution_status`— inspect an in-flight execution
//!   * `dune_list_my_queries`     — discover saved queries owned by the API key

use aomi_ext::dune::Client as DuneClient;
use aomi_ext::dune::types::{ModelsExecuteSqlRequest, ModelsExecuteSqlRequestPerformance};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub(crate) struct DuneApp;

const BASE_URL: &str = "https://api.dune.com/api";

// Polling cadence and bounds. Dune query latency varies a lot; the user can
// extend the wall-clock deadline via `max_wait_seconds`, but the iteration cap
// is a non-tunable belt-and-suspenders guard.
const POLL_INTERVAL_SECS: u64 = 2;
const DEFAULT_MAX_WAIT_SECS: u64 = 60;
const MAX_POLL_ITERATIONS: u32 = 600;

// ============================================================================
// Helpers
// ============================================================================

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value).map_err(|e| format!("[dune] serialize: {e}"))?;
    Ok(match value {
        Value::Object(mut m) => {
            m.insert("source".into(), Value::String("dune".into()));
            Value::Object(m)
        }
        other => json!({ "source": "dune", "data": other }),
    })
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[dune] runtime: {e}"))
}

fn resolve_key(ctx: &DynToolCallCtx, arg: Option<&str>) -> Result<String, String> {
    resolve_secret_value(
        ctx,
        arg,
        "DUNE_API_KEY",
        "[dune] missing api_key argument and DUNE_API_KEY env var",
    )
}

/// Poll an execution to terminal state, then fetch its result rows. Bounded by
/// both a wall-clock deadline and an iteration cap. Returns the raw results
/// payload as the typed `ModelsResultsResponse`.
async fn poll_and_fetch(
    client: &DuneClient,
    execution_id: &str,
    api_key: &str,
    max_wait: Duration,
) -> Result<Value, String> {
    let deadline = Instant::now() + max_wait;
    let mut iterations: u32 = 0;
    loop {
        if iterations >= MAX_POLL_ITERATIONS {
            return Err(format!(
                "[dune] poll loop exceeded {MAX_POLL_ITERATIONS} iterations for execution {execution_id}"
            ));
        }
        iterations += 1;

        let s = client
            .getv1_execution_executionid_status(execution_id, None, api_key)
            .await
            .map_err(|e| format!("[dune] status {execution_id}: {e}"))?
            .into_inner();
        let state = s.state.clone().unwrap_or_default();
        match state.as_str() {
            "QUERY_STATE_COMPLETED" => break,
            "QUERY_STATE_FAILED" | "QUERY_STATE_CANCELLED" => {
                return Err(format!(
                    "[dune] execution {execution_id} ended in state {state}"
                ));
            }
            _ => {}
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "[dune] timed out after {}s waiting for execution {execution_id} (last state: {state})",
                max_wait.as_secs()
            ));
        }
        tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }

    let results = client
        .getv1_execution_executionid_results(
            execution_id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            api_key,
        )
        .await
        .map_err(|e| format!("[dune] fetch results {execution_id}: {e}"))?
        .into_inner();
    ok(results)
}

// ============================================================================
// Tool 1: run_query — composite execute → poll → fetch (saved query by ID)
// ============================================================================

pub(crate) struct RunQuery;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct RunQueryArgs {
    /// API credential (falls back to env var DUNE_API_KEY).
    pub api_key: Option<String>,
    /// Numeric Dune query ID (visible in the query URL on dune.com, e.g. 1234567).
    pub query_id: i64,
    /// Optional URL-encoded query parameters (e.g. "param1=value1&param2=value2").
    #[serde(default)]
    pub query_parameters: Option<String>,
    /// Maximum seconds to wait for the query to finish before erroring out (default 60).
    #[serde(default)]
    pub max_wait_seconds: Option<u64>,
}

impl DynAomiTool for RunQuery {
    type App = DuneApp;
    type Args = RunQueryArgs;
    const NAME: &'static str = "dune_run_query";
    const DESCRIPTION: &'static str = "Execute a saved Dune SQL query by its query ID, wait for it to finish, and return the result rows. Use this when the user names a Dune query (or query ID) and wants the up-to-date data. Polls execution status every 2 seconds until QUERY_STATE_COMPLETED, then fetches results in one tool call. Bounded by max_wait_seconds (default 60).";

    fn run(_app: &DuneApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let max_wait = Duration::from_secs(args.max_wait_seconds.unwrap_or(DEFAULT_MAX_WAIT_SECS));
        let query_id = args.query_id;
        let query_parameters = args.query_parameters.clone();
        let runtime = rt()?;
        runtime.block_on(async move {
            let client = DuneClient::new(BASE_URL);
            let exec = client
                .postv1_query_queryid_execute(
                    query_id,
                    None,
                    None,
                    query_parameters.as_deref(),
                    api_key.as_str(),
                )
                .await
                .map_err(|e| format!("[dune] execute query {query_id}: {e}"))?
                .into_inner();
            let execution_id = exec
                .execution_id
                .clone()
                .ok_or_else(|| "[dune] execute response missing execution_id".to_string())?;
            poll_and_fetch(&client, execution_id.as_str(), api_key.as_str(), max_wait).await
        })
    }
}

// ============================================================================
// Tool 2: run_sql — composite execute → poll → fetch (raw SQL string)
// ============================================================================

pub(crate) struct RunSql;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct RunSqlArgs {
    /// API credential (falls back to env var DUNE_API_KEY).
    pub api_key: Option<String>,
    /// Raw SQL to execute against Dune's catalog (e.g. "SELECT * FROM ethereum.transactions LIMIT 10").
    pub sql: String,
    /// Performance tier: omit to use the plan's default (free plans reject an
    /// explicit tier); otherwise "small", "medium", or "large". Use "large"
    /// only for queries that scan a lot of data.
    #[serde(default)]
    pub performance: Option<String>,
    /// Maximum seconds to wait for the query to finish (default 60).
    #[serde(default)]
    pub max_wait_seconds: Option<u64>,
}

impl DynAomiTool for RunSql {
    type App = DuneApp;
    type Args = RunSqlArgs;
    const NAME: &'static str = "dune_run_sql";
    const DESCRIPTION: &'static str = "Run an ad-hoc SQL query against Dune's catalog without needing to save it first. Wait for the execution to complete and return the result rows in one tool call. Use when the user gives you a SQL string (or asks an analytical question you can answer with one). Polls every 2 seconds until QUERY_STATE_COMPLETED. Bounded by max_wait_seconds (default 60).";

    fn run(_app: &DuneApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let max_wait = Duration::from_secs(args.max_wait_seconds.unwrap_or(DEFAULT_MAX_WAIT_SECS));
        let perf = match args
            .performance
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("medium") => Some(ModelsExecuteSqlRequestPerformance::Medium),
            Some("large") => Some(ModelsExecuteSqlRequestPerformance::Large),
            Some("small") => Some(ModelsExecuteSqlRequestPerformance::Small),
            // Omit the field so Dune applies the plan's default tier: free
            // plans reject an explicit `small` with "performance tier is not
            // available with your subscription".
            None => None,
            Some(other) => {
                return Err(format!(
                    "[dune] performance must be small|medium|large, got {other}"
                ));
            }
        };
        let body = ModelsExecuteSqlRequest {
            performance: perf,
            sql: args.sql,
        };
        let runtime = rt()?;
        runtime.block_on(async move {
            let client = DuneClient::new(BASE_URL);
            let exec = client
                .postv1_sql_execute(None, api_key.as_str(), &body)
                .await
                .map_err(|e| format!("[dune] execute raw sql: {e}"))?
                .into_inner();
            let execution_id = exec
                .execution_id
                .clone()
                .ok_or_else(|| "[dune] execute response missing execution_id".to_string())?;
            poll_and_fetch(&client, execution_id.as_str(), api_key.as_str(), max_wait).await
        })
    }
}

// ============================================================================
// Tool 3: get_latest_results — cached results without re-execution
// ============================================================================

pub(crate) struct GetLatestResults;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLatestResultsArgs {
    pub api_key: Option<String>,
    /// Dune query ID. Returns the most recent execution's results without
    /// triggering a new run — useful for queries that auto-refresh.
    pub query_id: String,
    /// Limit rows returned (default Dune-side: 1000).
    #[serde(default)]
    pub limit: Option<i64>,
    /// Offset for paging.
    #[serde(default)]
    pub offset: Option<i64>,
}

impl DynAomiTool for GetLatestResults {
    type App = DuneApp;
    type Args = GetLatestResultsArgs;
    const NAME: &'static str = "dune_get_latest_results";
    const DESCRIPTION: &'static str = "Get the most recent cached results of a saved Dune query without re-executing it. Cheaper and faster than `dune_run_query` — prefer this when the user just wants to read a query whose schedule already keeps it fresh.";

    fn run(_app: &DuneApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let runtime = rt()?;
        runtime.block_on(async move {
            let client = DuneClient::new(BASE_URL);
            let r = client
                .getv1_query_queryid_results(
                    args.query_id.as_str(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    args.limit,
                    args.offset,
                    None,
                    None,
                    api_key.as_str(),
                )
                .await
                .map_err(|e| format!("[dune] latest results {}: {e}", args.query_id))?
                .into_inner();
            ok(r)
        })
    }
}

// ============================================================================
// Tool 4: get_execution_status — inspect an in-flight execution
// ============================================================================

pub(crate) struct GetExecutionStatus;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetExecutionStatusArgs {
    pub api_key: Option<String>,
    /// Execution ID returned by an earlier query execution.
    pub execution_id: String,
}

impl DynAomiTool for GetExecutionStatus {
    type App = DuneApp;
    type Args = GetExecutionStatusArgs;
    const NAME: &'static str = "dune_get_execution_status";
    const DESCRIPTION: &'static str = "Check the state of a Dune query execution by its execution_id. Returns one of QUERY_STATE_PENDING, QUERY_STATE_EXECUTING, QUERY_STATE_COMPLETED, QUERY_STATE_FAILED, QUERY_STATE_CANCELLED, plus timing/cost info. Only needed when inspecting an execution started elsewhere (e.g. one that timed out from `dune_run_query`); the run tools wait for completion themselves.";

    fn run(_app: &DuneApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let runtime = rt()?;
        runtime.block_on(async move {
            let client = DuneClient::new(BASE_URL);
            let s = client
                .getv1_execution_executionid_status(
                    args.execution_id.as_str(),
                    None,
                    api_key.as_str(),
                )
                .await
                .map_err(|e| format!("[dune] status {}: {e}", args.execution_id))?
                .into_inner();
            ok(s)
        })
    }
}

// ============================================================================
// Tool 5: list_my_queries — saved queries owned by the API key
// ============================================================================

pub(crate) struct ListMyQueries;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ListMyQueriesArgs {
    pub api_key: Option<String>,
    /// Max queries returned (default Dune-side).
    #[serde(default)]
    pub limit: Option<i64>,
    /// Offset for paging.
    #[serde(default)]
    pub offset: Option<i64>,
}

impl DynAomiTool for ListMyQueries {
    type App = DuneApp;
    type Args = ListMyQueriesArgs;
    const NAME: &'static str = "dune_list_my_queries";
    const DESCRIPTION: &'static str = "List Dune SQL queries owned by the account tied to the API key. Returns IDs, names, and metadata you can pass to `dune_run_query` or `dune_get_latest_results`. Use when the user asks 'what queries do I have' or wants to discover their saved queries before running one.";

    fn run(_app: &DuneApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let runtime = rt()?;
        runtime.block_on(async move {
            let client = DuneClient::new(BASE_URL);
            let r = client
                .getv1_queries(None, args.limit, args.offset, api_key.as_str())
                .await
                .map_err(|e| format!("[dune] list queries: {e}"))?
                .into_inner();
            ok(r)
        })
    }
}
