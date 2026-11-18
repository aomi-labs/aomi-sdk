// Copyright 2025 Oxide Computer Company
// Modified for Aomi: requests go through `aomi_sdk::http`.

use std::{cmp::Ordering, collections::BTreeSet, str::FromStr};

use openapiv3::{Components, Parameter, ReferenceOr, Response, StatusCode};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use typify::TypeSpace;

use crate::{
    Error, Generator, Result,
    template::PathTemplate,
    util::{Case, items, parameter_map, sanitize, unique_ident_from},
};
use crate::{to_schema::ToSchema, util::ReferenceOrExt};

/// The intermediate representation of an operation that will become a method.
pub(crate) struct OperationMethod {
    pub operation_id: String,
    pub method: HttpMethod,
    pub path: PathTemplate,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub params: Vec<OperationParameter>,
    pub responses: Vec<OperationResponse>,
}

/// The methods `aomi_sdk::http` can send.
pub enum HttpMethod {
    Get,
    Put,
    Post,
    Delete,
    Patch,
}

impl std::str::FromStr for HttpMethod {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "get" => Ok(Self::Get),
            "put" => Ok(Self::Put),
            "post" => Ok(Self::Post),
            "delete" => Ok(Self::Delete),
            "patch" => Ok(Self::Patch),
            _ => Err(Error::UnexpectedFormat(format!(
                "unsupported method {}: aomi_sdk::http sends GET, PUT, POST, DELETE and PATCH",
                s
            ))),
        }
    }
}
impl HttpMethod {
    fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "get",
            HttpMethod::Put => "put",
            HttpMethod::Post => "post",
            HttpMethod::Delete => "delete",
            HttpMethod::Patch => "patch",
        }
    }
}

struct MethodSigBody {
    success: TokenStream,
    error: TokenStream,
    body: TokenStream,
}

pub struct OperationParameter {
    /// Sanitized parameter name.
    pub name: String,
    /// Original parameter name provided by the API.
    pub api_name: String,
    pub description: Option<String>,
    pub typ: OperationParameterType,
    pub kind: OperationParameterKind,
}

#[derive(Eq, PartialEq)]
pub enum OperationParameterType {
    Type(typify::TypeId),
    RawBody,
}

#[derive(Debug, PartialEq, Eq)]
pub enum OperationParameterKind {
    Path,
    Query(bool),
    Header(bool),
    // TODO bodies may be optional
    Body(BodyContentType),
}

impl OperationParameterKind {
    fn is_required(&self) -> bool {
        match self {
            OperationParameterKind::Path => true,
            OperationParameterKind::Query(required) => *required,
            OperationParameterKind::Header(required) => *required,
            // TODO may be optional
            OperationParameterKind::Body(_) => true,
        }
    }
    fn is_optional(&self) -> bool {
        !self.is_required()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum BodyContentType {
    OctetStream,
    Json,
    FormUrlencoded,
    Text(String),
}

impl FromStr for BodyContentType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let offset = s.find(';').unwrap_or(s.len());
        match &s[..offset] {
            "application/octet-stream" => Ok(Self::OctetStream),
            "application/json" => Ok(Self::Json),
            "application/x-www-form-urlencoded" => Ok(Self::FormUrlencoded),
            "text/plain" | "text/x-markdown" => Ok(Self::Text(String::from(&s[..offset]))),
            _ => Err(Error::UnexpectedFormat(format!(
                "unexpected content type: {}",
                s
            ))),
        }
    }
}

impl std::fmt::Display for BodyContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::OctetStream => "application/octet-stream",
            Self::Json => "application/json",
            Self::FormUrlencoded => "application/x-www-form-urlencoded",
            Self::Text(typ) => typ,
        })
    }
}

#[derive(Debug)]
pub(crate) struct OperationResponse {
    pub status_code: OperationResponseStatus,
    pub typ: OperationResponseKind,
    // TODO this isn't currently used because dropshot doesn't give us a
    // particularly useful message here.
    #[allow(dead_code)]
    description: Option<String>,
}

