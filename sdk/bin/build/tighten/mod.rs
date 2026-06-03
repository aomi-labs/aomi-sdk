//! `aomi-build tighten-spec <platform>` — turn `additionalProperties: true`
//! response bodies into concrete schemas inferred from real captured JSON
//! samples.
//!
//! ## Workflow
//!
//! 1. Capture real responses from the live API (curl + redirect to file). Drop
//!    them under `ext/specs/<platform>.samples/<operationId>.<status>.json`,
//!    e.g. `ext/specs/khalani.samples/getQuote.200.json`.
//! 2. Run `aomi-build tighten-spec khalani` to preview the diff.
//! 3. Run `aomi-build tighten-spec khalani --in-place` to write the tightened
//!    schemas back into `ext/specs/khalani.yaml`.
//! 4. `aomi-build gen-client khalani --force` to regenerate the typed Rust
//!    client. The previously `Map<String, Value>` responses now become typed
//!    structs.
//!
//! Inference is intentionally minimal: it walks one sample per operation and
//! emits a strict schema (`type: object` with `properties`, `type: array` with
//! `items`, primitives). No nullability inference, no `oneOf` merging across
//! multiple samples — that's what schemathesis-driven re-tightening passes
//! are for. The output is a **starting point** the human reviews and edits.

use clap::Args;
use eyre::{Context, Result, bail};
use serde_json::Value;
use std::path::PathBuf;

use crate::specs::workspace_root;

#[derive(Args, Debug)]
pub struct TightenSpecArgs {
    /// Platform name. Spec is `ext/specs/<platform>.yaml`; samples are read
    /// from `ext/specs/<platform>.samples/`.
    pub platform: String,

    /// Override the spec path.
    #[arg(long)]
    pub spec: Option<PathBuf>,

    /// Override the samples directory.
    #[arg(long)]
    pub samples: Option<PathBuf>,

    /// Write tightened spec back to disk. Without this flag, prints the diff.
    #[arg(long)]
    pub in_place: bool,
}

pub fn run(args: TightenSpecArgs) -> Result<()> {
    let root = workspace_root()?;
    let spec_path = args.spec.clone().unwrap_or_else(|| {
        root.join("ext")
            .join("specs")
            .join(format!("{}.yaml", args.platform))
    });
    let samples_dir = args.samples.clone().unwrap_or_else(|| {
        root.join("ext")
            .join("specs")
            .join(format!("{}.samples", args.platform))
    });

    if !spec_path.exists() {
        bail!("spec not found at {}", spec_path.display());
    }
    if !samples_dir.exists() {
        bail!(
            "samples dir not found at {}.\n  Capture real responses there as <operationId>.<status>.json,\n  e.g. `{}/getQuote.200.json`",
            samples_dir.display(),
            samples_dir.display()
        );
    }

    println!("Spec:    {}", spec_path.display());
    println!("Samples: {}", samples_dir.display());

    let spec_text = std::fs::read_to_string(&spec_path)
        .with_context(|| format!("failed to read {}", spec_path.display()))?;
    let mut spec: serde_yaml::Value =
        serde_yaml::from_str(&spec_text).context("spec is not valid YAML")?;

    let samples = collect_samples(&samples_dir)?;
    if samples.is_empty() {
        bail!(
            "no samples found in {}.\n  Expected files like `getQuote.200.json` (or `.201.json` etc).",
            samples_dir.display()
        );
    }

    let mut tightened: Vec<String> = Vec::new();
    for sample in &samples {
        match tighten_one(&mut spec, sample) {
            Ok(true) => tightened.push(format!(
                "  ✓ {} {} → typed schema inferred from {}",
                sample.operation_id,
                sample.status,
                sample
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            )),
            Ok(false) => println!(
                "  - {} {}: no matching loose response in spec, skipped",
                sample.operation_id, sample.status
            ),
            Err(e) => println!("  ✗ {}: {e:#}", sample.operation_id),
        }
    }

    if tightened.is_empty() {
        println!("\nNo schemas tightened.");
        return Ok(());
    }
    for line in &tightened {
        println!("{line}");
    }

    let new_text = serde_yaml::to_string(&spec).context("failed to serialise tightened spec")?;

    if args.in_place {
        std::fs::write(&spec_path, &new_text)
            .with_context(|| format!("failed to write {}", spec_path.display()))?;
        println!("\n✓ wrote {}", spec_path.display());
        println!("\nNext: aomi-build gen-client {} --force", args.platform);
    } else {
        // Cheap diff: just count lines changed. Avoids pulling in a diff crate.
        let before_lines = spec_text.lines().count();
        let after_lines = new_text.lines().count();
        let delta = after_lines as isize - before_lines as isize;
        println!(
            "\nDry-run: spec would grow from {before_lines} → {after_lines} lines ({delta:+} lines).\n  Re-run with --in-place to write."
        );
    }
    Ok(())
}

