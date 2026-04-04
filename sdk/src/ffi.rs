//! C ABI contract and dyn macro helpers.

use std::{
    ffi::{CStr, CString, c_char},
    ptr::null_mut,
};

/// Convert a Rust String to a C string pointer.
#[doc(hidden)]
pub fn string_to_c_ptr(s: String) -> *mut c_char {
    CString::new(s).map_or(null_mut(), |cstr| cstr.into_raw())
}

/// Serialize any JSON-serializable envelope to a C string pointer.
#[doc(hidden)]
pub fn serialize_to_c_ptr<T: serde::Serialize>(value: &T) -> *mut c_char {
    match serde_json::to_string(value) {
        Ok(json) => string_to_c_ptr(json),
        Err(e) => {
            let fallback = crate::DynToolResult::Err(format!("failed to serialize envelope: {e}"));
            serde_json::to_string(&crate::DynToolStart::Ready { result: fallback })
                .map_or(null_mut(), string_to_c_ptr)
        }
    }
}

/// Parse a C string pointer to a Rust String.
///
/// # Safety
/// `ptr` must be a valid, non-null, NUL-terminated C string.
#[doc(hidden)]
pub unsafe fn parse_c_str(ptr: *const c_char, label: &str) -> Result<String, *mut c_char> {
    match unsafe { CStr::from_ptr(ptr) }.to_str() {
        Ok(s) => Ok(s.to_owned()),
        Err(_) => Err(serialize_to_c_ptr(&crate::DynToolStart::Ready {
            result: crate::DynToolResult::Err(format!("invalid UTF-8 in {label}")),
        })),
    }
}

/// Free a C string allocated by this crate.
///
/// # Safety
/// `ptr` must be a pointer returned by a function in this module, or null.
#[doc(hidden)]
pub unsafe fn free_c_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = unsafe { CString::from_raw(ptr) };
    }
}

// ── Tracing helpers for macro-generated code ────────────────────────────

/// Log a tool-start error (null pointer, UTF-8, panic).
#[doc(hidden)]
pub fn log_tool_start_error(tool: &str, error: &str) {
    tracing::error!(tool = tool, error = error, "tool start failed");
}

/// Log a sync tool execution error.
#[doc(hidden)]
pub fn log_tool_exec_error(tool: &str, error: &str) {
    tracing::error!(tool = tool, error = error, "tool execution failed");
}

/// Log an async tool failure.
#[doc(hidden)]
pub fn log_async_tool_error(tool: &str, error: &str) {
    tracing::error!(tool = tool, error = error, "async tool failed");
}

/// Log a poll-level error.
#[doc(hidden)]
pub fn log_poll_error(execution_id: u64, error: &str) {
    tracing::error!(execution_id = execution_id, error = error, "poll error");
}