impl Eq for OperationResponse {}
impl PartialEq for OperationResponse {
    fn eq(&self, other: &Self) -> bool {
        self.status_code == other.status_code
    }
}
impl Ord for OperationResponse {
    fn cmp(&self, other: &Self) -> Ordering {
        self.status_code.cmp(&other.status_code)
    }
}
impl PartialOrd for OperationResponse {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum OperationResponseStatus {
    Code(u16),
    Range(u16),
    Default,
}

impl OperationResponseStatus {
    fn to_value(&self) -> u16 {
        match self {
            OperationResponseStatus::Code(code) => {
                assert!(*code < 1000);
                *code
            }
            OperationResponseStatus::Range(range) => {
                assert!(*range < 10);
                *range * 100
            }
            OperationResponseStatus::Default => 1000,
        }
    }

    pub fn is_success_or_default(&self) -> bool {
        matches!(
            self,
            OperationResponseStatus::Default
                | OperationResponseStatus::Code(101)
                | OperationResponseStatus::Code(200..=299)
                | OperationResponseStatus::Range(2)
        )
    }

    pub fn is_error_or_default(&self) -> bool {
        matches!(
            self,
            OperationResponseStatus::Default
                | OperationResponseStatus::Code(400..=599)
                | OperationResponseStatus::Range(4..=5)
        )
    }

    pub fn is_default(&self) -> bool {
        matches!(self, OperationResponseStatus::Default)
    }
}

impl Ord for OperationResponseStatus {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_value().cmp(&other.to_value())
    }
}

