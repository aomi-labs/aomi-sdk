use aomi_sdk::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct DuneApp;

#[allow(unused_imports)]
pub(crate) use crate::tool::*;

// ============================================================================
// Dune Analytics Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_DUNE_API: &str = "https://api.dune.com/api/v1";

pub(crate) struct DuneClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_endpoint: String,
    pub(crate) api_key: String,
}

impl DuneClient {
    pub(crate) fn new(api_key: &str) -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[dune] build HTTP client failed: {e}"))?;
        Ok(Self {
            http,
            api_endpoint: std::env::var("DUNE_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_DUNE_API.to_string()),
            api_key: api_key.to_string(),
        })
    }

    fn get_json(&self, url: &str, op: &str) -> Result<Value, String> {
        let response = self
            .http
            .get(url)
            .header("X-DUNE-API-KEY", &self.api_key)
            .send()
            .map_err(|e| format!("[dune] {op} failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[dune] {op} failed: {status} {body}"));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[dune] {op} decode failed: {e}; body: {body}"))
    }

    fn post_json(&self, url: &str, payload: &Value, op: &str) -> Result<Value, String> {
        let response = self
            .http
            .post(url)
            .header("X-DUNE-API-KEY", &self.api_key)
            .json(payload)
            .send()
            .map_err(|e| format!("[dune] {op} failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[dune] {op} failed: {status} {body}"));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[dune] {op} decode failed: {e}; body: {body}"))
    }

    pub(crate) fn execute_query(
        &self,
        query_id: u64,
        query_parameters: Option<&Value>,
    ) -> Result<Value, String> {
        let url = format!("{}/query/{}/execute", self.api_endpoint, query_id);
        let payload = match query_parameters {
            Some(params) => serde_json::json!({ "query_parameters": params }),
            None => serde_json::json!({}),
        };
        self.post_json(&url, &payload, "execute_query")
    }

    pub(crate) fn get_execution_status(&self, execution_id: &str) -> Result<Value, String> {
        let url = format!("{}/execution/{}/status", self.api_endpoint, execution_id);
        self.get_json(&url, "get_execution_status")
    }

