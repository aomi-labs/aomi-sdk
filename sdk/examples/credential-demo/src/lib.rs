//! Harmless example app for exercising user-owned application credentials.
//!
//! Run `mock_server.py`, load this plugin, save the declared values for a
//! user, and call `credential_demo_validate`. The validation tool only talks
//! to a local mock and never returns credential values. The separate redaction
//! probe deliberately returns the harmless demo token for host regression
//! testing.

use aomi_sdk::{
    DynAomiTool, DynToolCallCtx, Secret, dyn_aomi_app, resolve_user_secret_value,
    schemars::JsonSchema,
    serde_json::{Value, json},
};
use serde::Deserialize;
use std::time::Duration;

const REQUIRED_TOKEN: Secret = Secret::new(
    "DEMO_API_TOKEN",
    "Harmless credential-demo token accepted by the local mock service.",
    true,
)
.user_owned();

const OPTIONAL_TAG: Secret = Secret::new(
    "DEMO_ACCOUNT_TAG",
    "Optional harmless account tag used to exercise an optional user credential.",
    false,
)
.user_owned();

const MOCK_URL: &str = "http://127.0.0.1:18080/validate";

#[derive(Clone, Default)]
struct CredentialDemoApp;

#[derive(Debug, Deserialize, JsonSchema)]
struct EmptyArgs {}

#[derive(Debug, Deserialize)]
struct MockResponse {
    validated: bool,
    credential_profile: String,
    optional_credential_present: bool,
}

struct ValidateCredential;

impl DynAomiTool for ValidateCredential {
    type App = CredentialDemoApp;
    type Args = EmptyArgs;

    const NAME: &'static str = "credential_demo_validate";
    const DESCRIPTION: &'static str = "Validate the current user's credential against the local harmless mock service. Returns only validation status and whether the optional credential was supplied.";

    fn run(_app: &Self::App, _args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let token = resolve_user_secret_value(
            &ctx,
            "DEMO_API_TOKEN",
            "credential-demo requires DEMO_API_TOKEN",
        )?;
        let account_tag = ctx
            .secrets
            .get("DEMO_ACCOUNT_TAG")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .map_err(|_| "credential-demo client setup failed".to_string())?;
        let mut request = client.post(MOCK_URL).bearer_auth(token);
        if let Some(account_tag) = account_tag {
            request = request.header("x-demo-account-tag", account_tag);
        }
        let response = request
            .send()
            .map_err(|_| "credential-demo mock request failed".to_string())?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!(
                "credential-demo mock rejected the credential with status {status}"
            ));
        }
        let response: MockResponse = response
            .json()
            .map_err(|_| "credential-demo mock returned an invalid response".to_string())?;
        if !response.validated {
            return Err("credential-demo mock did not validate the credential".to_string());
        }
        let credential_profile = match response.credential_profile.as_str() {
            "demo-account-a" | "demo-account-b" | "demo-account-a-rotated" => {
                response.credential_profile
            }
            _ => return Err("credential-demo mock returned an unknown profile".to_string()),
        };

        Ok(json!({
            "validated": true,
            "credential_profile": credential_profile,
            "optional_credential_present": response.optional_credential_present,
            "service": "local-credential-mock"
        }))
    }
}

struct RedactionProbe;

impl DynAomiTool for RedactionProbe {
    type App = CredentialDemoApp;
    type Args = EmptyArgs;

    const NAME: &'static str = "credential_demo_redaction_probe";
    const DESCRIPTION: &'static str = "Regression probe that deliberately returns the harmless required demo credential in a nested value and object key. Use only to verify host redaction before model delivery.";

    fn run(_app: &Self::App, _args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let token = resolve_user_secret_value(
            &ctx,
            "DEMO_API_TOKEN",
            "credential-demo requires DEMO_API_TOKEN",
        )?;
        let mut keyed_by_credential = serde_json::Map::new();
        keyed_by_credential.insert(token.clone(), json!("credential-key-marker"));

        Ok(json!({
            "probe": "host-credential-redaction",
            "nested": { "credential_value": token },
            "credential_as_object_key": keyed_by_credential,
            "expected_visible_value": "[REDACTED]"
        }))
    }
}

dyn_aomi_app!(
    app = CredentialDemoApp,
    name = "credential-demo",
    version = "0.1.0",
    preamble = "Use credential_demo_validate to verify the current user's harmless demo credential. credential_demo_redaction_probe is only for testing host redaction. Never ask the user to reveal credential values in chat.",
    tools = [ValidateCredential, RedactionProbe],
    secrets = [REQUIRED_TOKEN, OPTIONAL_TAG],
    namespaces = [],
);

#[cfg(test)]
mod tests {
    use super::*;
    use aomi_sdk::DynAomiApp;

    #[test]
    fn manifest_declares_required_and_optional_user_credentials() {
        let manifest = CredentialDemoApp.manifest();
        let secrets = manifest.secrets.unwrap();

        assert_eq!(secrets.len(), 2);
        assert!(secrets[0].required);
        assert!(secrets[0].user_own);
        assert!(!secrets[1].required);
        assert!(secrets[1].user_own);
        assert!(
            manifest
                .tools
                .iter()
                .any(|tool| tool.name == "credential_demo_redaction_probe")
        );
    }
}
