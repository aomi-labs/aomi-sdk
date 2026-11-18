// Copyright 2025 Oxide Computer Company
// Modified for Aomi: generated clients send through `aomi_sdk::http`.

//! The OpenAPI client generator behind `aomi-build gen-client`, forked from
//! progenitor 0.14. It emits positional async methods whose requests go
//! through `aomi_sdk::http` and whose replies decode through
//! `aomi_progenitor_client`. Operations that need a transport the sandbox
//! does not offer (websockets, multipart and form bodies, OPTIONS, HEAD,
//! TRACE) fail generation.

#![deny(missing_docs)]

use std::collections::HashSet;

use openapiv3::OpenAPI;
use proc_macro2::TokenStream;
use quote::quote;
use thiserror::Error;
use typify::{TypeSpace, TypeSpaceSettings};

use crate::to_schema::ToSchema;

mod method;
mod template;
mod to_schema;
mod util;

#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum Error {
    #[error("unexpected value type {0}: {1}")]
    BadValue(String, serde_json::Value),
    #[error("type error {0}")]
    TypeError(#[from] typify::Error),
    #[error("unexpected or unhandled format in the OpenAPI document {0}")]
    UnexpectedFormat(String),
    #[error("invalid operation path {0}")]
    InvalidPath(String),
    #[error("internal error {0}")]
    InternalError(String),
}

#[allow(missing_docs)]
pub type Result<T> = std::result::Result<T, Error>;

/// OpenAPI generator.
pub struct Generator {
    type_space: TypeSpace,
}

impl Default for Generator {
    fn default() -> Self {
        Self {
            type_space: TypeSpace::new(TypeSpaceSettings::default().with_type_mod("types")),
        }
    }
}

impl Generator {
    /// Emit a [TokenStream] containing the generated client code.
    pub fn generate_tokens(&mut self, spec: &OpenAPI) -> Result<TokenStream> {
        validate_openapi(spec)?;

        // Convert our components dictionary to schemars
        let schemas = spec.components.iter().flat_map(|components| {
            components
                .schemas
                .iter()
                .map(|(name, ref_or_schema)| (name.clone(), ref_or_schema.to_schema()))
        });

        self.type_space.add_ref_types(schemas)?;

        let raw_methods = spec
            .paths
            .iter()
            .flat_map(|(path, ref_or_item)| {
                // Exclude externally defined path items.
                let item = ref_or_item.as_item().unwrap();
                item.iter().map(move |(method, operation)| {
                    (path.as_str(), method, operation, &item.parameters)
                })
            })
            .map(|(path, method, operation, path_parameters)| {
                self.process_operation(operation, &spec.components, path, method, path_parameters)
            })
            .collect::<Result<Vec<_>>>()?;

        let methods = raw_methods
            .iter()
            .map(|method| self.positional_method(method))
            .collect::<Result<Vec<_>>>()?;

        let types = self.type_space.to_stream();

        let client_docstring = {
            let mut s = format!("Client for {}", spec.info.title);

            if let Some(ss) = &spec.info.description {
                s.push_str("\n\n");
                s.push_str(ss);
            }
            if let Some(ss) = &spec.info.terms_of_service {
                s.push_str("\n\n");
                s.push_str(ss);
            }

            s.push_str(&format!("\n\nVersion: {}", spec.info.version));

            s
        };

        let version_str = &spec.info.version;

        // The allow(unused_imports) on the `pub use` is necessary with Rust
        // 1.76+, in case the generated file is not at the top level of the
        // crate.

        let file = quote! {
            // Re-export types that are used by the public interface of Client.
            #[allow(unused_imports)]
            pub use aomi_progenitor_client::{
                ClientInfo,
                Error,
                ResponseValue,
            };
            #[allow(unused_imports)]
            use aomi_progenitor_client::{encode_path, QueryParam};

            /// Types used as operation parameters and responses.
            #[allow(clippy::all)]
            pub mod types {
                #types
            }

            #[derive(Clone, Debug)]
            #[doc = #client_docstring]
            pub struct Client {
                pub(crate) baseurl: String,
            }

            impl Client {
                /// Create a new client.
                ///
                /// `baseurl` should include a scheme and hostname, as well as
                /// port and a path stem if applicable. The host must be in the
                /// package's `[network] hosts`.
                pub fn new(baseurl: &str) -> Self {
                    Self {
                        baseurl: baseurl.to_string(),
                    }
                }
            }

            impl ClientInfo for Client {
                fn api_version() -> &'static str {
                    #version_str
                }

                fn baseurl(&self) -> &str {
                    self.baseurl.as_str()
                }
            }

            #[allow(clippy::all)]
            impl Client {
                #(#methods)*
            }

            /// Items consumers will typically use such as the Client.
            pub mod prelude {
                #[allow(unused_imports)]
                pub use super::Client;
            }
        };