#[derive(Debug)]
struct Sample {
    operation_id: String,
    status: String,
    path: PathBuf,
    body: Value,
}

fn collect_samples(dir: &std::path::Path) -> Result<Vec<Sample>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        // <operationId>.<status>
        let Some((op, status)) = stem.rsplit_once('.') else {
            println!(
                "  skip {} (filename should be <operationId>.<status>.json, e.g. getQuote.200.json)",
                path.display()
            );
            continue;
        };
        let body_text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let body: Value = serde_json::from_str(&body_text)
            .with_context(|| format!("{} is not valid JSON", path.display()))?;
        out.push(Sample {
            operation_id: op.to_string(),
            status: status.to_string(),
            path,
            body,
        });
    }
    Ok(out)
}

/// For the matching operation in `spec`, replace its `additionalProperties:
/// true` JSON response schema with one inferred from `sample`. Returns true
/// iff a schema was replaced.
fn tighten_one(spec: &mut serde_yaml::Value, sample: &Sample) -> Result<bool> {
    let inferred = infer_schema(&sample.body);
    let inferred_yaml: serde_yaml::Value =
        serde_yaml::to_value(&inferred).context("inferred schema is not serialisable as YAML")?;

    let Some(paths) = spec
        .get_mut("paths")
        .and_then(serde_yaml::Value::as_mapping_mut)
    else {
        bail!("spec has no `paths` mapping");
    };

    for (_path, item) in paths.iter_mut() {
        let Some(item_map) = item.as_mapping_mut() else {
            continue;
        };
        for method in [
            "get", "put", "post", "delete", "patch", "options", "head", "trace",
        ] {
            let Some(op) = item_map.get_mut(serde_yaml::Value::String(method.into())) else {
                continue;
            };
            let op_id = op
                .get("operationId")
                .and_then(serde_yaml::Value::as_str)
                .unwrap_or("");
            if op_id != sample.operation_id {
                continue;
            }
            let Some(responses) = op
                .get_mut("responses")
                .and_then(serde_yaml::Value::as_mapping_mut)
            else {
                continue;
            };
            // YAML keys for status codes can be strings ('200') or quoted —
            // try both representations.
            let mut status_keys: Vec<serde_yaml::Value> =
                vec![serde_yaml::Value::String(sample.status.clone())];
            if let Ok(n) = sample.status.parse::<u64>() {
                status_keys.push(serde_yaml::Value::Number(n.into()));
            }
            // find_map can't move a mutable borrow out of a closure called
            // repeatedly, so we walk the keys imperatively instead.
            let mut response: Option<&mut serde_yaml::Value> = None;
            for k in &status_keys {
                if responses.contains_key(k) {
                    response = responses.get_mut(k);
                    break;
                }
            }
            let Some(response) = response else {
                continue;
            };
            let Some(content) = response.get_mut("content") else {
                continue;
            };
            let Some(content_map) = content.as_mapping_mut() else {
                continue;
            };
            let json_key = serde_yaml::Value::String("application/json".to_string());
            let Some(media) = content_map.get_mut(&json_key) else {
                continue;
            };
            let Some(media_map) = media.as_mapping_mut() else {
                continue;
            };

            // Only tighten if currently loose: schema is the literal
            // `{ type: object, additionalProperties: true }` shape.
            let schema_key = serde_yaml::Value::String("schema".into());
            let is_loose = match media_map.get(&schema_key) {
                Some(s) => is_loose_schema(s),
                None => true,
            };
            if !is_loose {
                println!(
                    "  - {} {}: response already has a tight schema, skipped",
                    sample.operation_id, sample.status
                );
                return Ok(false);
            }
            media_map.insert(schema_key, inferred_yaml.clone());
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_loose_schema(schema: &serde_yaml::Value) -> bool {
    let Some(map) = schema.as_mapping() else {
        return false;
    };
    let type_key = serde_yaml::Value::String("type".into());
    let addl_key = serde_yaml::Value::String("additionalProperties".into());
    let props_key = serde_yaml::Value::String("properties".into());
    let typ = map.get(&type_key).and_then(serde_yaml::Value::as_str);
    let addl = map.get(&addl_key);
    let has_props = map.contains_key(&props_key);
    let is_any_addl = matches!(addl, Some(serde_yaml::Value::Bool(true)));
    typ == Some("object") && is_any_addl && !has_props
}

// ============================================================================
// Schema inference (one sample → JSON Schema fragment)
// ============================================================================
//
// Intentionally minimal: emits strict types from a single sample, no merging
// across samples, no nullability across runs. Treats whole-number `f64`s as
// `integer`, otherwise `number`. Arrays sniff the first element only and add
// an `additionalProperties: true` if empty (so the spec doesn't lie).

#[derive(serde::Serialize)]
#[serde(untagged)]
enum InferredSchema {
    Object {
        #[serde(rename = "type")]
        ty: &'static str,
        properties: indexmap::IndexMap<String, InferredSchema>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        required: Vec<String>,
    },
    Array {
        #[serde(rename = "type")]
        ty: &'static str,
        items: Box<InferredSchema>,
    },
    Primitive {
        #[serde(rename = "type")]
        ty: &'static str,
    },
    Loose {
        #[serde(rename = "type")]
        ty: &'static str,
        #[serde(rename = "additionalProperties")]
        additional_properties: bool,
    },
    Empty {
        #[serde(rename = "type")]
        ty: &'static str,
    },
}

fn infer_schema(value: &Value) -> InferredSchema {
    match value {
        Value::Null => InferredSchema::Loose {
            ty: "object",
            additional_properties: true,
        },
        Value::Bool(_) => InferredSchema::Primitive { ty: "boolean" },
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                InferredSchema::Primitive { ty: "integer" }
            } else {
                InferredSchema::Primitive { ty: "number" }
            }
        }
        Value::String(_) => InferredSchema::Primitive { ty: "string" },
        Value::Array(items) => {
            let item_schema = items
                .first()
                .map(infer_schema)
                .unwrap_or(InferredSchema::Loose {
                    ty: "object",
                    additional_properties: true,
                });
            InferredSchema::Array {
                ty: "array",
                items: Box::new(item_schema),
            }
        }
        Value::Object(map) => {
            if map.is_empty() {
                return InferredSchema::Loose {
                    ty: "object",
                    additional_properties: true,
                };
            }
            let mut properties = indexmap::IndexMap::with_capacity(map.len());
            let mut required = Vec::with_capacity(map.len());
            for (k, v) in map {
                if !matches!(v, Value::Null) {
                    required.push(k.clone());
                }
                properties.insert(k.clone(), infer_schema(v));
            }
            // Empty struct guard (theoretical given the early return above).
            if properties.is_empty() {
                return InferredSchema::Empty { ty: "object" };
            }
            InferredSchema::Object {
                ty: "object",
                properties,
                required,
            }
        }
    }
}
