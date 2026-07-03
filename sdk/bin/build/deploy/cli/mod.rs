//! Hosted deployment CLI surface for `aomi-build`.
//!
//! These commands are thin backend relays: they read local git facts and post the
//! repo-scoped deploy/activate requests defined in `deploy/types.rs`. They
//! never handle a GitHub token, never clone or push a platform repo, and never
//! generate release tags or manifests — the backend owns all of that.
//!
//! Each command lives in its own file; cross-command helpers (env/credential
//! resolution, git facts) live in `shared`.
//!
//! ```text
//! deploy                     # lifecycle: preflight → run → activate → status
//!   --platform <NAME>        # aomi.toml [app].platform (default community)
//!   --repo <OWNER/REPO>      # sync source when app_source_id is not known
//!   --app-source-id <ID>     # connected GitHub App install (AOMI_APP_SOURCE_ID)
//!   --branch <NAME>          # deprecated; checkout the branch locally instead
//!   --commit <SHA>           # deploy this source commit (default: HEAD)
//!   --aomi-toml <PATH>       # repeatable; default: all tracked aomi.toml
//!   --backend <URL>          # AOMI_BACKEND_URL
//!   --preflight              # resolve + print the plan without opening a PR
//!   --json
//!
//! activate [APP]...          # activate release tags (default: deployment.json tags)
//!   --platform <NAME>        # default: deployment.json platform
//!   --release-tag <TAG>      # repeatable; overrides deployment.json tags
//!   --backend <URL>          # AOMI_BACKEND_URL
//!   --activation-token <T>   # AOMI_APP_ACTIVATION_TOKEN
//!   --target-tag <TAG>       # repeatable
//!   --dry-run
//!   --json
//!
//! status                     # local deployment.json + backend per-app state
//!   --backend <URL>
//!   --json
//!
//! request                    # legacy ops onboarding request (Discord)
//! ```

pub mod activate;
pub mod apps;
pub mod connect;
pub mod deploy;
pub mod request;
pub mod source;
pub mod status;
pub mod token;

mod shared;

pub use activate::ActivateArgs;
pub use apps::AppsArgs;
pub use connect::ConnectArgs;
pub use deploy::{DeployArgs, DeployStepArgs};
pub use request::RequestArgs;
pub use source::SourceArgs;
pub use status::StatusArgs;
pub use token::TokenArgs;
