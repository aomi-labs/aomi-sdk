//! The versioned host/plugin contract, independent of the SDK package release.

use eyre::{Result, bail};

/// C signatures, JSON envelopes, allocation ownership and execution lifecycle.
/// Bump only when an existing compiled plugin can no longer use the contract.
pub const AOMI_FFI_VERSION: u32 = 1;

/// Optional extensions understood by this FFI implementation. Runtime policy
/// may still refuse an operation; supporting an interface never grants access.
pub const AOMI_FFI_INTERFACES: &[&str] = &[
    "async_tools_v1",
    "routes_v1",
    "app_skills_v1",
    "secret_slots_v1",
    "evm_sponsored_4337_v1",
];

/// Migration table for SDKs predating the `aomi_ffi_version` export. Entries
/// must have a historical-binary regression test before being added here.
pub fn legacy_ffi_version(sdk_version: &str) -> Option<u32> {
    match sdk_version {
        "5.1.1" | "5.1.2" => Some(1),
        _ => None,
    }
}

/// Resolve a plugin's FFI requirement. An explicit declaration always wins:
/// a familiar SDK version cannot override an unsupported FFI declaration.
pub fn validate_ffi_version(ffi_version: Option<u32>, sdk_version: &str) -> Result<u32> {
    let version = ffi_version
        .or_else(|| legacy_ffi_version(sdk_version))
        .ok_or_else(|| {
            eyre::eyre!(
                "plugin SDK {sdk_version} has no verified ffi_version; rebuild with an SDK that exports aomi_ffi_version"
            )
        })?;
    if version != AOMI_FFI_VERSION {
        bail!(
            "FFI version mismatch: plugin={version}, host={AOMI_FFI_VERSION} (SDK {sdk_version}); rebuild for a supported FFI version"
        );
    }
    Ok(version)
}

pub fn validate_ffi_interfaces(required: &[String]) -> Result<()> {
    for interface in required {
        if !AOMI_FFI_INTERFACES.contains(&interface.as_str()) {
            bail!("unsupported required FFI interface: {interface}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_release_does_not_decide_explicit_ffi_compatibility() {
        assert_eq!(validate_ffi_version(Some(1), "99.0.0").unwrap(), 1);
        assert!(validate_ffi_version(Some(2), "5.1.1").is_err());
        assert!(validate_ffi_version(Some(0), "5.1.1").is_err());
    }

    #[test]
    fn unversioned_plugins_need_a_verified_legacy_sdk() {
        assert_eq!(validate_ffi_version(None, "5.1.1").unwrap(), 1);
        assert_eq!(validate_ffi_version(None, "5.1.2").unwrap(), 1);
        assert!(validate_ffi_version(None, "5.1.0").is_err());
        assert!(validate_ffi_version(None, "99.0.0").is_err());
    }

    #[test]
    fn unknown_required_interfaces_fail_closed() {
        assert!(validate_ffi_interfaces(&["routes_v1".into()]).is_ok());
        assert!(validate_ffi_interfaces(&["future_permission_v2".into()]).is_err());
    }
}
