//! Shared C ABI symbols and function signatures for dynamic plugins.

use std::ffi::{c_char, c_void};

/// Opaque pointer returned by `aomi_dyn_create`.
pub type DynInstancePtr = *mut c_void;

/// `aomi_dyn_abi_version`
pub type DynAbiVersionFn = unsafe extern "C" fn() -> u32;
/// `aomi_dyn_create`
pub type DynCreateFn = unsafe extern "C" fn() -> DynInstancePtr;
/// `aomi_dyn_manifest`
pub type DynManifestFn = unsafe extern "C" fn(DynInstancePtr) -> *mut c_char;
/// `aomi_async_tool_start`
pub type DynToolStartFn = unsafe extern "C" fn(
    DynInstancePtr,
    *const c_char,
    *const c_char,
    *const c_char,
) -> *mut c_char;
/// `aomi_dyn_exec_poll`
pub type DynToolPollFn = unsafe extern "C" fn(DynInstancePtr, u64) -> *mut c_char;
/// `aomi_dyn_exec_cancel`
pub type DynToolCancelFn = unsafe extern "C" fn(DynInstancePtr, u64) -> *mut c_char;
/// `aomi_dyn_destroy`
pub type DynDestroyFn = unsafe extern "C" fn(DynInstancePtr);
/// `aomi_dyn_free_string`
pub type DynFreeStringFn = unsafe extern "C" fn(*mut c_char);

pub const AOMI_ABI_VERSION: &[u8] = b"aomi_abi_version\0";
pub const AOMI_CREATE: &[u8] = b"aomi_create\0";
pub const AOMI_MANIFEST: &[u8] = b"aomi_manifest\0";
pub const SYM_AOMI_ASYNC_TOOL_START: &[u8] = b"aomi_async_tool_start\0";
pub const AOMI_DYN_EXEC_POLL: &[u8] = b"aomi_dyn_exec_poll\0";
pub const AOMI_DYN_EXEC_CANCEL: &[u8] = b"aomi_dyn_exec_cancel\0";
pub const AOMI_DESTROY: &[u8] = b"aomi_destroy\0";
pub const AOMI_FREE_STRING: &[u8] = b"aomi_free_string\0";
