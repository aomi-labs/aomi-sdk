use crate::client::*;
use aomi_sdk::*;
use serde_json::Value;

impl DynAomiTool for ExecuteQuery {
    type App = DuneApp;
    type Args = ExecuteQueryArgs;
    const NAME: &'static str = "execute_query";
    const DESCRIPTION: &'static str = "Execute a Dune SQL query by its numeric ID. Returns an execution_id to poll for status and results.";

    fn run(_app: &DuneApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DuneClient::new(&args.api_key)?;
        client.execute_query(args.query_id, args.query_parameters.as_ref())
    }
}

impl DynAomiTool for GetExecutionStatus {
    type App = DuneApp;
    type Args = GetExecutionStatusArgs;
    const NAME: &'static str = "get_execution_status";
    const DESCRIPTION: &'static str = "Poll the status of a running Dune query execution. Returns state (e.g. QUERY_STATE_COMPLETED).";

    fn run(_app: &DuneApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DuneClient::new(&args.api_key)?;
        client.get_execution_status(&args.execution_id)
    }
}

impl DynAomiTool for GetExecutionResults {
    type App = DuneApp;
    type Args = GetExecutionResultsArgs;
    const NAME: &'static str = "get_execution_results";
    const DESCRIPTION: &'static str = "Fetch result rows from a completed Dune query execution. Supports limit/offset pagination.";

    fn run(_app: &DuneApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DuneClient::new(&args.api_key)?;
        client.get_execution_results(&args.execution_id, args.limit, args.offset)
    }
}

impl DynAomiTool for GetQueryResults {
    type App = DuneApp;
    type Args = GetQueryResultsArgs;
    const NAME: &'static str = "get_query_results";
    const DESCRIPTION: &'static str = "Get the latest cached results for a Dune query by its numeric ID. Useful for community queries without re-executing.";

    fn run(_app: &DuneApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DuneClient::new(&args.api_key)?;
        client.get_query_results(args.query_id, args.limit, args.offset)
    }
}
