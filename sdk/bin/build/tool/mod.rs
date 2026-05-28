use clap::Args;
use eyre::{Context, Result, bail};
use inquire::MultiSelect;
use std::path::PathBuf;

use crate::spec_load::{self, escape_keyword, pascal_case, snake_case};
use crate::specs::workspace_root;

mod render;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Generated client lives in apps/<platform>/src/client/.
    /// Tool layer imports from `crate::client`.
    AppLocal,
    /// Generated client lives in ext/src/<platform>/.
    /// Tool layer imports from `aomi_ext::<platform>`.
    Shared,
}

#[derive(Args, Debug)]
pub struct GenToolArgs {
    /// Platform name.
    pub platform: String,

    /// Override the spec path. Defaults to apps/<platform>/openapi.yaml
    /// (or ext/specs/<platform>.yaml when --shared is set).
    #[arg(long)]
    pub spec: Option<PathBuf>,

    /// Override the output directory. Defaults to apps/<platform>/.
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Skip the interactive picker and include every operation.
    #[arg(long)]
    pub all: bool,

    /// Overwrite existing files (Cargo.toml, src/lib.rs, src/tool.rs).
    #[arg(long)]
    pub force: bool,

    /// Treat as a shared library (client lives under ext/, tool layer imports
    /// from `aomi_ext::<platform>`). Default is app-local (client at
    /// `apps/<platform>/src/client/`, tool layer imports from `crate::client`).
    /// Auto-detected when omitted: presence of `apps/<platform>/src/client/`
    /// implies app-local; otherwise --shared is assumed.
    #[arg(long)]
    pub shared: bool,
}

pub fn run(args: GenToolArgs) -> Result<()> {
    let root = workspace_root()?;
    // Detect mode: explicit --shared OR auto-detect from filesystem.
    let app_client_dir = root
        .join("apps")
        .join(&args.platform)
        .join("src")
        .join("client");
    let app_local = !args.shared && app_client_dir.exists();
    let mode = if app_local {
        Mode::AppLocal
    } else {
        Mode::Shared
    };
    println!(
        "Mode: {}",
        match mode {
            Mode::AppLocal => "app-local (client in apps/<platform>/src/client/)",
            Mode::Shared => "shared (client in ext/src/<platform>/)",
        }
    );

    let spec_path = args.spec.clone().unwrap_or_else(|| match mode {
        Mode::AppLocal => root.join("apps").join(&args.platform).join("openapi.yaml"),
        Mode::Shared => root
            .join("ext")
            .join("specs")
            .join(format!("{}.yaml", args.platform)),
    });
    let out_dir = args
        .out
        .clone()
        .unwrap_or_else(|| root.join("apps").join(&args.platform));

    println!("Reading spec: {}", spec_path.display());
    let spec = spec_load::load_and_preprocess(&spec_path)?;

    let ops = collect_ops(&spec);
    if ops.is_empty() {
        bail!("spec has no operations");
    }

    let chosen: Vec<Op> = if args.all {
        ops
    } else {
        let labels: Vec<String> = ops
            .iter()
            .map(|o| {
                format!(
                    "{} {} → {}_{}",
                    o.method.to_uppercase(),
                    o.path,
                    args.platform,
                    o.operation_id
                )
            })
            .collect();
        let picked = MultiSelect::new("Select operations to expose as Aomi tools:", labels.clone())
            .with_page_size(20)
            .prompt()
            .context("operation picker failed or was cancelled")?;
        picked
            .iter()
            .map(|p| {
                let idx = labels
                    .iter()
                    .position(|l| l == p)
                    .expect("picked label exists");
                ops[idx].clone()
            })
            .collect()
    };

    if chosen.is_empty() {
        bail!("no operations selected");
    }

    let (chosen, skipped): (Vec<_>, Vec<_>) = chosen.into_iter().partition(|o| {
        !o.non_json_response
            && !o.has_request_body
            && !o
                .params
                .iter()
                .any(|p| matches!(p.kind, ParamKind::EnumString | ParamKind::Other))
    });
    for op in &skipped {
        let reason = if op.non_json_response {
            "non-JSON response"
        } else if op.has_request_body {
            "has typed request body (synthesize manually)"
        } else if op.params.iter().any(|p| p.kind == ParamKind::EnumString) {
            "has enum-typed params (progenitor generates Rust enums we can't synth from String)"
        } else {
            "has array/object-typed params (synthesize manually)"
        };
        println!(
            "  skipped {} {} ({})",
            op.method.to_uppercase(),
            op.path,
            reason
        );
    }
    if chosen.is_empty() {
        bail!("all selected operations were skipped");
    }

    let src_dir = out_dir.join("src");
    std::fs::create_dir_all(&src_dir)
        .with_context(|| format!("failed to create {}", src_dir.display()))?;

    let cargo_toml_path = out_dir.join("Cargo.toml");
    let lib_rs_path = src_dir.join("lib.rs");
    let tool_rs_path = src_dir.join("tool.rs");

    for p in [&cargo_toml_path, &lib_rs_path, &tool_rs_path] {
        if p.exists() && !args.force {
            bail!("{} already exists. Pass --force to overwrite.", p.display());
        }
    }

    let app_struct = format!("{}App", pascal_case(&args.platform));
    let preamble = render::preamble_default(&args.platform);

    std::fs::write(&cargo_toml_path, render::cargo_toml(&args.platform, mode))?;
    std::fs::write(
        &lib_rs_path,
        render::lib_rs(&args.platform, &app_struct, &chosen, &preamble, mode),
    )?;
    std::fs::write(
        &tool_rs_path,
        render::tool_rs(&args.platform, &app_struct, &chosen, mode),
    )?;

    println!("✓ wrote {}", cargo_toml_path.display());
    println!("✓ wrote {}", lib_rs_path.display());
    println!(
        "✓ wrote {} ({} tools)",
        tool_rs_path.display(),
        chosen.len()
    );

    if ensure_workspace_excludes(&root, &args.platform)? {
        println!("✓ added `apps/{}` to workspace exclude list", args.platform);
    }

    println!();
    println!("Verify with:");
    println!("  cd apps/{} && cargo build", args.platform);
    Ok(())
}

