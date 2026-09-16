//! Minimal app with one ordinary tool and one tool owned by an app skill.
//!
//! `ping` is always visible. `draft_greeting` is registered for execution but
//! its schema is exposed by the host only after `skill-owned/writing` activates.

use aomi_sdk::{DynAomiTool, DynToolCallCtx, dyn_aomi_app, schemars::JsonSchema, serde_json::json};
use serde::Deserialize;

#[derive(Clone, Default)]
struct SkillOwnedApp;

#[derive(Debug, Deserialize, JsonSchema)]
struct EmptyArgs {}

struct Ping;

impl DynAomiTool for Ping {
    type App = SkillOwnedApp;
    type Args = EmptyArgs;

    const NAME: &'static str = "ping";
    const DESCRIPTION: &'static str = "Check that the app is available.";

    fn run(
        _app: &Self::App,
        _args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<serde_json::Value, String> {
        Ok(json!({ "ok": true }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GreetingArgs {
    name: String,
}

struct DraftGreeting;

impl DynAomiTool for DraftGreeting {
    type App = SkillOwnedApp;
    type Args = GreetingArgs;

    const NAME: &'static str = "draft_greeting";
    const DESCRIPTION: &'static str = "Draft a concise greeting for a person.";

    fn run(
        _app: &Self::App,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<serde_json::Value, String> {
        Ok(json!({ "greeting": format!("Hello, {}!", args.name) }))
    }
}

dyn_aomi_app!(
    app = SkillOwnedApp,
    name = "skill-owned",
    version = "0.1.0",
    preamble = "A compact SDK example.",
    tools = [Ping],
    namespaces = [],
    skills = [{
        id: "skill-owned/writing",
        description: "Draft short, friendly greetings",
        tags: ["writing", "greeting"],
        tools: [DraftGreeting],
        sections: { instructions: "skill/writing.md" },
    }],
);

#[cfg(test)]
mod tests {
    use super::*;
    use aomi_sdk::DynAomiApp;

    #[test]
    fn manifest_links_the_typed_tool_to_its_skill() {
        let manifest = SkillOwnedApp.manifest();
        assert_eq!(manifest.tools.len(), 2);
        assert_eq!(manifest.skills[0].injected_tools, ["draft_greeting"]);
        aomi_sdk::validate_app_skills_with_tools(&manifest.name, &manifest.skills, &manifest.tools)
            .expect("valid owned tool");
    }
}
