//! C ABI contract and the `declare_dyn!` macro.
//!
//! A dynamic plugin (`.so`/`.dylib`) must export these six symbols:
//!
//! | Symbol                   | Signature                                                                   | Purpose                                  |
//! |--------------------------|-----------------------------------------------------------------------------|------------------------------------------|
//! | `aomi_dyn_abi_version`   | `extern "C" fn() -> u32`                                                    | ABI version check                        |
//! | `aomi_dyn_create`        | `extern "C" fn() -> *mut c_void`                                            | Instantiate the plugin runtime           |
//! | `aomi_dyn_manifest`      | `extern "C" fn(*mut c_void) -> *mut c_char`                                 | Get manifest as JSON (caller frees)      |
//! | `aomi_dyn_exec_tool`     | `extern "C" fn(*mut c_void, *const c_char, *const c_char, *const c_char) -> *mut c_char` | Execute a tool (returns JSON DynResult)  |
//! | `aomi_dyn_destroy`       | `extern "C" fn(*mut c_void)`                                                | Destroy the plugin runtime               |
//! | `aomi_dyn_free_string`   | `extern "C" fn(*mut c_char)`                                                | Free a string allocated by the plugin    |
//!
//! Plugin authors should NOT implement these manually. Instead, use the [`declare_dyn!`] macro.

use std::{
    ffi::{CStr, CString, c_char},
    ptr::null_mut,
};

// ============================================================================
// Helper functions for the macro (not part of public API)
// ============================================================================

/// Convert a Rust String to a C string pointer.
#[doc(hidden)]
pub fn string_to_c_ptr(s: String) -> *mut c_char {
    CString::new(s).map_or(null_mut(), |cstr| cstr.into_raw())
}

/// Serialize a DynResult to a C string pointer.
#[doc(hidden)]
pub fn serialize_dyn_result(result: &crate::DynResult) -> *mut c_char {
    match serde_json::to_string(result) {
        Ok(json) => string_to_c_ptr(json),
        Err(e) => {
            let fallback = crate::DynResult::Err(format!("failed to serialize result: {e}"));
            serde_json::to_string(&fallback).map_or(null_mut(), string_to_c_ptr)
        }
    }
}

/// Create an error DynResult as a C string pointer.
#[doc(hidden)]
pub fn dyn_error_ptr(msg: String) -> *mut c_char {
    serialize_dyn_result(&crate::DynResult::Err(msg))
}

