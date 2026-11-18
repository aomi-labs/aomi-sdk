//! Generates the fixture clients from `specs/` with the canonical generator.

use std::path::Path;

fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    for name in ["across", "verbs"] {
        let path = format!("specs/{name}.yaml");
        println!("cargo:rerun-if-changed={path}");
        let spec: openapiv3::OpenAPI =
            serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let tokens = aomi_progenitor_impl::Generator::default()
            .generate_tokens(&spec)
            .unwrap_or_else(|e| panic!("{path}: {e}"));
        std::fs::write(
            Path::new(&out).join(format!("{name}.rs")),
            tokens.to_string(),
        )
        .unwrap();
    }
}
