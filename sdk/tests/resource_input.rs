mod common;

use aomi_sdk::{
    DynToolMetadata, ResourceInputDeclaration, resource_input_schema, validate_resource_inputs,
};
use serde_json::json;

fn declarations() -> Vec<ResourceInputDeclaration> {
    serde_json::from_value(json!([
        {"pointer":"/order_plan","kind":"data.value@1","schema":"limitless.order-plan@1"},
        {"pointer":"/order_signature","kind":"evm.signature@1","paired_with":"/order_plan"}
    ]))
    .unwrap()
}

#[test]
fn legacy_metadata_has_no_input_resolution() {
    let metadata: DynToolMetadata = serde_json::from_value(json!({
        "name":"echo","app":"test","description":"Echo",
        "parameters_schema":{},"supports_async":false
    }))
    .unwrap();
    assert!(metadata.resource_inputs.is_empty());
    assert!(
        serde_json::to_value(metadata)
            .unwrap()
            .get("resource_inputs")
            .is_none()
    );
}

#[test]
fn registration_rejects_authority_overlap_and_unbound_signatures() {
    let valid = declarations();
    validate_resource_inputs(&valid).unwrap();
    let mut wire = serde_json::to_value(&valid[0]).unwrap();
    wire["owner"] = json!("forged");
    assert!(serde_json::from_value::<ResourceInputDeclaration>(wire).is_err());
    for pointer in ["", "/bad~2", "/order_signature", "/order_signature/child"] {
        let mut invalid = valid.clone();
        invalid[0].pointer = pointer.into();
        assert!(validate_resource_inputs(&invalid).is_err());
    }
    for pair in [None, Some("/missing"), Some("/order_signature")] {
        let mut invalid = valid.clone();
        invalid[1].paired_with = pair.map(str::to_owned);
        assert!(validate_resource_inputs(&invalid).is_err());
    }
    let mut invalid = valid.clone();
    invalid[0].paired_with = Some("/order_signature".into());
    assert!(validate_resource_inputs(&invalid).is_err());
    let mut invalid = valid;
    invalid[0].schema = Some("x".repeat(8193));
    assert!(validate_resource_inputs(&invalid).is_err());
}

#[test]
fn schema_preserves_literals_and_adds_only_exact_declared_refs() {
    let original = json!({"type":"object","properties":{
        "order_plan":{"$ref":"#/$defs/Plan"},
        "order_signature":{"type":["string","null"]},
        "ordinary":{"type":"string"}
    }});
    let schema = resource_input_schema(original.clone(), &declarations()).unwrap();
    for key in ["order_plan", "order_signature"] {
        assert_eq!(
            schema["properties"][key]["anyOf"][0],
            original["properties"][key]
        );
        let handle = &schema["properties"][key]["anyOf"][1];
        assert_eq!(handle["required"], json!(["uri"]));
        assert_eq!(handle["additionalProperties"], false);
        assert_eq!(handle["properties"].as_object().unwrap().len(), 1);
    }
    assert_eq!(
        schema["properties"]["ordinary"],
        original["properties"]["ordinary"]
    );
    assert!(resource_input_schema(json!({}), &declarations()).is_err());
}
