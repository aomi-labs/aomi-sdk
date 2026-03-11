/// Internal helper for `dyn_runtime!` schema handling.
#[doc(hidden)]
#[macro_export]
macro_rules! __dyn_tool_schema {
    (derive, $args_ty:ty) => {
        $crate::serde_json::to_value(::schemars::schema_for!($args_ty))
            .expect("derived tool schema should serialize")
    };
    ($schema:expr, $args_ty:ty) => {
        ($schema)()
    };
}

/// Internal helper for `dyn_runtime!` tool descriptor generation.
#[doc(hidden)]
#[macro_export]
macro_rules! __dyn_tool_descriptor {
    (
        $namespace:expr,
        {
            name: $tool_name:expr,
            description: $description:expr,
            schema: derive,
            args: $args_ty:ty,
            ctx: $ctx_ty:ty,
            handler: $handler:expr,
            is_async: $is_async:expr $(,)?
        }
    ) => {
        $crate::DynToolDescriptor {
            name: ($tool_name).into(),
            namespace: $namespace.clone(),
            description: ($description).into(),
            parameters_schema: $crate::__dyn_tool_schema!(derive, $args_ty),
            is_async: $is_async,
        }
    };
    (
        $namespace:expr,
        {
            name: $tool_name:expr,
            description: $description:expr,
            schema: $schema:expr,
            args: $args_ty:ty,
            ctx: $ctx_ty:ty,
            handler: $handler:expr,
            is_async: $is_async:expr $(,)?
        }
    ) => {
        $crate::DynToolDescriptor {
            name: ($tool_name).into(),
            namespace: $namespace.clone(),
            description: ($description).into(),
            parameters_schema: $crate::__dyn_tool_schema!($schema, $args_ty),
            is_async: $is_async,
        }
    };
}

/// Internal helper for `dyn_runtime!` typed dispatch generation.
#[doc(hidden)]
#[macro_export]
macro_rules! __dyn_tool_try_dispatch {
    (
        $runtime_type:ty,
        $self:expr,
        $tool_name:expr,
        $args_json:expr,
        $ctx_json:expr,
        {
            name: $name:expr,
            description: $description:expr,
            schema: derive,
            args: $args_ty:ty,
            ctx: $ctx_ty:ty,
            handler: $handler:expr,
            is_async: $is_async:expr $(,)?
        }
    ) => {
        if $tool_name == $name {
            let handler: fn(&$runtime_type, $args_ty, $ctx_ty) -> $crate::DynToolResult = $handler;
            return $crate::execute_typed_tool($self, $args_json, $ctx_json, handler);
        }
    };
    (
        $runtime_type:ty,
        $self:expr,
        $tool_name:expr,
        $args_json:expr,
        $ctx_json:expr,
        {
            name: $name:expr,
            description: $description:expr,
            schema: $schema:expr,
            args: $args_ty:ty,
            ctx: $ctx_ty:ty,
            handler: $handler:expr,
            is_async: $is_async:expr $(,)?
        }
    ) => {
        if $tool_name == $name {
            let handler: fn(&$runtime_type, $args_ty, $ctx_ty) -> $crate::DynToolResult = $handler;
            return $crate::execute_typed_tool($self, $args_json, $ctx_json, handler);
        }
    };
}

/// Generate a [`DynRuntime`] implementation from manifest metadata plus a typed tool table.
///
/// Each tool entry defines:
/// - `name`: the runtime-dispatch name
/// - `description`: human-readable tool description
/// - `schema`: either `derive` or a zero-arg function/closure returning the JSON Schema
/// - `args`: the deserialized argument type
/// - `ctx`: the deserialized context type
/// - `handler`: a typed runtime method/function
/// - `is_async`: whether the tool supports async/streaming execution
#[macro_export]
macro_rules! dyn_runtime {
    (
        impl DynRuntime for $runtime_type:ty {
            manifest {
                name: $name:expr,
                version: $version:expr,
                preamble: $preamble:expr,
                model_preference: $model_preference:expr $(,)?
            }
            tools [
                $( { $($tool:tt)* } ),* $(,)?
            ]
        }
    ) => {
        impl $crate::DynRuntime for $runtime_type {
            fn manifest(&self) -> $crate::DynManifest {
                let name: ::std::string::String = ($name).into();
                let version: ::std::string::String = ($version).into();
                let preamble: ::std::string::String = ($preamble).into();

                $crate::DynManifest {
                    abi_version: $crate::DYN_ABI_VERSION,
                    name: name.clone(),
                    version,
                    preamble,
                    model_preference: $model_preference,
                    tools: vec![
                        $(
                            $crate::__dyn_tool_descriptor!(name, { $($tool)* })
                        ),*
                    ],
                }
            }

            fn execute_tool(&self, name: &str, args_json: &str, ctx_json: &str) -> $crate::DynResult {
                $(
                    $crate::__dyn_tool_try_dispatch!(
                        $runtime_type,
                        self,
                        name,
                        args_json,
                        ctx_json,
                        { $($tool)* }
                    );
                )*
                $crate::DynResult::err(format!("unknown tool: {name}"))
            }
        }
    };
}