        Ok(file)
    }
}

fn validate_openapi_spec_version(spec_version: &str) -> Result<()> {
    // progenitor currenlty only support OAS 3.0.x
    if spec_version.trim().starts_with("3.0.") {
        Ok(())
    } else {
        Err(Error::UnexpectedFormat(format!(
            "invalid version: {}",
            spec_version
        )))
    }
}

/// Do some very basic checks of the OpenAPI documents.
pub fn validate_openapi(spec: &OpenAPI) -> Result<()> {
    validate_openapi_spec_version(spec.openapi.as_str())?;

    let mut opids = HashSet::new();
    spec.paths.paths.iter().try_for_each(|p| {
        match p.1 {
            openapiv3::ReferenceOr::Reference { reference: _ } => Err(Error::UnexpectedFormat(
                format!("path {} uses reference, unsupported", p.0,),
            )),
            openapiv3::ReferenceOr::Item(item) => {
                // Make sure every operation has an operation ID, and that each
                // operation ID is only used once in the document.
                item.iter().try_for_each(|(_, o)| {
                    if let Some(oid) = o.operation_id.as_ref() {
                        if !opids.insert(oid.to_string()) {
                            return Err(Error::UnexpectedFormat(format!(
                                "duplicate operation ID: {}",
                                oid,
                            )));
                        }
                    } else {
                        return Err(Error::UnexpectedFormat(format!(
                            "path {} is missing operation ID",
                            p.0,
                        )));
                    }
                    Ok(())
                })
            }
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{Error, validate_openapi_spec_version};

    #[test]
    fn test_bad_value() {
        assert_eq!(
            Error::BadValue("nope".to_string(), json! { "nope"},).to_string(),
            "unexpected value type nope: \"nope\"",
        );
    }

    #[test]
    fn test_type_error() {
        assert_eq!(
            Error::UnexpectedFormat("nope".to_string()).to_string(),
            "unexpected or unhandled format in the OpenAPI document nope",
        );
    }

    #[test]
    fn test_invalid_path() {
        assert_eq!(
            Error::InvalidPath("nope".to_string()).to_string(),
            "invalid operation path nope",
        );
    }

    #[test]
    fn test_internal_error() {
        assert_eq!(
            Error::InternalError("nope".to_string()).to_string(),
            "internal error nope",
        );
    }

    fn generate(operation: &str) -> crate::Result<String> {
        let spec = format!(
            "openapi: 3.0.3\ninfo: {{ title: t, version: '1' }}\npaths:\n  /x:\n{operation}"
        );
        let spec = serde_yaml::from_str(&spec).unwrap();
        crate::Generator::default()
            .generate_tokens(&spec)
            .map(|t| t.to_string())
    }

    #[test]
    fn requests_go_through_the_sdk() {
        let code = generate(
            "    put:\n      operationId: putX\n      responses: { '204': { description: ok } }\n",
        )
        .unwrap();
        assert!(code.contains(":: aomi_sdk :: http :: put (url)"), "{code}");
        assert!(code.contains("ResponseValue :: empty (response)"), "{code}");
        assert!(!code.contains("reqwest"), "{code}");
    }

    #[test]
    fn transports_the_sandbox_lacks_fail_generation() {
        let head = generate(
            "    head:\n      operationId: headX\n      responses: { '200': { description: ok } }\n",
        );
        assert!(
            head.unwrap_err()
                .to_string()
                .contains("unsupported method head")
        );

        let form = generate(
            "    post:\n      operationId: formX\n      requestBody:\n        content:\n          \
             application/x-www-form-urlencoded:\n            schema: { type: object }\n      \
             responses: { '200': { description: ok } }\n",
        );
        assert!(form.unwrap_err().to_string().contains("form body of formX"));

        let socket = generate(
            "    get:\n      operationId: sockX\n      x-dropshot-websocket: {}\n      \
             responses: { '101': { description: up } }\n",
        );
        assert!(
            socket
                .unwrap_err()
                .to_string()
                .contains("websocket operation sockX")
        );

        let upgrade = generate(
            "    get:\n      operationId: upX\n      responses: { '101': { description: up } }\n",
        );
        assert!(
            upgrade
                .unwrap_err()
                .to_string()
                .contains("upgrade response of upX")
        );
    }

    #[test]
    fn test_validate_openapi_spec_version() {
        assert!(validate_openapi_spec_version("3.0.0").is_ok());
        assert!(validate_openapi_spec_version("3.0.1").is_ok());
        assert!(validate_openapi_spec_version("3.0.4").is_ok());
        assert!(validate_openapi_spec_version("3.0.5-draft").is_ok());
        assert_eq!(
            validate_openapi_spec_version("3.1.0")
                .unwrap_err()
                .to_string(),
            "unexpected or unhandled format in the OpenAPI document invalid version: 3.1.0"
        );
    }
}