    pub(crate) fn get_execution_results(
        &self,
        execution_id: &str,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> Result<Value, String> {
        let mut url = format!("{}/execution/{}/results", self.api_endpoint, execution_id);
        let mut params = Vec::new();
        if let Some(l) = limit {
            params.push(format!("limit={l}"));
        }
        if let Some(o) = offset {
            params.push(format!("offset={o}"));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        self.get_json(&url, "get_execution_results")
    }

    pub(crate) fn get_query_results(
        &self,
        query_id: u64,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> Result<Value, String> {
        let mut url = format!("{}/query/{}/results", self.api_endpoint, query_id);
        let mut params = Vec::new();
        if let Some(l) = limit {
            params.push(format!("limit={l}"));
        }
        if let Some(o) = offset {
            params.push(format!("offset={o}"));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        self.get_json(&url, "get_query_results")
    }
}

// ============================================================================
// Tool structs & arg types
// ============================================================================

pub(crate) struct ExecuteQuery;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ExecuteQueryArgs {
    /// Dune API key for authentication.
    pub api_key: String,
    /// Numeric Dune query ID (from the dashboard URL).
    pub query_id: u64,
    /// Optional JSON object of query parameters that map to {{param}} placeholders in the SQL.
    #[serde(default)]
    pub query_parameters: Option<Value>,
}

pub(crate) struct GetExecutionStatus;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetExecutionStatusArgs {
    /// Dune API key for authentication.
    pub api_key: String,
    /// Execution ID returned by execute_query.
    pub execution_id: String,
}

pub(crate) struct GetExecutionResults;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetExecutionResultsArgs {
    /// Dune API key for authentication.
    pub api_key: String,
    /// Execution ID returned by execute_query.
    pub execution_id: String,
    /// Maximum number of rows to return.
    #[serde(default)]
    pub limit: Option<u64>,
    /// Number of rows to skip (for pagination).
    #[serde(default)]
    pub offset: Option<u64>,
}

pub(crate) struct GetQueryResults;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetQueryResultsArgs {
    /// Dune API key for authentication.
    pub api_key: String,
    /// Numeric Dune query ID.
    pub query_id: u64,
    /// Maximum number of rows to return.
    #[serde(default)]
    pub limit: Option<u64>,
    /// Number of rows to skip (for pagination).
    #[serde(default)]
    pub offset: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api_key_or_skip() -> Option<String> {
        match std::env::var("DUNE_API_KEY") {
            Ok(key) if !key.is_empty() => Some(key),
            _ => {
                println!("DUNE_API_KEY not set, skipping test");
                None
            }
        }
    }

    /// Story: "Run a query to track on-chain data and act on findings"
    /// Execute a query → poll status → fetch results when completed.
    #[test]
    fn execute_and_poll_workflow() {
        let api_key = match api_key_or_skip() {
            Some(k) => k,
            None => return,
        };

        let client = DuneClient::new(&api_key).expect("failed to create DuneClient");
        // Query 1747157: list of chains and block explorers (fast, reliable)
        let query_id: u64 = 1747157;
        println!("[execute_poll] Starting workflow for query_id={query_id}");

        // Step 1: Execute the query and get an execution_id back.
        println!("[execute_poll] Step 1: Executing query...");
        let exec_response = client
            .execute_query(query_id, None)
            .expect("execute_query failed");
        println!("[execute_poll] Step 1: execute_query response: {exec_response}");
        let execution_id = exec_response["execution_id"]
            .as_str()
            .expect("response should contain execution_id");
        println!("[execute_poll] Step 1: Got execution_id={execution_id}");
        assert!(!execution_id.is_empty(), "execution_id should not be empty");

        // Step 2: Poll execution status (a few attempts — free tier queues can be slow).
        println!("[execute_poll] Step 2: Polling execution status...");
        let mut state = String::new();
        for attempt in 1..=5 {
            std::thread::sleep(Duration::from_secs(2));
            let status_response = client
                .get_execution_status(execution_id)
                .expect("get_execution_status failed");
            state = status_response["state"]
                .as_str()
                .unwrap_or("UNKNOWN")
                .to_string();
            println!("[execute_poll] Step 2: attempt {attempt}, state={state}");
            if state == "QUERY_STATE_COMPLETED" || state == "QUERY_STATE_FAILED" {
                break;
            }
        }
        assert!(
            [
                "QUERY_STATE_PENDING",
                "QUERY_STATE_EXECUTING",
                "QUERY_STATE_COMPLETED"
            ]
            .contains(&state.as_str()),
            "state should be a valid Dune execution state, got {state}"
        );

        // Step 3: If completed, fetch results; otherwise verify the pipeline worked.
        if state == "QUERY_STATE_COMPLETED" {
            println!("[execute_poll] Step 3: Query completed, fetching results (limit=10)...");
            let results = client
                .get_execution_results(execution_id, Some(10), None)
                .expect("get_execution_results failed");
            let rows = results["result"]["rows"]
                .as_array()
                .expect("results should contain rows array");
            println!("[execute_poll] Step 3: Fetched {} rows", rows.len());
            for (i, row) in rows.iter().enumerate().take(5) {
                println!("[execute_poll]   row[{i}]: {row}");
            }
            assert!(!rows.is_empty(), "completed query should return rows");

            let metadata = &results["result"]["metadata"];
            println!("[execute_poll] Step 3: Metadata: {metadata}");
        } else {
            println!(
                "[execute_poll] Step 3: Query still {state} (free-tier queue), verifying cached results endpoint..."
            );
            // Use get_query_results to pull the most recent cached run instead
            let cached = client
                .get_query_results(query_id, Some(5), None)
                .expect("get_query_results (cached) failed");
            let cached_rows = cached["result"]["rows"]
                .as_array()
                .expect("cached results should contain rows");
            println!(
                "[execute_poll] Step 3: Fetched {} cached rows as fallback",
                cached_rows.len()
            );
            for (i, row) in cached_rows.iter().enumerate().take(3) {
                println!("[execute_poll]   cached_row[{i}]: {row}");
            }
            assert!(!cached_rows.is_empty(), "cached results should have rows");
        }

        println!(
            "[execute_poll] Workflow complete (execution_id={execution_id}, final_state={state})"
        );
    }

    /// Story: "Fetch cached analytics to inform a trading decision"
    /// Use get_query_results to pull the latest cached results without re-executing.
    #[test]
    fn cached_query_results_workflow() {
        let api_key = match api_key_or_skip() {
            Some(k) => k,
            None => return,
        };

        let client = DuneClient::new(&api_key).expect("failed to create DuneClient");
        // Same query 1747157 — should have cached results from our execute test or prior runs
        let query_id: u64 = 1747157;
        println!("[cached_results] Starting workflow for query_id={query_id}");

        // Step 1: Get cached query results.
        println!("[cached_results] Step 1: Fetching cached query results (limit=10)...");
        let response = client
            .get_query_results(query_id, Some(10), None)
            .expect("get_query_results failed");
        println!(
            "[cached_results] Step 1: Response keys: {:?}",
            response.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );

        // Step 2: Assert the response has the expected structure.
        let result = &response["result"];
        assert!(
            !result.is_null(),
            "response should contain a 'result' field"
        );
        println!("[cached_results] Step 2: 'result' field present");

        let rows = result["rows"]
            .as_array()
            .expect("result should contain a rows array");
        println!("[cached_results] Step 2: Row count = {}", rows.len());
        for (i, row) in rows.iter().enumerate().take(5) {
            println!("[cached_results]   row[{i}]: {row}");
        }
        assert!(!rows.is_empty(), "cached results should have rows");

        let metadata = &result["metadata"];
        assert!(!metadata.is_null(), "result should contain metadata");
        if let Some(column_names) = metadata["column_names"].as_array() {
            println!("[cached_results] Step 2: Column names: {:?}", column_names);
        }
        if let Some(column_types) = metadata["column_types"].as_array() {
            println!("[cached_results] Step 2: Column types: {:?}", column_types);
        }

        let execution_id = &response["execution_id"];
        println!("[cached_results] Execution ID (from cache): {execution_id}");

        println!(
            "[cached_results] Workflow complete (rows={}, metadata_present=true)",
            rows.len()
        );
    }
}