impl PartialOrd for OperationResponseStatus {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub(crate) enum OperationResponseKind {
    Type(typify::TypeId),
    None,
    Raw,
}

impl OperationResponseKind {
    pub fn into_tokens(self, type_space: &TypeSpace) -> TokenStream {
        match self {
            OperationResponseKind::Type(ref type_id) => {
                let type_name = type_space.get_type(type_id).unwrap().ident();
                quote! { #type_name }
            }
            OperationResponseKind::None => {
                quote! { () }
            }
            OperationResponseKind::Raw => {
                quote! { Vec<u8> }
            }
        }
    }
}

impl Generator {
    pub(crate) fn process_operation(
        &mut self,
        operation: &openapiv3::Operation,
        components: &Option<Components>,
        path: &str,
        method: &str,
        path_parameters: &[ReferenceOr<Parameter>],
    ) -> Result<OperationMethod> {
        let operation_id = operation.operation_id.as_ref().unwrap();

        let mut combined_path_parameters = parameter_map(path_parameters, components)?;
        for operation_param in items(&operation.parameters, components) {
            let parameter = operation_param?;
            combined_path_parameters.insert(&parameter.parameter_data_ref().name, parameter);
        }

        // Filter out any path parameters that have been overridden by an
        // operation parameter
        let mut params = combined_path_parameters
            .values()
            .map(|parameter| {
                match parameter {
                    openapiv3::Parameter::Path {
                        parameter_data,
                        style: openapiv3::PathStyle::Simple,
                    } => {
                        // Path parameters MUST be required.
                        assert!(parameter_data.required);

                        let schema = parameter_data.schema()?.to_schema();

                        let name = sanitize(
                            &format!("{}-{}", operation_id, parameter_data.name),
                            Case::Pascal,
                        );
                        let typ = self.type_space.add_type_with_name(&schema, Some(name))?;

                        Ok(OperationParameter {
                            name: sanitize(&parameter_data.name, Case::Snake),
                            api_name: parameter_data.name.clone(),
                            description: parameter_data.description.clone(),
                            typ: OperationParameterType::Type(typ),
                            kind: OperationParameterKind::Path,
                        })
                    }
                    openapiv3::Parameter::Query {
                        parameter_data,
                        allow_reserved: _, // We always encode reserved chars
                        style: openapiv3::QueryStyle::Form,
                        allow_empty_value: _, // Irrelevant for this client
                    } => {
                        let schema = parameter_data.schema()?.to_schema();
                        let name = sanitize(
                            &format!(
                                "{}-{}",
                                operation.operation_id.as_ref().unwrap(),
                                parameter_data.name,
                            ),
                            Case::Pascal,
                        );

                        let type_id = self.type_space.add_type_with_name(&schema, Some(name))?;

                        let ty = self.type_space.get_type(&type_id).unwrap();

                        // If the type is itself optional, then we'll treat it
                        // as optional (irrespective of the `required` field on
                        // the parameter) and use the "inner" type.
                        let details = ty.details();
                        let (type_id, required) =
                            if let typify::TypeDetails::Option(inner_type_id) = details {
                                (inner_type_id, false)
                            } else {
                                (type_id, parameter_data.required)
                            };

                        Ok(OperationParameter {
                            name: sanitize(&parameter_data.name, Case::Snake),
                            api_name: parameter_data.name.clone(),
                            description: parameter_data.description.clone(),
                            typ: OperationParameterType::Type(type_id),
                            kind: OperationParameterKind::Query(required),
                        })
                    }
                    openapiv3::Parameter::Header {
                        parameter_data,
                        style: openapiv3::HeaderStyle::Simple,
                    } => {
                        let schema = parameter_data.schema()?.to_schema();
                        let name = sanitize(
                            &format!(
                                "{}-{}",
                                operation.operation_id.as_ref().unwrap(),
                                parameter_data.name,
                            ),
                            Case::Pascal,
                        );

                        let typ = self.type_space.add_type_with_name(&schema, Some(name))?;

                        Ok(OperationParameter {
                            name: sanitize(&parameter_data.name, Case::Snake),
                            api_name: parameter_data.name.clone(),
                            description: parameter_data.description.clone(),
                            typ: OperationParameterType::Type(typ),
                            kind: OperationParameterKind::Header(parameter_data.required),
                        })
                    }
                    openapiv3::Parameter::Path { style, .. } => Err(Error::UnexpectedFormat(
                        format!("unsupported style of path parameter {:#?}", style,),
                    )),
                    openapiv3::Parameter::Query { style, .. } => Err(Error::UnexpectedFormat(
                        format!("unsupported style of query parameter {:#?}", style,),
                    )),
                    cookie @ openapiv3::Parameter::Cookie { .. } => Err(Error::UnexpectedFormat(
                        format!("cookie parameters are not supported {:#?}", cookie,),
                    )),
                }
            })
            .collect::<Result<Vec<_>>>()?;

        if operation.extensions.contains_key("x-dropshot-websocket") {
            return Err(Error::UnexpectedFormat(format!(
                "websocket operation {} is not supported",
                operation_id
            )));
        }

        if let Some(body_param) = self.get_body_param(operation, components)? {
            params.push(body_param);
        }

        let tmp = crate::template::parse(path)?;
        let names = tmp.names();

        sort_params(&mut params, &names);

        let mut success = false;

        let mut responses =
            operation
                .responses
                .default
                .iter()
                .map(|response_or_ref| {
                    Ok((
                        OperationResponseStatus::Default,
                        response_or_ref.item(components)?,
                    ))
                })
                .chain(operation.responses.responses.iter().map(
                    |(status_code, response_or_ref)| {
                        Ok((
                            match status_code {
                                StatusCode::Code(code) => OperationResponseStatus::Code(*code),
                                StatusCode::Range(range) => OperationResponseStatus::Range(*range),
                            },
                            response_or_ref.item(components)?,
                        ))
                    },
                ))
                .map(|v: Result<(OperationResponseStatus, &Response)>| {
                    let (status_code, response) = v?;

                    // We categorize responses as "typed" based on the
                    // "application/json" content type, "raw" if there's any
                    // other response content type (we don't investigate
                    // further), or "none" if there is no content. Upgrades
                    // (101) are not supported.
                    // TODO if there are multiple response content types we could
                    // treat those like different response types and create an
                    // enum; the generated client method would check for the
                    // content type of the response just as it currently examines
                    // the status code.
                    let typ = if let Some(mt) = response.content.iter().find_map(|(x, v)| {
                        (x == "application/json" || x.starts_with("application/json;")).then_some(v)
                    }) {
                        assert!(mt.encoding.is_empty());

                        let typ = if let Some(schema) = &mt.schema {
                            let schema = schema.to_schema();
                            let name = sanitize(
                                &format!("{}-response", operation.operation_id.as_ref().unwrap(),),
                                Case::Pascal,
                            );
                            self.type_space.add_type_with_name(&schema, Some(name))?
                        } else {
                            todo!("media type encoding, no schema: {:#?}", mt);
                        };

                        OperationResponseKind::Type(typ)
                    } else if status_code == OperationResponseStatus::Code(101) {
                        return Err(Error::UnexpectedFormat(format!(
                            "upgrade response of {} is not supported",
                            operation_id
                        )));
                    } else if response.content.first().is_some() {
                        OperationResponseKind::Raw
                    } else {
                        OperationResponseKind::None
                    };

                    // See if there's a status code that covers success cases.
                    if matches!(
                        status_code,
                        OperationResponseStatus::Default
                            | OperationResponseStatus::Code(101)
                            | OperationResponseStatus::Code(200..=299)
                            | OperationResponseStatus::Range(2)
                    ) {
                        success = true;
                    }

                    let description = if response.description.is_empty() {
                        None
                    } else {
                        Some(response.description.clone())
                    };

                    Ok(OperationResponse {
                        status_code,
                        typ,
                        description,
                    })
                })
                .collect::<Result<Vec<_>>>()?;

        // If the API has declined to specify the characteristics of a
        // successful response, we cons up a generic one. Note that this is
        // technically permissible within OpenAPI, but advised against by the
        // spec.
        if !success {
            responses.push(OperationResponse {
                status_code: OperationResponseStatus::Range(2),
                typ: OperationResponseKind::Raw,
                description: None,
            });
        }

        Ok(OperationMethod {
            operation_id: sanitize(operation_id, Case::Snake),
            method: HttpMethod::from_str(method)?,
            path: tmp,
            summary: operation.summary.clone().filter(|s| !s.is_empty()),
            description: operation.description.clone().filter(|s| !s.is_empty()),
            params,
            responses,
        })
    }

