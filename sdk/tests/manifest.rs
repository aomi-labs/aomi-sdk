mod common;

use aomi_sdk::{
    AOMI_SDK_VERSION, AppVariant, DynAomiApp, DynAsyncSink, DynManifest, DynToolDispatch,
    DynToolMetadata, DynToolResult,
};
use common::fixtures::TestApp;
use serde_json::Value;

macro_rules! impl_test_app {
    ($app:ty, $name:literal, $variant:expr) => {
        impl_test_app!($app, $name, $variant, {
            Some(vec!["evm-core".to_string()])
        });
    };
    ($app:ty, $name:literal, $variant:expr, $namespaces:block) => {
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

            fn variant(&self) -> Option<AppVariant> {
                $variant
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
fn manifest_variant_default_is_none() {
    assert_eq!(TestApp.manifest().variant, None);
}

#[test]
fn manifest_omits_variant_field_when_none() {
    let json = serde_json::to_value(TestApp.manifest()).expect("serialize");
    assert!(
        json.get("variant").is_none(),
        "variant field omitted when None; got: {json}"
    );
}

#[test]
fn app_variant_as_str_kebab_matches_host_builtin_app() {
    assert_eq!(AppVariant::Evm.as_str(), "evm");
    assert_eq!(AppVariant::Svm.as_str(), "svm");
    assert_eq!(AppVariant::SvmSelfBroadcast.as_str(), "svm-self-broadcast");
    assert_eq!(AppVariant::SvmAppBroadcast.as_str(), "svm-app-broadcast");
    assert_eq!(
        AppVariant::SvmBundleBroadcast.as_str(),
        "svm-bundle-broadcast"
    );
    assert_eq!(AppVariant::SvmOffChainSign.as_str(), "svm-off-chain-sign");
    assert_eq!(AppVariant::SvmReadOnly.as_str(), "svm-read-only");
}

#[test]
fn app_variant_default_namespaces_match_host_composition() {
    assert_eq!(AppVariant::Evm.default_namespaces(), &["evm-core"]);
    assert_eq!(AppVariant::Svm.default_namespaces(), &["svm-core"]);
    assert_eq!(
        AppVariant::SvmSelfBroadcast.default_namespaces(),
        &["svm-reads", "svm-stage", "svm-commit"]
    );
    assert_eq!(
        AppVariant::SvmAppBroadcast.default_namespaces(),
        &["svm-reads", "svm-stage"]
    );
    assert_eq!(
        AppVariant::SvmBundleBroadcast.default_namespaces(),
        &["svm-reads", "svm-stage", "svm-bundle"]
    );
    assert_eq!(
        AppVariant::SvmOffChainSign.default_namespaces(),
        &["svm-reads", "svm-sign-data"]
    );
    assert_eq!(AppVariant::SvmReadOnly.default_namespaces(), &["svm-reads"]);
}

#[test]
fn manifest_variant_populates_from_app_trait() {
    let manifest = VariantApp.manifest();
    assert_eq!(manifest.variant, Some("svm-app-broadcast".to_string()));
    assert_eq!(manifest.namespaces, Some(vec!["evm-core".to_string()]));
}

#[test]
fn manifest_serializes_variant_field() {
    let json = serde_json::to_value(ReadOnlyApp.manifest()).expect("serialize");
    assert_eq!(
        json.get("variant").and_then(Value::as_str),
        Some("svm-read-only")
    );

    let back: DynManifest = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back.variant, Some("svm-read-only".to_string()));
}

#[test]
fn manifest_can_opt_out_of_host_namespaces() {
    let manifest = NoHostNamespacesApp.manifest();
    assert_eq!(manifest.namespaces, Some(vec![]));
}

#[derive(Clone, Default)]
struct VariantApp;

#[derive(Clone, Default)]
struct ReadOnlyApp;

#[derive(Clone, Default)]
struct NoHostNamespacesApp;

impl_test_app!(
    VariantApp,
    "variant-app",
    Some(AppVariant::SvmAppBroadcast),
    { Some(vec!["evm-core".to_string()]) }
);
impl_test_app!(ReadOnlyApp, "read-only-app", Some(AppVariant::SvmReadOnly));
impl_test_app!(NoHostNamespacesApp, "no-host-namespaces", None, {
    Some(vec![])
});
