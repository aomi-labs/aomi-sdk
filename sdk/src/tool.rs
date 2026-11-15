//! The `Tool` trait and the glue `package!` uses to describe and call tools.

use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::bindings::aomi::package::types::{Outcome as WitOutcome, ToolDef};
use crate::{Audience, Ctx, Outcome, Result, err};

/// One tool of a package. The host exposes it as `<package>_<NAME>`.
pub trait Tool {
    /// Short name, `[a-z0-9_]`.
    const NAME: &'static str;
    const DESCRIPTION: &'static str;
    /// `Model` tools are in the model's catalogue; `Direct` tools are called
    /// only by channel shortcuts and buttons.
    const AUDIENCE: Audience = Audience::Model;
    /// Set on a tool named by a `sign` outcome's `then` or a prepared
    /// transaction's `follow_up`; it reads [`Ctx::continuation`].
    const CONTINUATION: bool = false;
    /// The arguments; their JSON Schema is what `describe` reports.
    type Args: DeserializeOwned + JsonSchema;

    async fn run(args: Self::Args, ctx: &Ctx) -> Result<Outcome>;
}

impl ToolDef {
    /// The definition `describe` reports for `T`.
    pub fn of<T: Tool>() -> Self {
        Self {
            name: T::NAME.into(),
            description: T::DESCRIPTION.into(),
            schema: schemars::schema_for!(T::Args).as_value().to_string(),
            audience: T::AUDIENCE,
            continuation: T::CONTINUATION,
        }
    }
}

/// Deserializes `args`, runs `T` and maps the outcome to its WIT form.
pub async fn call<T: Tool>(args: &str, ctx: &Ctx) -> Result<WitOutcome> {
    let args = serde_json::from_str(args).map_err(|e| err("invalid_args", e.to_string()))?;
    T::run(args, ctx).await?.wit()
}