/// Ensure the workspace root Cargo.toml lists `apps/<platform>` under
/// `[workspace] exclude = [...]`. The existing convention in this repo is to
/// exclude every app crate (each is a cdylib that compiles independently).
/// Returns true iff the file was modified.
fn ensure_workspace_excludes(root: &std::path::Path, platform: &str) -> Result<bool> {
    let cargo_path = root.join("Cargo.toml");
    let cargo = std::fs::read_to_string(&cargo_path)?;
    let needle = format!(r#""apps/{platform}""#);
    if cargo.contains(&needle) {
        return Ok(false);
    }
    // Insert before the closing `]` of the exclude block. Best-effort string
    // manipulation: find `exclude = [` and the next `]`.
    if let Some(start) = cargo.find("exclude = [") {
        if let Some(end_offset) = cargo[start..].find(']') {
            let end = start + end_offset;
            let prefix = &cargo[..end];
            let suffix = &cargo[end..];
            let new_cargo = format!("{prefix}    {needle},\n{suffix}");
            std::fs::write(&cargo_path, new_cargo)?;
            return Ok(true);
        }
    }
    // No exclude block — silently skip rather than fail the gen.
    Ok(false)
}

#[derive(Debug, Clone)]
pub(crate) struct Op {
    pub operation_id: String,
    pub method: &'static str,
    pub path: String,
    pub server_url: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub params: Vec<Param>,
    pub has_request_body: bool,
    pub tool_marker: String,
    /// True when the success response is NOT JSON (e.g. text/csv, octet-stream).
    /// Such ops return ByteStream from progenitor — we skip JSON-style codegen.
    pub non_json_response: bool,
    /// Best-effort summary of the success-response Rust type, for the
    /// "typed projection" TODO comment in the rendered tool body.
    pub response_summary: ResponseSummary,
}

#[derive(Debug, Clone)]
pub(crate) enum ResponseSummary {
    /// Response is a typed Rust struct/enum (e.g. `SearchTokensResponse`,
    /// `Vec<Chain>`). Project before forwarding to the LLM.
    Typed { rust_type: String },
    /// Response is `Map<String, Value>` (spec marked `additionalProperties: true`).
    /// Tightening the spec gives the curator typed access.
    Loose,
    /// Response is bytes (e.g. text/csv, application/octet-stream) or unknown.
    Bytes,
}

#[derive(Debug, Clone)]
pub(crate) struct Param {
    pub name: String,
    pub snake_name: String,
    pub location: ParamLoc,
    pub required: bool,
    pub kind: ParamKind,
    pub description: Option<String>,
    pub is_auth: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParamLoc {
    Path,
    Query,
    Header,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParamKind {
    String,
    Int32,
    Int64,
    Number,
    Boolean,
    /// String param with an `enum:` constraint — progenitor generates a typed
    /// enum that we can't synthesise from a plain String at the tool layer.
    /// We mark these and let the caller decide what to do.
    EnumString,
    /// Array, object, or anything else we don't try to type — surface as `String`
    /// in the generated Args (caller passes JSON-encoded text).
    Other,
}

fn collect_ops(spec: &openapiv3::OpenAPI) -> Vec<Op> {
    use openapiv3::ReferenceOr;
    let global_server = spec_load::default_server_url(spec);
    let mut out = Vec::new();
    for (path, item_ref) in spec.paths.paths.iter() {
        let ReferenceOr::Item(item) = item_ref else {
            continue;
        };
        let path_server = item
            .servers
            .first()
            .map(|s| s.url.clone())
            .unwrap_or_else(|| global_server.clone());
        for (method, op_opt) in [
            ("get", &item.get),
            ("put", &item.put),
            ("post", &item.post),
            ("delete", &item.delete),
            ("patch", &item.patch),
            ("head", &item.head),
            ("options", &item.options),
            ("trace", &item.trace),
        ] {
            let Some(op) = op_opt else { continue };
            let server_url = op
                .servers
                .first()
                .map(|s| s.url.clone())
                .unwrap_or_else(|| path_server.clone());
            let operation_id = op
                .operation_id
                .clone()
                .unwrap_or_else(|| format!("{method}_{}", snake_case(path)));
            let tool_marker = pascal_case(&operation_id);
            let mut params = Vec::new();
            for p_ref in &op.parameters {
                let ReferenceOr::Item(p) = p_ref else {
                    continue;
                };
                params.push(map_param(p));
            }
            // progenitor's positional ordering: path params in path-declaration order,
            // then everything else alphabetically by snake_case name.
            let path_order: Vec<&str> = path_param_order(path);
            let path_rank = |name: &str| -> usize {
                path_order
                    .iter()
                    .position(|p| *p == name)
                    .unwrap_or(usize::MAX)
            };
            params.sort_by(|a, b| match (a.location, b.location) {
                (ParamLoc::Path, ParamLoc::Path) => path_rank(&a.name).cmp(&path_rank(&b.name)),
                (ParamLoc::Path, _) => std::cmp::Ordering::Less,
                (_, ParamLoc::Path) => std::cmp::Ordering::Greater,
                _ => a.snake_name.cmp(&b.snake_name),
            });
            let non_json_response = first_success_content_type(op)
                .map(|ct| !ct.starts_with("application/json"))
                .unwrap_or(false);
            let response_summary = if non_json_response {
                ResponseSummary::Bytes
            } else {
                derive_response_summary(op, &operation_id)
            };
            out.push(Op {
                operation_id,
                method,
                path: path.clone(),
                server_url,
                summary: op.summary.clone(),
                description: op.description.clone(),
                params,
                has_request_body: op.request_body.is_some(),
                tool_marker,
                non_json_response,
                response_summary,
            });
        }
    }
    out
}

fn map_param(p: &openapiv3::Parameter) -> Param {
    use openapiv3::Parameter;
    let (data, location) = match p {
        Parameter::Path { parameter_data, .. } => (parameter_data, ParamLoc::Path),
        Parameter::Query { parameter_data, .. } => (parameter_data, ParamLoc::Query),
        Parameter::Header { parameter_data, .. } => (parameter_data, ParamLoc::Header),
        Parameter::Cookie { parameter_data, .. } => (parameter_data, ParamLoc::Header),
    };
    let kind = schema_kind(&data.format);
    let snake_name = escape_keyword(&snake_case(&data.name));
    let is_auth = header_looks_like_auth(&data.name);
    Param {
        name: data.name.clone(),
        snake_name,
        location,
        required: data.required,
        kind,
        description: data.description.clone(),
        is_auth,
    }
}

fn schema_kind(format: &openapiv3::ParameterSchemaOrContent) -> ParamKind {
    use openapiv3::{
        IntegerFormat, ParameterSchemaOrContent, ReferenceOr, SchemaKind, Type,
        VariantOrUnknownOrEmpty,
    };
    let ParameterSchemaOrContent::Schema(ReferenceOr::Item(schema)) = format else {
        return ParamKind::Other;
    };
    match &schema.schema_kind {
        SchemaKind::Type(Type::String(s)) => {
            if !s.enumeration.is_empty() {
                ParamKind::EnumString
            } else {
                ParamKind::String
            }
        }
        SchemaKind::Type(Type::Integer(i)) => match &i.format {
            VariantOrUnknownOrEmpty::Item(IntegerFormat::Int32) => ParamKind::Int32,
            _ => ParamKind::Int64,
        },
        SchemaKind::Type(Type::Number(_)) => ParamKind::Number,
        SchemaKind::Type(Type::Boolean(_)) => ParamKind::Boolean,
        _ => ParamKind::Other,
    }
}

/// Extract the path-parameter names from a path template like
/// `/v1/{owner}/dashboards/{dashboard_id}` → `["owner", "dashboard_id"]`.
fn path_param_order(path: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'}' {
                j += 1;
            }
            if j < bytes.len() {
                if let Ok(name) = std::str::from_utf8(&bytes[start..j]) {
                    out.push(name);
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Inspect the operation's first 2xx JSON response and derive a best-effort
/// Rust type name matching what progenitor would emit. Used purely as a
/// human-readable comment in the generated tool body — never as a real type.
fn derive_response_summary(op: &openapiv3::Operation, operation_id: &str) -> ResponseSummary {
    use openapiv3::ReferenceOr;
    for (code, resp_ref) in &op.responses.responses {
        let is_2xx = matches!(code, openapiv3::StatusCode::Code(c) if (200..300).contains(c))
            || matches!(code, openapiv3::StatusCode::Range(2));
        if !is_2xx {
            continue;
        }
        let ReferenceOr::Item(resp) = resp_ref else {
            continue;
        };
        let Some(media) = resp
            .content
            .iter()
            .find(|(ct, _)| ct.starts_with("application/json"))
            .map(|(_, m)| m)
        else {
            continue;
        };
        let Some(schema_ref) = &media.schema else {
            continue;
        };
        return summarise_schema(schema_ref, operation_id);
    }
    ResponseSummary::Bytes
}

fn summarise_schema(
    schema_ref: &openapiv3::ReferenceOr<openapiv3::Schema>,
    operation_id: &str,
) -> ResponseSummary {
    use openapiv3::{ReferenceOr, SchemaKind, Type};
    match schema_ref {
        ReferenceOr::Reference { reference } => {
            // "#/components/schemas/Foo" → "Foo"
            let name = reference.rsplit('/').next().unwrap_or(reference);
            ResponseSummary::Typed {
                rust_type: pascal_case(name),
            }
        }
        ReferenceOr::Item(schema) => match &schema.schema_kind {
            SchemaKind::Type(Type::Array(arr)) => {
                let inner = match &arr.items {
                    Some(boxed) => match boxed.clone().unbox() {
                        ReferenceOr::Reference { reference } => reference
                            .rsplit('/')
                            .next()
                            .map(pascal_case)
                            .unwrap_or_else(|| "Value".into()),
                        ReferenceOr::Item(_) => "Value".into(),
                    },
                    None => "Value".into(),
                };
                ResponseSummary::Typed {
                    rust_type: format!("Vec<{inner}>"),
                }
            }
            SchemaKind::Type(Type::Object(obj)) => {
                let is_loose = matches!(
                    obj.additional_properties,
                    Some(openapiv3::AdditionalProperties::Any(true))
                ) || (obj.properties.is_empty()
                    && matches!(
                        obj.additional_properties,
                        Some(openapiv3::AdditionalProperties::Schema(_))
                    ));
                if is_loose && obj.properties.is_empty() {
                    ResponseSummary::Loose
                } else {
                    // Inline object → progenitor synthesises `<OperationId>Response`.
                    ResponseSummary::Typed {
                        rust_type: format!("{}Response", pascal_case(operation_id)),
                    }
                }
            }
            SchemaKind::Type(Type::String(_)) => ResponseSummary::Typed {
                rust_type: "String".into(),
            },
            SchemaKind::Type(Type::Number(_)) => ResponseSummary::Typed {
                rust_type: "f64".into(),
            },
            SchemaKind::Type(Type::Integer(_)) => ResponseSummary::Typed {
                rust_type: "i64".into(),
            },
            SchemaKind::Type(Type::Boolean(_)) => ResponseSummary::Typed {
                rust_type: "bool".into(),
            },
            SchemaKind::OneOf { .. } | SchemaKind::AnyOf { .. } | SchemaKind::AllOf { .. } => {
                // Progenitor emits a synthesised enum/struct named after the op.
                ResponseSummary::Typed {
                    rust_type: format!("{}Response", pascal_case(operation_id)),
                }
            }
            _ => ResponseSummary::Loose,
        },
    }
}

fn first_success_content_type(op: &openapiv3::Operation) -> Option<String> {
    use openapiv3::ReferenceOr;
    for (code, resp_ref) in &op.responses.responses {
        let is_2xx = matches!(code, openapiv3::StatusCode::Code(c) if (200..300).contains(c))
            || matches!(code, openapiv3::StatusCode::Range(2));
        if !is_2xx {
            continue;
        }
        let ReferenceOr::Item(resp) = resp_ref else {
            continue;
        };
        if let Some((ct, _)) = resp.content.iter().next() {
            return Some(ct.clone());
        }
    }
    None
}

fn header_looks_like_auth(name: &str) -> bool {
    let n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    n.contains("apikey") || n == "authorization" || n.ends_with("apitoken") || n.ends_with("token")
}
