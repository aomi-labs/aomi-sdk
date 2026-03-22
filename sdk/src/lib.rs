//! # aomi-sdk
//!
//! SDK for building dynamic Aomi plugins (ABI v3).
//!
//! Plugin authors define:
//! - an app struct implementing `Default + Clone`
//! - tool structs implementing [`DynAomiTool`]
//! - one [`dyn_aomi_app!`] invocation to generate manifest/router/FFI exports.

mod abi;
mod ffi;
mod handle;
pub mod testing;
mod types;

pub use abi::*;
pub use handle::*;
pub use types::*;

// Re-export serde_json and schemars for convenience in plugin code.
pub use schemars;
pub use serde_json;

/// Internal helpers for macros. Do not use directly.
#[doc(hidden)]
pub mod __private {
    pub use crate::ffi::{free_c_string, parse_c_str, serialize_to_c_ptr, string_to_c_ptr};
    pub use crate::types::AsyncExecQueue;
}
