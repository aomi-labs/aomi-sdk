mod common;

use aomi_sdk::{
    AOMI_SDK_VERSION, DynAomiApp, DynAsyncSink, DynManifest, DynToolDispatch, DynToolMetadata,
    DynToolResult, EvmExecutionRequirement,
};
use common::fixtures::TestApp;

macro_rules! impl_test_app {
    ($app:ty, $name:literal, $namespaces:block) => {
        impl DynAomiApp for $app {
            fn name(&self) -> &'static str {
                $name
            }

            fn version(&self) -> &'static str {
                "0.0.0"
            }

            fn preamble(&self) -> &'static str {
                ""
            }

            fn tools(&self) -> Vec<DynToolMetadata> {
                vec![]
            }

            fn start_tool(&self, _: &str, _: &str, _: &str, _: DynAsyncSink) -> DynToolDispatch {
                DynToolDispatch::Ready(DynToolResult::err("not needed"))
            }

            fn namespaces(&self) -> Option<Vec<String>> {
                $namespaces
            }
        }
    };
}

#[test]
fn manifest_defaults_to_evm_core_namespace() {
    let manifest = TestApp.manifest();
    assert_eq!(manifest.sdk_version, AOMI_SDK_VERSION);
    assert_eq!(manifest.namespaces, Some(vec!["evm-core".to_string()]));
}

#[test]
fn manifest_omits_namespace_field_when_none() {
    let json = serde_json::to_value(NoHostNamespacesFieldApp.manifest()).expect("serialize");
    assert!(
        json.get("namespaces").is_none(),
        "namespaces field omitted when None; got: {json}"
    );

    let back: DynManifest = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back.namespaces, None);
}

#[test]
fn manifest_populates_explicit_svm_namespaces() {
    let manifest = SvmSelfBroadcastApp.manifest();
    assert_eq!(
        manifest.namespaces,
        Some(vec![
            "svm-reads".to_string(),
            "svm-ix-broadcast".to_string(),
            "svm-tx-broadcast".to_string(),
        ])
    );
}

#[test]
fn manifest_serializes_namespace_field() {
    let json = serde_json::to_value(SvmReadOnlyApp.manifest()).expect("serialize");
    assert_eq!(
        json.get("namespaces"),
        Some(&serde_json::json!(["svm-reads"]))
    );

    let back: DynManifest = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back.namespaces, Some(vec!["svm-reads".to_string()]));
}

#[test]
fn manifest_can_opt_out_of_host_namespaces() {
    let manifest = EmptyHostNamespacesApp.manifest();
    assert_eq!(manifest.namespaces, Some(vec![]));
}

#[test]
fn manifest_serializes_backend_owned_4337_requirement() {
    let manifest = Sponsored4337App.manifest();
    assert_eq!(
        manifest.evm_execution,
        Some(EvmExecutionRequirement::AomiSponsored4337)
    );

    let json = serde_json::to_value(manifest).expect("serialize");
    assert_eq!(json["evm_execution"], "aomi_sponsored_4337");
}

#[derive(Clone, Default)]
struct SvmSelfBroadcastApp;

#[derive(Clone, Default)]
struct SvmReadOnlyApp;

#[derive(Clone, Default)]
struct EmptyHostNamespacesApp;

#[derive(Clone, Default)]
struct NoHostNamespacesFieldApp;

#[derive(Clone, Default)]
struct Sponsored4337App;

impl DynAomiApp for Sponsored4337App {
    fn name(&self) -> &'static str {
        "sponsored-4337-app"
    }

    fn version(&self) -> &'static str {
        "0.0.0"
    }

    fn preamble(&self) -> &'static str {
        ""
    }

    fn tools(&self) -> Vec<DynToolMetadata> {
        vec![]
    }

    fn start_tool(&self, _: &str, _: &str, _: &str, _: DynAsyncSink) -> DynToolDispatch {
        DynToolDispatch::Ready(DynToolResult::err("not needed"))
    }

    fn evm_execution(&self) -> Option<EvmExecutionRequirement> {
        Some(EvmExecutionRequirement::AomiSponsored4337)
    }
}

impl_test_app!(SvmSelfBroadcastApp, "svm-self-broadcast-app", {
    Some(vec![
        "svm-reads".to_string(),
        "svm-ix-broadcast".to_string(),
        "svm-tx-broadcast".to_string(),
    ])
});
impl_test_app!(SvmReadOnlyApp, "read-only-app", {
    Some(vec!["svm-reads".to_string()])
});
impl_test_app!(EmptyHostNamespacesApp, "empty-host-namespaces", {
    Some(vec![])
});
impl_test_app!(NoHostNamespacesFieldApp, "no-host-namespaces-field", {
    None
});
