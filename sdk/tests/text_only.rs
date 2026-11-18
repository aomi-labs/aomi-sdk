//! A skills-only package: no tools, so `describe` lists none and every call is unknown.

use aomi_sdk::bindings::Guest;
use aomi_sdk::testing;

aomi_sdk::package!();

#[test]
fn a_package_without_tools_describes_none_and_answers_unknown_tool() {
    assert!(AomiPackage::describe().tools.is_empty());
    let e = AomiPackage::call_tool("x".into(), "{}".into(), testing::ctx()).unwrap_err();
    assert_eq!(e.code, "unknown_tool");
}
