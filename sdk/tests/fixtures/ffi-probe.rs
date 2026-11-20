//! Standalone probe: intentionally has no dependency on the candidate SDK.
use std::ffi::{c_char, c_void, CString};

#[unsafe(no_mangle)]
pub extern "C" fn aomi_sdk_version() -> *const c_char {
    // An unsupported FFI must be rejected before querying SDK diagnostics.
    if __FFI__ != 1 { return std::ptr::null(); }
    b"__SDK__\0".as_ptr().cast()
}

#[unsafe(no_mangle)]
pub extern "C" fn aomi_ffi_version() -> u32 { __FFI__ }

#[unsafe(no_mangle)]
pub extern "C" fn aomi_create() -> *mut c_void {
    Box::into_raw(Box::new(0_u8)).cast()
}

#[unsafe(no_mangle)]
pub extern "C" fn aomi_manifest(_: *mut c_void) -> *mut c_char {
    CString::new(r#"__MANIFEST__"#).unwrap().into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn aomi_async_tool_start(_: *mut c_void, _: *const c_char, _: *const c_char, _: *const c_char) -> *mut c_char { std::ptr::null_mut() }

#[unsafe(no_mangle)]
pub extern "C" fn aomi_dyn_exec_poll(_: *mut c_void, _: u64) -> *mut c_char { std::ptr::null_mut() }

#[unsafe(no_mangle)]
pub extern "C" fn aomi_dyn_exec_cancel(_: *mut c_void, _: u64) -> *mut c_char { std::ptr::null_mut() }

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aomi_destroy(ptr: *mut c_void) {
    if !ptr.is_null() { drop(unsafe { Box::from_raw(ptr.cast::<u8>()) }); }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aomi_free_string(ptr: *mut c_char) {
    if !ptr.is_null() { drop(unsafe { CString::from_raw(ptr) }); }
}