/// Parse a C string pointer to a Rust String.
///
/// # Safety
/// `ptr` must be a valid, non-null, NUL-terminated C string.
#[doc(hidden)]
pub unsafe fn parse_c_str(ptr: *const c_char, label: &str) -> Result<String, *mut c_char> {
    match unsafe { CStr::from_ptr(ptr) }.to_str() {
        Ok(s) => Ok(s.to_owned()),
        Err(_) => Err(dyn_error_ptr(format!("invalid UTF-8 in {label}"))),
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

/// Generate the six C ABI entry points for a dynamic plugin.
///
/// The type must implement [`DynRuntime`](crate::DynRuntime) and [`Default`].
///
/// # Usage
///
/// ```rust,ignore
/// use aomi_dyn_sdk::*;
///
/// #[derive(Default)]
/// struct MyPlugin { /* ... */ }
///
/// impl DynRuntime for MyPlugin {
///     fn manifest(&self) -> DynManifest { /* ... */ }
///     fn execute_tool(&self, name: &str, args_json: &str, ctx_json: &str) -> DynResult { /* ... */ }
/// }
///
/// declare_dyn!(MyPlugin);
/// ```
///
/// # Safety
///
/// The generated functions use `unsafe` for FFI pointer operations.
/// The plugin author must ensure that `DynRuntime::execute_tool` is thread-safe
/// (which is enforced by the `Send + Sync` bounds on the trait).
#[macro_export]
macro_rules! declare_dyn {
    ($runtime_type:ty) => {
        /// Returns the ABI version this plugin was compiled with.
        #[unsafe(no_mangle)]
        pub extern "C" fn aomi_dyn_abi_version() -> u32 {
            $crate::DYN_ABI_VERSION
        }

        /// Create a new plugin runtime instance.
        ///
        /// Returns an opaque pointer that must be passed to all other functions
        /// and eventually freed with `aomi_dyn_destroy`.
        #[unsafe(no_mangle)]
        pub extern "C" fn aomi_dyn_create() -> *mut ::std::ffi::c_void {
            let runtime: Box<$runtime_type> = Box::new(<$runtime_type>::default());
            Box::into_raw(runtime) as *mut ::std::ffi::c_void
        }

        /// Get the plugin manifest as a JSON string.
        ///
        /// Returns a pointer to a NUL-terminated UTF-8 string, or null on error.
        /// The caller must free this string using `aomi_dyn_free_string`.
        ///
        /// # Safety
        ///
        /// `ptr` must be a valid pointer returned by `aomi_dyn_create`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn aomi_dyn_manifest(
            ptr: *mut ::std::ffi::c_void,
        ) -> *mut ::std::ffi::c_char {
            if ptr.is_null() {
                return ::std::ptr::null_mut();
            }
            let runtime = unsafe { &*(ptr as *const $runtime_type) };
            let manifest = <$runtime_type as $crate::DynRuntime>::manifest(runtime);
            match $crate::serde_json::to_string(&manifest) {
                Ok(json) => $crate::__private::string_to_c_ptr(json),
                Err(_) => ::std::ptr::null_mut(),
            }
        }

        /// Execute a tool by name.
        ///
        /// All string parameters are NUL-terminated UTF-8.
        /// Returns a JSON string of [`DynResult`] that must be freed with `aomi_dyn_free_string`.
        ///
        /// # Safety
        ///
        /// - `ptr` must be a valid pointer returned by `aomi_dyn_create`.
        /// - `name`, `args_json`, `ctx_json` must be valid NUL-terminated UTF-8 strings.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn aomi_dyn_exec_tool(
            ptr: *mut ::std::ffi::c_void,
            name: *const ::std::ffi::c_char,
            args_json: *const ::std::ffi::c_char,
            ctx_json: *const ::std::ffi::c_char,
        ) -> *mut ::std::ffi::c_char {
            if ptr.is_null() || name.is_null() || args_json.is_null() || ctx_json.is_null() {
                return $crate::__private::dyn_error_ptr(
                    "null pointer passed to aomi_dyn_exec_tool".into(),
                );
            }

            let runtime = unsafe { &*(ptr as *const $runtime_type) };

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

            // Catch panics to prevent unwinding across FFI boundary
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                <$runtime_type as $crate::DynRuntime>::execute_tool(
                    runtime, &name_str, &args_str, &ctx_str,
                )
            }));

            let dyn_result = match result {
                Ok(r) => r,
                Err(_) => $crate::DynResult::Err(format!(
                    "plugin panicked during execution of tool '{}'",
                    name_str
                )),
            };

            $crate::__private::serialize_dyn_result(&dyn_result)
        }

        /// Destroy a plugin runtime instance.
        ///
        /// # Safety
        ///
        /// `ptr` must be a valid pointer returned by `aomi_dyn_create`,
        /// and must not be used after this call.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn aomi_dyn_destroy(ptr: *mut ::std::ffi::c_void) {
            if !ptr.is_null() {
                let _ = unsafe { Box::from_raw(ptr as *mut $runtime_type) };
            }
        }

        /// Free a string allocated by the plugin.
        ///
        /// Must be called on every non-null string returned by
        /// `aomi_dyn_manifest` or `aomi_dyn_exec_tool`.
        ///
        /// # Safety
        ///
        /// `ptr` must be a pointer returned by a plugin function, or null.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn aomi_dyn_free_string(ptr: *mut ::std::ffi::c_char) {
            unsafe { $crate::__private::free_c_string(ptr) };
        }
    };
}
