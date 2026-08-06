//! The individual spec patches applied by `spec_load::load_and_preprocess`.
//!
//! Real-world specs frequently violate progenitor's strict expectations. Each
//! patch here rewrites one class of violation and returns how many spots it
//! touched so the caller can print a summary.

use crate::spec_load::{is_success, snake_case};

/// Visit every concrete path item with its path key.
fn for_each_item(
    spec: &mut openapiv3::OpenAPI,
    mut f: impl FnMut(&str, &mut openapiv3::PathItem),
) {
    for (path, item_ref) in spec.paths.paths.iter_mut() {
        if let openapiv3::ReferenceOr::Item(item) = item_ref {
            f(path, item);
        }
    }
}

/// A path item's eight method slots, with their method names.
fn op_slots(
    item: &mut openapiv3::PathItem,
) -> [(&'static str, &mut Option<openapiv3::Operation>); 8] {
    [
        ("get", &mut item.get),
        ("put", &mut item.put),
        ("post", &mut item.post),
        ("delete", &mut item.delete),
        ("patch", &mut item.patch),
        ("head", &mut item.head),
        ("options", &mut item.options),
        ("trace", &mut item.trace),
    ]
}

/// Visit every operation in the spec.
fn for_each_op(spec: &mut openapiv3::OpenAPI, mut f: impl FnMut(&mut openapiv3::Operation)) {
    for_each_item(spec, |_, item| {
        for (_, slot) in op_slots(item) {
            if let Some(op) = slot {
                f(op);
            }
        }
    });
}

/// Build a schema from a JSON literal (permissive stubs and retypes).
fn schema_from_json(value: serde_json::Value) -> openapiv3::Schema {
    serde_json::from_value(value).expect("schema literal must parse")
}

/// progenitor allows exactly one media type per request/response body. Keep
/// `application/json` if present, else the first remaining type, and drop the
/// rest. True iff the map was modified.
fn collapse_content(content: &mut indexmap::IndexMap<String, openapiv3::MediaType>) -> bool {
    if content.len() <= 1 {
        return false;
    }
    let keep = if content.contains_key("application/json") {
        Some("application/json".to_string())
    } else {
        content.keys().next().cloned()
    };
    let Some(keep) = keep else {
        return false;
    };
    let to_drop: Vec<String> = content.keys().filter(|k| **k != keep).cloned().collect();
    for k in &to_drop {
        content.shift_remove(k);
    }
    true
}

pub(crate) fn collapse_response_content_to_json(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::ReferenceOr;
    let mut collapsed = 0;
    for_each_op(spec, |op| {
        for resp in op.responses.responses.values_mut() {
            if let ReferenceOr::Item(r) = resp
                && collapse_content(&mut r.content)
            {
                collapsed += 1;
            }
        }
        if let Some(ReferenceOr::Item(r)) = op.responses.default.as_mut()
            && collapse_content(&mut r.content)
        {
            collapsed += 1;
        }
    });
    collapsed
}

pub(crate) fn collapse_request_body_to_json(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::ReferenceOr;
    let mut collapsed = 0;
    for_each_op(spec, |op| {
        if let Some(ReferenceOr::Item(rb)) = &mut op.request_body
            && collapse_content(&mut rb.content)
        {
            collapsed += 1;
        }
    });
    collapsed
}

/// Walk the entire spec for `$ref: '#/components/schemas/<Name>'` references
/// and create a stub `additionalProperties: true` schema for any that aren't
/// defined. Lets progenitor's typify pass succeed even when the upstream spec
/// has a broken cross-reference (Limitless ships a few of these).
pub(crate) fn stub_missing_schema_refs(spec: &mut openapiv3::OpenAPI) -> usize {
    // Serialize → walk JSON for refs → diff vs defined → inject stubs into
    // components.schemas. Cheap and avoids hand-walking openapiv3's enum tree.
    let json = match serde_json::to_string(&spec) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let prefix = "\"#/components/schemas/";
    let mut referenced: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut idx = 0;
    while let Some(start) = json[idx..].find(prefix) {
        let from = idx + start + prefix.len();
        if let Some(end_offset) = json[from..].find('"') {
            referenced.insert(json[from..from + end_offset].to_string());
            idx = from + end_offset;
        } else {
            break;
        }
    }
    let components = spec.components.get_or_insert_with(Default::default);
    let mut stubbed = 0;
    for name in referenced {
        if components.schemas.contains_key(&name) {
            continue;
        }
        // Build a permissive stub: type=object, additionalProperties=true.
        let stub = schema_from_json(serde_json::json!({
            "type": "object",
            "additionalProperties": true
        }));
        components
            .schemas
            .insert(name, openapiv3::ReferenceOr::Item(stub));
        stubbed += 1;
    }
    stubbed
}

/// For every `{name}` placeholder in a path that doesn't have a corresponding
/// `parameters` entry on the operation (or path item), inject a default
/// `string` path parameter. Specs occasionally omit these declarations and
/// progenitor refuses to generate the operation otherwise.
pub(crate) fn inject_missing_path_params(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::{Parameter, ParameterData, ParameterSchemaOrContent, ReferenceOr};
    let mut injected = 0;
    for_each_item(spec, |path, item| {
        let placeholders = path_placeholders(path);
        if placeholders.is_empty() {
            return;
        }
        // Path-level params apply to all ops; collect their names.
        let path_lvl_names: std::collections::HashSet<String> = item
            .parameters
            .iter()
            .filter_map(|p| match p {
                ReferenceOr::Item(Parameter::Path { parameter_data, .. }) => {
                    Some(parameter_data.name.clone())
                }
                _ => None,
            })
            .collect();
        for (_, slot) in op_slots(item) {
            let Some(op) = slot else { continue };
            let mut have: std::collections::HashSet<String> = path_lvl_names.clone();
            for p in &op.parameters {
                if let ReferenceOr::Item(Parameter::Path { parameter_data, .. }) = p {
                    have.insert(parameter_data.name.clone());
                }
            }
            for name in &placeholders {
                if have.contains(name) {
                    continue;
                }
                let param = Parameter::Path {
                    parameter_data: ParameterData {
                        name: name.clone(),
                        description: None,
                        required: true,
                        deprecated: None,
                        format: ParameterSchemaOrContent::Schema(ReferenceOr::Item(
                            schema_from_json(serde_json::json!({ "type": "string" })),
                        )),
                        example: None,
                        examples: Default::default(),
                        explode: None,
                        extensions: Default::default(),
                    },
                    style: openapiv3::PathStyle::Simple,
                };
                op.parameters.push(ReferenceOr::Item(param));
                injected += 1;
            }
        }
    });
    injected
}

/// `{name}` placeholder names in a path template, in declaration order.
fn path_placeholders(path: &str) -> Vec<String> {
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
            if let Ok(name) = std::str::from_utf8(&bytes[start..j]) {
                out.push(name.to_string());
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// Path params with `schema: { example: "..." }` but NO `type:` make
/// progenitor fall back to `serde_json::Value`, which then double-quotes
/// strings on the wire. Default these to `type: string` (path params are
/// nearly always strings).
pub(crate) fn retype_path_params_as_string(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::{Parameter, ParameterSchemaOrContent, ReferenceOr};
    let mut fixed = 0;
    for_each_op(spec, |op| {
        for p in &mut op.parameters {
            let ReferenceOr::Item(Parameter::Path { parameter_data, .. }) = p else {
                continue;
            };
            let needs_retype = match &parameter_data.format {
                ParameterSchemaOrContent::Schema(ReferenceOr::Item(schema)) => {
                    matches!(&schema.schema_kind, openapiv3::SchemaKind::Any(_))
                }
                _ => false,
            };
            if needs_retype {
                parameter_data.format = ParameterSchemaOrContent::Schema(ReferenceOr::Item(
                    schema_from_json(serde_json::json!({ "type": "string" })),
                ));
                fixed += 1;
            }
        }
    });
    fixed
}

/// Pagination params commonly typed as `number` (== f64) in specs but the
/// server expects integers. Detect by name + retype to `integer`.
pub(crate) fn retype_pagination_as_integer(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::{Parameter, ParameterSchemaOrContent, ReferenceOr, SchemaKind, Type};
    const PAGINATION_NAMES: &[&str] = &[
        "limit", "page", "offset", "count", "size", "per_page", "perpage", "pagesize",
    ];
    let mut fixed = 0;
    for_each_op(spec, |op| {
        for p in &mut op.parameters {
            let ReferenceOr::Item(param) = p else {
                continue;
            };
            let parameter_data = match param {
                Parameter::Query { parameter_data, .. }
                | Parameter::Header { parameter_data, .. }
                | Parameter::Path { parameter_data, .. }
                | Parameter::Cookie { parameter_data, .. } => parameter_data,
            };
            let lname = parameter_data.name.to_ascii_lowercase();
            if !PAGINATION_NAMES.contains(&lname.as_str()) {
                continue;
            }
            let ParameterSchemaOrContent::Schema(ReferenceOr::Item(schema)) =
                &mut parameter_data.format
            else {
                continue;
            };
            if matches!(schema.schema_kind, SchemaKind::Type(Type::Number(_))) {
                *schema = schema_from_json(serde_json::json!({
                    "type": "integer",
                    "format": "int64"
                }));
                fixed += 1;
            }
        }
    });
    fixed
}

/// progenitor asserts `parameter_data.required` on every path param. Specs
/// occasionally violate this (it's an OpenAPI spec bug — path params MUST be
/// required per the spec), so we silently coerce.
pub(crate) fn force_path_params_required(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::{Parameter, ReferenceOr};
    let mut fixed = 0;
    for_each_op(spec, |op| {
        for p_ref in &mut op.parameters {
            let ReferenceOr::Item(Parameter::Path { parameter_data, .. }) = p_ref else {
                continue;
            };
            if !parameter_data.required {
                parameter_data.required = true;
                fixed += 1;
            }
        }
    });
    fixed
}

/// progenitor doesn't handle `multipart/form-data` request bodies. Drop those
/// operations entirely from the spec — they need hand-written wrappers.
pub(crate) fn drop_multipart_ops(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::ReferenceOr;
    let mut dropped = 0;
    for_each_item(spec, |_, item| {
        for (_, slot) in op_slots(item) {
            let has_multipart = slot.as_ref().is_some_and(|op| {
                matches!(&op.request_body, Some(ReferenceOr::Item(rb))
                    if rb.content.contains_key("multipart/form-data"))
            });
            if has_multipart {
                *slot = None;
                dropped += 1;
            }
        }
    });
    dropped
}

pub(crate) fn fill_missing_operation_ids(spec: &mut openapiv3::OpenAPI) -> usize {
    let mut count = 0;
    for_each_item(spec, |path, item| {
        for (method, slot) in op_slots(item) {
            if let Some(op) = slot
                && op.operation_id.is_none()
            {
                op.operation_id = Some(synthesize_op_id(method, path));
                count += 1;
            }
        }
    });
    count
}

fn synthesize_op_id(method: &str, path: &str) -> String {
    let mut s = String::from(method);
    let mut sep = true;
    for ch in path.chars() {
        match ch {
            '/' | '{' | '}' | '-' | '.' => {
                if !sep {
                    s.push('_');
                    sep = true;
                }
            }
            c if c.is_ascii_alphanumeric() => {
                s.push(c.to_ascii_lowercase());
                sep = false;
            }
            _ => {}
        }
    }
    while s.ends_with('_') {
        s.pop();
    }
    s
}

pub(crate) fn rename_wildcard_content_types(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::ReferenceOr;
    let mut count = 0;
    fn fix(c: &mut indexmap::IndexMap<String, openapiv3::MediaType>, n: &mut usize) {
        if let Some(media) = c.shift_remove("*/*") {
            c.entry("application/json".to_string()).or_insert(media);
            *n += 1;
        }
    }
    for_each_op(spec, |op| {
        if let Some(ReferenceOr::Item(req)) = &mut op.request_body {
            fix(&mut req.content, &mut count);
        }
        for resp_ref in op.responses.responses.values_mut() {
            if let ReferenceOr::Item(resp) = resp_ref {
                fix(&mut resp.content, &mut count);
            }
        }
        if let Some(ReferenceOr::Item(resp)) = op.responses.default.as_mut() {
            fix(&mut resp.content, &mut count);
        }
    });
    count
}

pub(crate) fn dedupe_success_responses(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::ReferenceOr;
    let mut dropped = 0;
    for_each_op(spec, |op| {
        // Pick which single response to keep:
        //   1. first 2xx with a body (real success type),
        //   2. else first 2xx (empty body — generated client returns ()),
        //   3. else nothing — every other response is dropped.
        //
        // progenitor's assertion is on response_type uniqueness, and a
        // response with no body still counts as `()` — distinct from a
        // typed body. Mixing empty 200 + bodied 202 panics. Keeping only
        // ONE response avoids every variant of that bug.
        let keep: Option<openapiv3::StatusCode> = {
            let bodied_2xx = op.responses.responses.iter().find_map(|(c, r)| {
                if !is_success(c) {
                    return None;
                }
                matches!(r, ReferenceOr::Item(rr) if !rr.content.is_empty()).then(|| c.clone())
            });
            bodied_2xx.or_else(|| {
                op.responses
                    .responses
                    .iter()
                    .find_map(|(c, _)| is_success(c).then(|| c.clone()))
            })
        };
        let to_remove: Vec<openapiv3::StatusCode> = op
            .responses
            .responses
            .keys()
            .filter(|c| Some(*c) != keep.as_ref())
            .cloned()
            .collect();
        for code in &to_remove {
            op.responses.responses.shift_remove(code);
            dropped += 1;
        }
        // Always drop the `default` response. progenitor counts it as a
        // response_type even when empty, which trips its `<= 1` assertion
        // whenever an op has both a 200 and a `default` (very common).
        if op.responses.default.is_some() {
            op.responses.default = None;
            dropped += 1;
        }
    });
    dropped
}

pub(crate) fn drop_param_name_collisions(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::{Parameter, ReferenceOr};
    let mut dropped = 0;
    for_each_op(spec, |op| {
        let path_keys: std::collections::HashSet<String> = op
            .parameters
            .iter()
            .filter_map(|p| match p {
                ReferenceOr::Item(Parameter::Path { parameter_data, .. }) => {
                    Some(snake_case(&parameter_data.name))
                }
                _ => None,
            })
            .collect();
        let before = op.parameters.len();
        op.parameters.retain(|p| match p {
            ReferenceOr::Item(Parameter::Query { parameter_data, .. })
            | ReferenceOr::Item(Parameter::Header { parameter_data, .. })
            | ReferenceOr::Item(Parameter::Cookie { parameter_data, .. }) => {
                !path_keys.contains(&snake_case(&parameter_data.name))
            }
            _ => true,
        });
        dropped += before - op.parameters.len();
    });
    dropped
}