    pub(crate) fn positional_method(&self, method: &OperationMethod) -> Result<TokenStream> {
        let operation_id = format_ident!("{}", method.operation_id);

        // Render each parameter as it will appear in the method signature.
        let params = method
            .params
            .iter()
            .map(|param| {
                let name = format_ident!("{}", param.name);
                let typ = match (&param.typ, param.kind.is_optional()) {
                    (OperationParameterType::Type(type_id), false) => self
                        .type_space
                        .get_type(type_id)
                        .unwrap()
                        .parameter_ident_with_lifetime("a"),
                    (OperationParameterType::Type(type_id), true) => {
                        let t = self
                            .type_space
                            .get_type(type_id)
                            .unwrap()
                            .parameter_ident_with_lifetime("a");
                        quote! { Option<#t> }
                    }
                    (OperationParameterType::RawBody, false) => match &param.kind {
                        OperationParameterKind::Body(BodyContentType::OctetStream) => {
                            quote! { B }
                        }
                        OperationParameterKind::Body(BodyContentType::Text(_)) => {
                            quote! { String }
                        }
                        _ => unreachable!(),
                    },
                    (OperationParameterType::RawBody, true) => unreachable!(),
                };
                quote! {
                    #name: #typ
                }
            })
            .collect::<Vec<_>>();

        let raw_body_param = method.params.iter().any(|param| {
            param.typ == OperationParameterType::RawBody
                && param.kind == OperationParameterKind::Body(BodyContentType::OctetStream)
        });

        let bounds = if raw_body_param {
            quote! { <'a, B: Into<Vec<u8>> > }
        } else {
            quote! { <'a> }
        };

        let doc_comment = make_doc_comment(method);

        let MethodSigBody {
            success: success_type,
            error: error_type,
            body,
        } = self.method_sig_body(method)?;

        Ok(quote! {
            #[doc = #doc_comment]
            pub async fn #operation_id #bounds (
                &'a self,
                #(#params),*
            ) -> Result<
                ResponseValue<#success_type>,
                Error<#error_type>,
            > {
                #body
            }
        })
    }

    /// Returns the success and error types and the body that marshals the
    /// arguments, sends the request through `aomi_sdk::http` and decodes the
    /// reply by status.
    fn method_sig_body(&self, method: &OperationMethod) -> Result<MethodSigBody> {
        let param_names = method
            .params
            .iter()
            .map(|param| format_ident!("{}", param.name))
            .collect::<Vec<_>>();

        // Generate a unique Ident for internal variables
        let url_ident = unique_ident_from("url", &param_names);
        let request_ident = unique_ident_from("request", &param_names);
        let response_ident = unique_ident_from("response", &param_names);

        // Generate code for query parameters.
        let query_params = method
            .params
            .iter()
            .filter_map(|param| match &param.kind {
                OperationParameterKind::Query(_) => {
                    let qn = &param.api_name;
                    let qn_ident = format_ident!("{}", &param.name);
                    Some(quote! {
                        &QueryParam::new(#qn, &#qn_ident)
                    })
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        let headers = method
            .params
            .iter()
            .filter_map(|param| match &param.kind {
                OperationParameterKind::Header(required) => {
                    let hn = &param.api_name;
                    let hn_ident = format_ident!("{}", &param.name);
                    let res = if *required {
                        quote! {
                            #request_ident = #request_ident.header(#hn, #hn_ident.to_string());
                        }
                    } else {
                        quote! {
                            if let Some(value) = #hn_ident {
                                #request_ident = #request_ident.header(#hn, value.to_string());
                            }
                        }
                    };
                    Some(res)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        // Generate the path rename map; then use it to generate code for
        // assigning the path parameters to the `url` variable.
        let url_renames = method
            .params
            .iter()
            .filter_map(|param| match &param.kind {
                OperationParameterKind::Path => Some((&param.api_name, &param.name)),
                _ => None,
            })
            .collect();

        let url_path = method.path.compile(url_renames, quote! { self });
        let url_path = quote! {
            let #url_ident = #url_path;
        };

        // Generate code to handle the body param.
        let body_func = method.params.iter().filter_map(|param| {
            match (&param.kind, &param.typ) {
                (
                    OperationParameterKind::Body(BodyContentType::OctetStream),
                    OperationParameterType::RawBody,
                ) => Some(quote! {
                    .header("content-type", "application/octet-stream")
                    .body(body)
                }),
                (
                    OperationParameterKind::Body(BodyContentType::Text(mime_type)),
                    OperationParameterType::RawBody,
                ) => Some(quote! {
                    .header("content-type", #mime_type)
                    .body(body)
                }),
                (
                    OperationParameterKind::Body(BodyContentType::Json),
                    OperationParameterType::Type(_),
                ) => Some(quote! {
                    // Serialization errors are deferred to `send`.
                    .json(&body)
                }),
                (OperationParameterKind::Body(_), _) => {
                    unreachable!("invalid body kind/type combination")
                }
                _ => None,
            }
        });
        // ... and there can be at most one body.
        assert!(body_func.clone().count() <= 1);

        let (success_response_items, response_type) =
            self.extract_responses(method, OperationResponseStatus::is_success_or_default);

        let success_response_matches = success_response_items.iter().map(|response| {
            let pat = match &response.status_code {
                OperationResponseStatus::Code(code) => quote! { #code },
                OperationResponseStatus::Range(_) | OperationResponseStatus::Default => {
                    quote! { 200 ..= 299 }
                }
            };

            let decode = match &response.typ {
                OperationResponseKind::Type(_) => {
                    quote! {
                        ResponseValue::from_response(#response_ident)
                    }
                }
                OperationResponseKind::None => {
                    quote! {
                        Ok(ResponseValue::empty(#response_ident))
                    }
                }
                OperationResponseKind::Raw => {
                    quote! {
                        Ok(ResponseValue::raw(#response_ident))
                    }
                }
            };

            quote! { #pat => { #decode } }
        });

        // Errors...
        let (error_response_items, error_type) =
            self.extract_responses(method, OperationResponseStatus::is_error_or_default);

        let error_response_matches = error_response_items.iter().map(|response| {
            let pat = match &response.status_code {
                OperationResponseStatus::Code(code) => {
                    quote! { #code }
                }
                OperationResponseStatus::Range(r) => {
                    let min = r * 100;
                    let max = min + 99;
                    quote! { #min ..= #max }
                }

                OperationResponseStatus::Default => {
                    quote! { _ }
                }
            };

            let decode = match &response.typ {
                OperationResponseKind::Type(_) => {
                    quote! {
                        Err(Error::ErrorResponse(
                            ResponseValue::from_response(#response_ident)?
                        ))
                    }
                }
                OperationResponseKind::None => {
                    quote! {
                        Err(Error::ErrorResponse(
                            ResponseValue::empty(#response_ident)
                        ))
                    }
                }
                OperationResponseKind::Raw => {
                    quote! {
                        Err(Error::ErrorResponse(
                            ResponseValue::raw(#response_ident)
                        ))
                    }
                }
            };

            quote! { #pat => { #decode } }
        });

        let accept_header = matches!(
            (&response_type, &error_type),
            (OperationResponseKind::Type(_), _)
                | (OperationResponseKind::None, OperationResponseKind::Type(_))
        )
        .then(|| {
            quote! {
                .header("accept", "application/json")
            }
        });

        // Generate the catch-all case for other statuses. If the operation
        // specifies a default response, we've already generated a default
        // match as part of error response code handling. (And we've handled
        // the default as a success response as well.) Otherwise the catch-all
        // produces an error corresponding to a response not specified in the
        // API description.
        let default_response = match method.responses.iter().last() {
            Some(response) if response.status_code.is_default() => quote! {},
            _ => {
                quote! { _ => Err(Error::UnexpectedResponse(#response_ident)), }
            }
        };

        let method_func = format_ident!("{}", method.method.as_str());

        // Header params reassign the request after it is built.
        let mutable = (!headers.is_empty()).then(|| quote! { mut });

        let body_impl = quote! {
            #url_path

            let #mutable #request_ident = ::aomi_sdk::http:: #method_func (#url_ident)
                #accept_header
                #(#body_func)*
                .header("api-version", Self::api_version())
                #( .query(#query_params) )*;

            #(#headers)*

            let #response_ident = #request_ident.send().await?;

            match #response_ident.status() {
                #(#success_response_matches)*
                #(#error_response_matches)*
                #default_response
            }
        };

        Ok(MethodSigBody {
            success: response_type.into_tokens(&self.type_space),
            error: error_type.into_tokens(&self.type_space),
            body: body_impl,
        })
    }

    /// Extract responses that match criteria specified by the `filter`. The
    /// result is a `Vec<OperationResponse>` that enumerates the cases matching
    /// the filter, and a `TokenStream` that represents the generated type for
    /// those cases.
    pub(crate) fn extract_responses<'a>(
        &self,
        method: &'a OperationMethod,
        filter: fn(&OperationResponseStatus) -> bool,
    ) -> (Vec<&'a OperationResponse>, OperationResponseKind) {
        let mut response_items = method
            .responses
            .iter()
            .filter(|response| filter(&response.status_code))
            .collect::<Vec<_>>();
        response_items.sort();

        // If we have a success range and a default, we can pop off the default
        // since it will never be hit. Note that this is a no-op for error
        // responses.
        if let [
            ..,
            OperationResponse {
                status_code: OperationResponseStatus::Range(2),
                ..
            },
            OperationResponse {
                status_code: OperationResponseStatus::Default,
                ..
            },
        ] = response_items[..]
        {
            response_items.pop();
        }

        let response_types = response_items
            .iter()
            .map(|response| response.typ.clone())
            .collect::<BTreeSet<_>>();

        // TODO to deal with multiple response types, we'll need to create an
        // enum type with variants for each of the response types.
        assert!(response_types.len() <= 1);
        let response_type = response_types
            .into_iter()
            .next()
            // TODO should this be OperationResponseType::Raw?
            .unwrap_or(OperationResponseKind::None);
        (response_items, response_type)
    }

    fn get_body_param(
        &mut self,
        operation: &openapiv3::Operation,
        components: &Option<Components>,
    ) -> Result<Option<OperationParameter>> {
        let body = match &operation.request_body {
            Some(body) => body.item(components)?,
            None => return Ok(None),
        };

        let (content_str, media_type) = match (body.content.first(), body.content.len()) {
            (None, _) => return Ok(None),
            (Some(first), 1) => first,
            (_, n) => todo!(
                "more media types than expected for {}: {}",
                operation.operation_id.as_ref().unwrap(),
                n,
            ),
        };

        let schema = media_type.schema.as_ref().ok_or_else(|| {
            Error::UnexpectedFormat("No schema specified for request body".to_string())
        })?;

        let content_type = BodyContentType::from_str(content_str)?;

        let typ = match content_type {
            BodyContentType::OctetStream => {
                // For an octet stream, we expect a simple, specific schema:
                // "schema": {
                //     "type": "string",
                //     "format": "binary"
                // }
                match schema.item(components)? {
                    openapiv3::Schema {
                        schema_data:
                            openapiv3::SchemaData {
                                nullable: false,
                                discriminator: None,
                                default: None,
                                // Other fields that describe or document the
                                // schema are fine.
                                ..
                            },
                        schema_kind:
                            openapiv3::SchemaKind::Type(openapiv3::Type::String(
                                openapiv3::StringType {
                                    format:
                                        openapiv3::VariantOrUnknownOrEmpty::Item(
                                            openapiv3::StringFormat::Binary,
                                        ),
                                    pattern: None,
                                    enumeration,
                                    min_length: None,
                                    max_length: None,
                                },
                            )),
                    } if enumeration.is_empty() => Ok(()),
                    _ => Err(Error::UnexpectedFormat(format!(
                        "invalid schema for application/octet-stream: {:?}",
                        schema
                    ))),
                }?;
                OperationParameterType::RawBody
            }
            BodyContentType::Text(_) => {
                // For a plain text body, we expect a simple, specific schema:
                // "schema": {
                //     "type": "string",
                // }
                match schema.item(components)? {
                    openapiv3::Schema {
                        schema_data:
                            openapiv3::SchemaData {
                                nullable: false,
                                discriminator: None,
                                default: None,
                                // Other fields that describe or document the
                                // schema are fine.
                                ..
                            },
                        schema_kind:
                            openapiv3::SchemaKind::Type(openapiv3::Type::String(
                                openapiv3::StringType {
                                    format: openapiv3::VariantOrUnknownOrEmpty::Empty,
                                    pattern: None,
                                    enumeration,
                                    min_length: None,
                                    max_length: None,
                                },
                            )),
                    } if enumeration.is_empty() => Ok(()),
                    _ => Err(Error::UnexpectedFormat(format!(
                        "invalid schema for {}: {:?}",
                        content_type, schema
                    ))),
                }?;
                OperationParameterType::RawBody
            }
            BodyContentType::FormUrlencoded => {
                return Err(Error::UnexpectedFormat(format!(
                    "form body of {} is not supported",
                    operation.operation_id.as_ref().unwrap(),
                )));
            }
            BodyContentType::Json => {
                if !media_type.encoding.is_empty() {
                    todo!("media type encoding not empty: {:#?}", media_type);
                }
                let name = sanitize(
                    &format!("{}-body", operation.operation_id.as_ref().unwrap(),),
                    Case::Pascal,
                );
                let typ = self
                    .type_space
                    .add_type_with_name(&schema.to_schema(), Some(name))?;
                OperationParameterType::Type(typ)
            }
        };

        Ok(Some(OperationParameter {
            name: "body".to_string(),
            api_name: "body".to_string(),
            description: body.description.clone(),
            typ,
            kind: OperationParameterKind::Body(content_type),
        }))
    }
}

fn make_doc_comment(method: &OperationMethod) -> String {
    let mut buf = String::new();

    if let Some(summary) = &method.summary {
        buf.push_str(summary.trim_end_matches(['.', ',']));
        buf.push_str("\n\n");
    }
    if let Some(description) = &method.description {
        buf.push_str(description);
        buf.push_str("\n\n");
    }

    buf.push_str(&format!(
        "Sends a `{}` request to `{}`\n\n",
        method.method.as_str().to_ascii_uppercase(),
        method.path,
    ));

    if method
        .params
        .iter()
        .filter(|param| param.description.is_some())
        .count()
        > 0
    {
        buf.push_str("Arguments:\n");
        for param in &method.params {
            buf.push_str(&format!("- `{}`", param.name));
            if let Some(description) = &param.description {
                buf.push_str(": ");
                buf.push_str(description);
            }
            buf.push('\n');
        }
    }

    buf
}

fn sort_params(raw_params: &mut [OperationParameter], names: &[String]) {
    raw_params.sort_by(
        |OperationParameter {
             kind: a_kind,
             api_name: a_name,
             ..
         },
         OperationParameter {
             kind: b_kind,
             api_name: b_name,
             ..
         }| {
            match (a_kind, b_kind) {
                // Path params are first and are in positional order.
                (OperationParameterKind::Path, OperationParameterKind::Path) => {
                    let a_index = names
                        .iter()
                        .position(|x| x == a_name)
                        .unwrap_or_else(|| panic!("{} missing from path", a_name));
                    let b_index = names
                        .iter()
                        .position(|x| x == b_name)
                        .unwrap_or_else(|| panic!("{} missing from path", b_name));
                    a_index.cmp(&b_index)
                }
                (OperationParameterKind::Path, OperationParameterKind::Query(_)) => Ordering::Less,
                (OperationParameterKind::Path, OperationParameterKind::Body(_)) => Ordering::Less,
                (OperationParameterKind::Path, OperationParameterKind::Header(_)) => Ordering::Less,

                // Query params are in lexicographic order.
                (OperationParameterKind::Query(_), OperationParameterKind::Body(_)) => {
                    Ordering::Less
                }
                (OperationParameterKind::Query(_), OperationParameterKind::Query(_)) => {
                    a_name.cmp(b_name)
                }
                (OperationParameterKind::Query(_), OperationParameterKind::Path) => {
                    Ordering::Greater
                }
                (OperationParameterKind::Query(_), OperationParameterKind::Header(_)) => {
                    Ordering::Less
                }

                // Body params are last and should be singular.
                (OperationParameterKind::Body(_), OperationParameterKind::Path) => {
                    Ordering::Greater
                }
                (OperationParameterKind::Body(_), OperationParameterKind::Query(_)) => {
                    Ordering::Greater
                }
                (OperationParameterKind::Body(_), OperationParameterKind::Header(_)) => {
                    Ordering::Greater
                }
                (OperationParameterKind::Body(_), OperationParameterKind::Body(_)) => {
                    panic!("should only be one body")
                }

                // Header params are in lexicographic order.
                (OperationParameterKind::Header(_), OperationParameterKind::Header(_)) => {
                    a_name.cmp(b_name)
                }
                (OperationParameterKind::Header(_), _) => Ordering::Greater,
            }
        },
    );
}

trait ParameterDataExt {
    fn schema(&self) -> Result<&openapiv3::ReferenceOr<openapiv3::Schema>>;
}

impl ParameterDataExt for openapiv3::ParameterData {
    fn schema(&self) -> Result<&openapiv3::ReferenceOr<openapiv3::Schema>> {
        match &self.format {
            openapiv3::ParameterSchemaOrContent::Schema(s) => Ok(s),
            openapiv3::ParameterSchemaOrContent::Content(c) => Err(Error::UnexpectedFormat(
                format!("unexpected content {:#?}", c),
            )),
        }
    }
}
