//! Low-level C ABI function signatures for dynamic plugins.
//!
//! This module defines the FFI contract between the host process and a plugin
//! shared library. Each type alias corresponds to a symbol the host resolves
//! via `dlsym` / `libloading`. Plugin authors rarely interact with these
//! directly — the [`dyn_aomi_app!`](crate::dyn_aomi_app) macro generates all
//! the required exports.
//!
//! Host integrators use [`DynFnHandle`](crate::DynFnHandle) which wraps these
//! raw function pointers in a safe interface.

use std::ffi::{c_char, c_void};

/// Opaque pointer to a plugin instance created by [`DynCreateFn`].
///
/// The host treats this as an opaque handle and passes it back to every
/// subsequent call. The plugin side casts it to its internal `__DynInstance`
/// struct.
pub type DynInstancePtr = *mut c_void;

/// `aomi_abi_version` — returns the plugin's ABI version number.
///
/// Called once immediately after `dlopen`. If the returned value does not
/// match [`DYN_ABI_VERSION`](crate::DYN_ABI_VERSION) the host refuses to
/// load the plugin.
pub type DynAbiVersionFn = unsafe extern "C" fn() -> u32;

/// `aomi_create` — allocates and returns a new plugin instance.
///
/// Called once after the ABI version check. The returned pointer is passed
/// to all other functions and must remain valid until [`DynDestroyFn`] is
/// called. Returns null on failure.
pub type DynCreateFn = unsafe extern "C" fn() -> DynInstancePtr;

/// `aomi_manifest` — returns the plugin manifest as a JSON C string.
///
/// Called once after creation. The returned pointer is a NUL-terminated
/// JSON string that deserializes into [`DynManifest`](crate::DynManifest).
/// The host **must** free the returned pointer via [`DynFreeStringFn`].
pub type DynManifestFn = unsafe extern "C" fn(DynInstancePtr) -> *mut c_char;

/// `aomi_async_tool_start` — starts execution of a tool.
///
/// # Parameters
///
/// 1. `instance` — plugin instance from [`DynCreateFn`]
/// 2. `name` — NUL-terminated tool name (e.g. `"get_price"`)
/// 3. `args_json` — NUL-terminated JSON string of tool arguments
/// 4. `ctx_json` — NUL-terminated JSON string deserializable as
///    [`DynToolCallCtx`](crate::DynToolCallCtx)
///
/// Returns a NUL-terminated JSON string deserializable as
/// [`DynToolStart`](crate::DynToolStart). For sync tools this contains the
/// result directly; for async tools it returns an `execution_id` used with
/// [`DynToolPollFn`] and [`DynToolCancelFn`].
///
/// The host **must** free the returned pointer via [`DynFreeStringFn`].
pub type DynToolStartFn = unsafe extern "C" fn(
    DynInstancePtr,
    *const c_char,
    *const c_char,
    *const c_char,
) -> *mut c_char;

/// `aomi_dyn_exec_poll` — polls an async execution for updates.
///
/// # Parameters
///
/// 1. `instance` — plugin instance
/// 2. `execution_id` — the `u64` id returned by [`DynToolStartFn`]
///
/// Returns a NUL-terminated JSON string deserializable as
/// [`AsyncExecPool`](crate::AsyncExecPool). The host **must** free the
/// returned pointer via [`DynFreeStringFn`].
pub type DynToolPollFn = unsafe extern "C" fn(DynInstancePtr, u64) -> *mut c_char;

/// `aomi_dyn_exec_cancel` — requests cancellation of an async execution.
///
/// # Parameters
///
/// 1. `instance` — plugin instance
/// 2. `execution_id` — the `u64` id to cancel
///
/// Returns a NUL-terminated JSON string deserializable as
/// [`DynExecCancel`](crate::DynExecCancel). The host **must** free the
/// returned pointer via [`DynFreeStringFn`].
pub type DynToolCancelFn = unsafe extern "C" fn(DynInstancePtr, u64) -> *mut c_char;

/// `aomi_destroy` — frees the plugin instance.
///
/// Called once when the host is done with the plugin. After this call the
/// instance pointer is invalid and must not be used.
pub type DynDestroyFn = unsafe extern "C" fn(DynInstancePtr);

/// `aomi_free_string` — frees a C string allocated by the plugin.
///
/// Every `*mut c_char` returned by [`DynManifestFn`], [`DynToolStartFn`],
/// [`DynToolPollFn`], or [`DynToolCancelFn`] **must** be freed by calling
/// this function. Passing null is safe (no-op).
pub type DynFreeStringFn = unsafe extern "C" fn(*mut c_char);

pub const AOMI_ABI_VERSION: &[u8] = b"aomi_abi_version\0";
pub const AOMI_CREATE: &[u8] = b"aomi_create\0";
pub const AOMI_MANIFEST: &[u8] = b"aomi_manifest\0";
pub const SYM_AOMI_ASYNC_TOOL_START: &[u8] = b"aomi_async_tool_start\0";
pub const AOMI_DYN_EXEC_POLL: &[u8] = b"aomi_dyn_exec_poll\0";
pub const AOMI_DYN_EXEC_CANCEL: &[u8] = b"aomi_dyn_exec_cancel\0";
pub const AOMI_DESTROY: &[u8] = b"aomi_destroy\0";
pub const AOMI_FREE_STRING: &[u8] = b"aomi_free_string\0";
