//! Declared result exports. The host owns retention, authorization and publication.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceSensitivity {
    Public,
    Private,
    /// Omit from retained/model-visible output. Never an executable export.
    Secret,
}

/// A whole value selected from the raw result with a fixed JSON Pointer.
/// A declared kind does not certify executable bytes or verification evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceExportDeclaration {
    pub name: String,
    pub pointer: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub sensitivity: ResourceSensitivity,
}

/// Registration metadata, independent of the full raw `ToolReturn` and routes.
/// The host validates declared exports against a trusted domain adapter before
/// publishing executable/evidence kinds; unsupported declarations are rejected
/// or retained as generic data. The plugin supplies no scope or URI authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceOutputDeclaration {
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_pointer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub sensitivity: ResourceSensitivity,
    #[serde(default)]
    pub outputs: Vec<ResourceExportDeclaration>,
}

impl ResourceOutputDeclaration {
    /// Validate registration shape only. This grants no publication authority.
    pub fn validate(&self) -> Result<(), String> {
        validate_label(&self.name)?;
        validate_kind(&self.kind)?;
        validate_schema(self.schema.as_deref())?;
        if let Some(pointer) = &self.summary_pointer {
            validate_pointer(pointer)?;
        }
        if self.outputs.len() > 32 {
            return Err("resource outputs exceed the registration limit of 32".into());
        }
        let mut names = BTreeSet::new();
        for output in &self.outputs {
            validate_label(&output.name)?;
            validate_kind(&output.kind)?;
            validate_pointer(&output.pointer)?;
            validate_schema(output.schema.as_deref())?;
            if !names.insert(&output.name) {
                return Err("duplicate resource output name".into());
            }
        }
        Ok(())
    }
}

fn validate_schema(value: Option<&str>) -> Result<(), String> {
    if value.is_some_and(|schema| schema.len() > 8192) {
        return Err("resource schema exceeds the registration limit of 8192 bytes".into());
    }
    Ok(())
}

fn validate_label(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
    {
        return Err("resource name must be a bounded identifier".into());
    }
    Ok(())
}

fn validate_kind(value: &str) -> Result<(), String> {
    let Some((name, version)) = value.rsplit_once('@') else {
        return Err("resource kind requires an explicit version".into());
    };
    if value.len() > 128
        || name.is_empty()
        || !name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-'))
        || version.parse::<u32>().is_err()
        || !version.bytes().all(|byte| byte.is_ascii_digit())
        || version.starts_with('0')
    {
        return Err("invalid versioned resource kind".into());
    }
    Ok(())
}

fn validate_pointer(value: &str) -> Result<(), String> {
    if value.len() > 1024
        || (!value.is_empty() && !value.starts_with('/'))
        || value.split('/').skip(1).count() > 16
    {
        return Err("resource JSON Pointer exceeds registration bounds".into());
    }
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'~' && !matches!(bytes.next(), Some(b'0' | b'1')) {
            return Err("invalid resource JSON Pointer escape".into());
        }
    }
    Ok(())
}