/// Generate the C ABI entry points for a dynamic plugin app.
///
/// This macro is called automatically by `dyn_aomi_app!` — you typically
/// don't need to invoke it directly.
///
/// # Generated symbols
///
/// | Symbol                      | Purpose                           |
/// |-----------------------------|-----------------------------------|
/// | `aomi_abi_version`          | Returns [`DYN_ABI_VERSION`]       |
/// | `aomi_create`               | Allocates a new plugin instance   |
/// | `aomi_manifest`             | Serializes the plugin manifest    |
/// | `aomi_async_tool_start`     | Dispatches a tool call            |
/// | `aomi_dyn_exec_poll`        | Polls an async execution          |
/// | `aomi_dyn_exec_cancel`      | Cancels an async execution        |
/// | `aomi_destroy`              | Frees the plugin instance         |
/// | `aomi_free_string`          | Frees a returned C string         |
///
/// [`DYN_ABI_VERSION`]: crate::DYN_ABI_VERSION
#[macro_export]
macro_rules! declare_dyn {
    ($app_type:ty) => {
        #[doc(hidden)]
        struct __DynInstance {
            app: $app_type,
            next_execution_id: ::std::sync::atomic::AtomicU64,
            executions: ::std::sync::Mutex<
                ::std::collections::HashMap<
                    u64,
                    ::std::sync::Arc<$crate::__private::AsyncExecQueue>,
                >,
            >,
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn aomi_abi_version() -> u32 {
            $crate::DYN_ABI_VERSION
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn aomi_create() -> *mut ::std::ffi::c_void {
            let instance = __DynInstance {
                app: <$app_type>::default(),
                next_execution_id: ::std::sync::atomic::AtomicU64::new(1),
                executions: ::std::sync::Mutex::new(::std::collections::HashMap::new()),
            };
            ::std::boxed::Box::into_raw(::std::boxed::Box::new(instance)) as *mut ::std::ffi::c_void
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn aomi_manifest(
            ptr: *mut ::std::ffi::c_void,
        ) -> *mut ::std::ffi::c_char {
            if ptr.is_null() {
                return ::std::ptr::null_mut();
            }

            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let instance = unsafe { &*(ptr as *const __DynInstance) };
                <$app_type as $crate::DynAomiApp>::manifest(&instance.app)
            }));

            match result {
                Ok(manifest) => $crate::__private::serialize_to_c_ptr(&manifest),
                Err(_) => ::std::ptr::null_mut(),
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn aomi_async_tool_start(
            ptr: *mut ::std::ffi::c_void,
            name: *const ::std::ffi::c_char,
            args_json: *const ::std::ffi::c_char,
            ctx_json: *const ::std::ffi::c_char,
        ) -> *mut ::std::ffi::c_char {
            if ptr.is_null() || name.is_null() || args_json.is_null() || ctx_json.is_null() {
                let err = "null pointer passed to aomi_async_tool_start";
                $crate::__private::log_tool_start_error("<unknown>", err);
                return $crate::__private::serialize_to_c_ptr(&$crate::DynToolStart::Ready {
                    result: $crate::DynToolResult::Err(err.to_string()),
                });
            }

            let name_str = match unsafe { $crate::__private::parse_c_str(name, "tool name") } {
                Ok(s) => s,
                Err(err_ptr) => return err_ptr,
            };

            let args_str = match unsafe { $crate::__private::parse_c_str(args_json, "args_json") } {
                Ok(s) => s,
                Err(err_ptr) => return err_ptr,
            };

            let ctx_str = match unsafe { $crate::__private::parse_c_str(ctx_json, "ctx_json") } {
                Ok(s) => s,
                Err(err_ptr) => return err_ptr,
            };

            let instance = unsafe { &*(ptr as *const __DynInstance) };
            let queue = ::std::sync::Arc::new($crate::__private::AsyncExecQueue::default());
            let sink = $crate::DynAsyncSink::__from_queue(queue.clone());

            let start_result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                <$app_type as $crate::DynAomiApp>::start_tool(
                    &instance.app,
                    &name_str,
                    &args_str,
                    &ctx_str,
                    sink,
                )
            }));

            match start_result {
                Ok($crate::DynToolDispatch::Ready(result)) => {
                    $crate::__private::serialize_to_c_ptr(&$crate::DynToolStart::Ready { result })
                }
                Ok($crate::DynToolDispatch::AsyncQueued) => {
                    let execution_id = instance
                        .next_execution_id
                        .fetch_add(1, ::std::sync::atomic::Ordering::Relaxed);
                    if let Ok(mut executions) = instance.executions.lock() {
                        executions.insert(execution_id, queue);
                    }
                    $crate::__private::serialize_to_c_ptr(&$crate::DynToolStart::AsyncQueued {
                        execution_id,
                    })
                }
                Err(_) => {
                    let err = format!("plugin panicked during start of tool '{}'", name_str);
                    $crate::__private::log_tool_start_error(&name_str, &err);
                    $crate::__private::serialize_to_c_ptr(&$crate::DynToolStart::Ready {
                        result: $crate::DynToolResult::Err(err),
                    })
                }
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn aomi_dyn_exec_poll(
            ptr: *mut ::std::ffi::c_void,
            execution_id: u64,
        ) -> *mut ::std::ffi::c_char {
            if ptr.is_null() {
                let err = "null pointer passed to aomi_dyn_exec_poll";
                $crate::__private::log_poll_error(execution_id, err);
                return $crate::__private::serialize_to_c_ptr(&$crate::AsyncExecPool::Error {
                    message: err.to_string(),
                });
            }

            let instance = unsafe { &*(ptr as *const __DynInstance) };

            let queue = match instance.executions.lock() {
                Ok(executions) => executions.get(&execution_id).cloned(),
                Err(_) => None,
            };

            let Some(queue) = queue else {
                return $crate::__private::serialize_to_c_ptr(&$crate::AsyncExecPool::NotFound);
            };

            let poll = queue.poll();
            let terminal = matches!(
                poll,
                $crate::AsyncExecPool::Update {
                    has_more: false,
                    ..
                } | $crate::AsyncExecPool::Error { .. }
                    | $crate::AsyncExecPool::Canceled
            );

            if let $crate::AsyncExecPool::Error { ref message } = poll {
                $crate::__private::log_poll_error(execution_id, message);
            }

            if terminal {
                if let Ok(mut executions) = instance.executions.lock() {
                    executions.remove(&execution_id);
                }
            }

            $crate::__private::serialize_to_c_ptr(&poll)
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn aomi_dyn_exec_cancel(
            ptr: *mut ::std::ffi::c_void,
            execution_id: u64,
        ) -> *mut ::std::ffi::c_char {
            if ptr.is_null() {
                return $crate::__private::serialize_to_c_ptr(&$crate::DynExecCancel {
                    canceled: false,
                });
            }

            let instance = unsafe { &*(ptr as *const __DynInstance) };

            let canceled = if let Ok(executions) = instance.executions.lock() {
                if let Some(queue) = executions.get(&execution_id) {
                    queue.cancel();
                    true
                } else {
                    false
                }
            } else {
                false
            };

            $crate::__private::serialize_to_c_ptr(&$crate::DynExecCancel { canceled })
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn aomi_destroy(ptr: *mut ::std::ffi::c_void) {
            if !ptr.is_null() {
                let _ = unsafe { ::std::boxed::Box::from_raw(ptr as *mut __DynInstance) };
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn aomi_free_string(ptr: *mut ::std::ffi::c_char) {
            unsafe { $crate::__private::free_c_string(ptr) };
        }
    };
}

/// Define a dynamic app and compile tool list into manifest, router, and FFI exports.
///
/// This is the main entry point for plugin authors. A single invocation at the
/// crate root generates:
///
/// - A [`DynAomiApp`](crate::DynAomiApp) impl for your app struct (manifest,
///   tool descriptors, dispatch router)
/// - All C ABI entry points via [`declare_dyn!`] (see that macro for the full
///   symbol table)
///
/// # Forms
///
/// **Basic** (tools only, using the recommended explicit common namespace):
/// ```rust,ignore
/// dyn_aomi_app!(app = MyApp, name = "my", version = "0.1.0",
///     preamble = "...", tools = [ToolA, ToolB], namespaces = ["common"]);
/// ```
///
/// **With host-side namespaces** (tools can be empty for namespace-only apps):
/// ```rust,ignore
/// dyn_aomi_app!(app = MyApp, name = "my", version = "0.1.0",
///     preamble = "...", tools = [], namespaces = ["database"]);
/// ```
///
/// **With explicit no host namespaces**:
/// ```rust,ignore
/// dyn_aomi_app!(app = MyApp, name = "my", version = "0.1.0",
///     preamble = "...", tools = [ToolA], namespaces = []);
/// ```
#[macro_export]
macro_rules! dyn_aomi_app {
    // ── With namespaces ──────────────────────────────────────────────────
    (
        app = $app_type:ty,
        name = $name:expr,
        version = $version:expr,
        preamble = $preamble:expr,
        tools = [ $( $tool_type:ty ),* $(,)? ],
        namespaces = [ $( $ns:expr ),* $(,)? ]
    ) => {
        impl $crate::DynAomiApp for $app_type {
            fn name(&self) -> &'static str { $name }
            fn version(&self) -> &'static str { $version }
            fn preamble(&self) -> &'static str { $preamble }

            fn tools(&self) -> ::std::vec::Vec<$crate::DynToolMetadata> {
                ::std::vec![ $( <$tool_type as $crate::DynAomiTool>::descriptor(self) ),* ]
            }

            fn namespaces(&self) -> ::std::option::Option<::std::vec::Vec<::std::string::String>> {
                ::std::option::Option::Some(::std::vec![ $( $ns.to_string() ),* ])
            }

            fn start_tool(
                &self,
                name: &str,
                args_json: &str,
                ctx_json: &str,
                sink: $crate::DynAsyncSink,
            ) -> $crate::DynToolDispatch {
                $crate::__dispatch_tool!(self, name, args_json, ctx_json, sink, [ $( $tool_type ),* ])
            }
        }

        $crate::declare_dyn!($app_type);
    };

    // ── Without namespaces (backward compatible) ─────────────────────────
    (
        app = $app_type:ty,
        name = $name:expr,
        version = $version:expr,
        preamble = $preamble:expr,
        tools = [ $( $tool_type:ty ),* $(,)? ]
    ) => {
        impl $crate::DynAomiApp for $app_type {
            fn name(&self) -> &'static str { $name }
            fn version(&self) -> &'static str { $version }
            fn preamble(&self) -> &'static str { $preamble }

            fn tools(&self) -> ::std::vec::Vec<$crate::DynToolMetadata> {
                ::std::vec![ $( <$tool_type as $crate::DynAomiTool>::descriptor(self) ),* ]
            }

            fn start_tool(
                &self,
                name: &str,
                args_json: &str,
                ctx_json: &str,
                sink: $crate::DynAsyncSink,
            ) -> $crate::DynToolDispatch {
                $crate::__dispatch_tool!(self, name, args_json, ctx_json, sink, [ $( $tool_type ),* ])
            }
        }

        $crate::declare_dyn!($app_type);
    };
}

/// Internal helper: generates the `match name { ... }` dispatch for tool routing.
#[doc(hidden)]
#[macro_export]
macro_rules! __dispatch_tool {
    ($self:ident, $name:ident, $args_json:ident, $ctx_json:ident, $sink:ident,
     [ $( $tool_type:ty ),* ]) => {
        match $name {
            $(
                <$tool_type as $crate::DynAomiTool>::NAME => {
                    let args = match $crate::parse_dyn_args::<<$tool_type as $crate::DynAomiTool>::Args>($args_json) {
                        Ok(args) => args,
                        Err(ref err) => {
                            $crate::__private::log_tool_exec_error($name, err);
                            return $crate::DynToolDispatch::Ready($crate::DynToolResult::err(err));
                        }
                    };

                    let ctx = match $crate::parse_dyn_ctx($ctx_json) {
                        Ok(ctx) => ctx,
                        Err(ref err) => {
                            $crate::__private::log_tool_exec_error($name, err);
                            return $crate::DynToolDispatch::Ready($crate::DynToolResult::err(err));
                        }
                    };

                    if <$tool_type as $crate::DynAomiTool>::IS_ASYNC {
                        let tool_name = $name.to_string();
                        let app_clone = $self.clone();
                        let sink_clone = $sink.clone();
                        ::std::thread::spawn(move || {
                            let result = <$tool_type as $crate::DynAomiTool>::run_async(
                                &app_clone, args, ctx, sink_clone.clone(),
                            );
                            if let Err(ref err) = result {
                                $crate::__private::log_async_tool_error(&tool_name, err);
                                sink_clone.fail(err);
                            }
                        });
                        $crate::DynToolDispatch::AsyncQueued
                    } else {
                        match <$tool_type as $crate::DynAomiTool>::run($self, args, ctx) {
                            Ok(value) => $crate::DynToolDispatch::Ready($crate::DynToolResult::ok(value)),
                            Err(ref err) => {
                                $crate::__private::log_tool_exec_error($name, err);
                                $crate::DynToolDispatch::Ready($crate::DynToolResult::err(err))
                            }
                        }
                    }
                }
            )*
            _ => {
                let err = format!("unknown tool: {}", $name);
                $crate::__private::log_tool_exec_error($name, &err);
                $crate::DynToolDispatch::Ready($crate::DynToolResult::err(err))
            }
        }
    };
}
