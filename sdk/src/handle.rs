//! Host-side dynamic library loader for Aomi plugins.
//!
//! This module is for **host integrators** who load plugin `.so`/`.dylib`
//! files at runtime. The main entry point is [`DynFnHandle`].
//!
//! # Usage
//!
//! ```rust,ignore
//! use aomi_sdk::DynFnHandle;
//! use std::path::Path;
//!
//! // Load the plugin (validates SDK version and resolves symbols)
//! let handle = unsafe { DynFnHandle::load(Path::new("./libmy_plugin.so"))? };
//!
//! // Read the manifest to discover tools
//! let manifest = handle.call_manifest()?;
//! println!("Plugin '{}' exposes {} tools", manifest.name, manifest.tools.len());
//!
//! // Execute a tool (blocks until completion, 300s timeout)
//! let result = handle.call_exec_tool("greet", r#"{"name":"world"}"#, "{}")?;
//! println!("Result: {result}");
//! ```

use std::ffi::{CStr, CString, c_char};
use std::path::Path;
use std::time::Duration;

use eyre::{Context, Result, bail};
use serde_json::Value;

use crate::{
    AOMI_CREATE, AOMI_DESTROY, AOMI_DYN_EXEC_CANCEL, AOMI_DYN_EXEC_POLL, AOMI_FREE_STRING,
    AOMI_MANIFEST, AOMI_SDK_VERSION, AsyncExecPool, DynCreateFn, DynDestroyFn, DynExecCancel,
    DynFreeStringFn, DynInstancePtr, DynManifest, DynManifestFn, DynSdkVersionFn, DynToolCancelFn,
    DynToolPollFn, DynToolResult, DynToolStart, DynToolStartFn, SYM_AOMI_ASYNC_TOOL_START,
    SYM_AOMI_SDK_VERSION,
};

/// Handle to a loaded dynamic plugin library.
///
/// Wraps a `libloading::Library` and the resolved C ABI function pointers.
/// Manages the plugin instance lifecycle: the instance is created on
/// [`load`](Self::load) and destroyed when the handle is dropped.
///
/// # Safety
///
/// The loaded library must be a valid aomi-sdk plugin compiled against the
/// same SDK version. The handle is `Send + Sync` because plugin
/// instances are designed to be thread-safe (the generated code uses
/// `Mutex` for shared state).
pub struct DynFnHandle {
    instance: DynInstancePtr,
    fn_manifest: DynManifestFn,
    fn_tool_start: DynToolStartFn,
    fn_tool_poll: DynToolPollFn,
    fn_tool_cancel: DynToolCancelFn,
    fn_destroy: DynDestroyFn,
    fn_free_string: DynFreeStringFn,
    _library: libloading::Library,
}

unsafe impl Send for DynFnHandle {}
unsafe impl Sync for DynFnHandle {}

impl DynFnHandle {
    /// Load a plugin from a shared library file.
    ///
    /// # Safety
    /// The caller must ensure the library at `path` is a valid aomi-sdk plugin.
    pub unsafe fn load(path: &Path) -> Result<Self> {
        let library = unsafe {
            libloading::Library::new(path)
                .with_context(|| format!("failed to dlopen {}", path.display()))?
        };

        let fn_sdk_version: DynSdkVersionFn = unsafe {
            *library
                .get::<DynSdkVersionFn>(SYM_AOMI_SDK_VERSION)
                .context("symbol aomi_sdk_version not found")?
        };
        let plugin_sdk_version =
            Self::read_static_c_string(unsafe { fn_sdk_version() }, "aomi_sdk_version")?;
        if plugin_sdk_version != AOMI_SDK_VERSION {
            bail!(
                "SDK version mismatch: plugin={plugin_sdk_version}, host={AOMI_SDK_VERSION} ({})",
                path.display()
            );
        }

        let fn_create: DynCreateFn = unsafe {
            *library
                .get::<DynCreateFn>(AOMI_CREATE)
                .context("symbol aomi_dyn_create not found")?
        };
        let fn_manifest: DynManifestFn = unsafe {
            *library
                .get::<DynManifestFn>(AOMI_MANIFEST)
                .context("symbol aomi_dyn_manifest not found")?
        };
        let fn_tool_start: DynToolStartFn = unsafe {
            *library
                .get::<DynToolStartFn>(SYM_AOMI_ASYNC_TOOL_START)
                .context("symbol aomi_async_tool_start not found")?
        };
        let fn_tool_poll: DynToolPollFn = unsafe {
            *library
                .get::<DynToolPollFn>(AOMI_DYN_EXEC_POLL)
                .context("symbol aomi_dyn_exec_poll not found")?
        };
        let fn_tool_cancel: DynToolCancelFn = unsafe {
            *library
                .get::<DynToolCancelFn>(AOMI_DYN_EXEC_CANCEL)
                .context("symbol aomi_dyn_exec_cancel not found")?
        };
        let fn_destroy: DynDestroyFn = unsafe {
            *library
                .get::<DynDestroyFn>(AOMI_DESTROY)
                .context("symbol aomi_dyn_destroy not found")?
        };
        let fn_free_string: DynFreeStringFn = unsafe {
            *library
                .get::<DynFreeStringFn>(AOMI_FREE_STRING)
                .context("symbol aomi_dyn_free_string not found")?
        };

        let instance = unsafe { fn_create() };
        if instance.is_null() {
            bail!("aomi_create returned null ({})", path.display());
        }

        Ok(Self {
            instance,
            fn_manifest,
            fn_tool_start,
            fn_tool_poll,
            fn_tool_cancel,
            fn_destroy,
            fn_free_string,
            _library: library,
        })
    }

    fn read_c_string(&self, raw: *mut c_char, label: &str) -> Result<String> {
        if raw.is_null() {
            bail!("{label} returned null");
        }

        // Copy bytes first, then free plugin allocation regardless of UTF-8 validity.
        let bytes = unsafe { CStr::from_ptr(raw).to_bytes().to_vec() };
        unsafe { (self.fn_free_string)(raw) };

        String::from_utf8(bytes).with_context(|| format!("{label} is not valid UTF-8"))
    }

    fn read_static_c_string(raw: *const c_char, label: &str) -> Result<String> {
        if raw.is_null() {
            bail!("{label} returned null");
        }

        let bytes = unsafe { CStr::from_ptr(raw).to_bytes().to_vec() };
        String::from_utf8(bytes).with_context(|| format!("{label} is not valid UTF-8"))
    }

    /// Read and parse the plugin manifest.
    pub fn call_manifest(&self) -> Result<DynManifest> {
        let raw = unsafe { (self.fn_manifest)(self.instance) };
        let json_str = self.read_c_string(raw, "aomi_manifest")?;
        serde_json::from_str(&json_str).context("failed to parse manifest JSON")
    }

    /// Start a tool execution.
    pub fn call_tool_start(
        &self,
        name: &str,
        args_json: &str,
        ctx_json: &str,
    ) -> Result<DynToolStart> {
        let c_name = CString::new(name).context("tool name contains null byte")?;
        let c_args = CString::new(args_json).context("args_json contains null byte")?;
        let c_ctx = CString::new(ctx_json).context("ctx_json contains null byte")?;

        let raw = unsafe {
            (self.fn_tool_start)(
                self.instance,
                c_name.as_ptr(),
                c_args.as_ptr(),
                c_ctx.as_ptr(),
            )
        };
        let json_str = self.read_c_string(raw, "aomi_async_tool_start")?;

        serde_json::from_str(&json_str).context("failed to parse DynToolStart JSON")
    }

    /// Poll an async execution.
    pub fn call_tool_poll(&self, execution_id: u64) -> Result<AsyncExecPool> {
        let raw = unsafe { (self.fn_tool_poll)(self.instance, execution_id) };
        let json_str = self.read_c_string(raw, "aomi_dyn_exec_poll")?;
        serde_json::from_str(&json_str).context("failed to parse AsyncExecPool JSON")
    }

    /// Cancel an async execution.
    pub fn call_tool_cancel(&self, execution_id: u64) -> Result<DynExecCancel> {
        let raw = unsafe { (self.fn_tool_cancel)(self.instance, execution_id) };
        let json_str = self.read_c_string(raw, "aomi_dyn_exec_cancel")?;
        serde_json::from_str(&json_str).context("failed to parse DynExecCancel JSON")
    }

    /// Execute a tool synchronously, blocking until terminal completion.
    ///
    /// This is a convenience wrapper around [`call_tool_start`](Self::call_tool_start)
    /// and [`call_tool_poll`](Self::call_tool_poll). For sync tools the result is
    /// returned immediately. For async tools it polls in a loop (25 ms interval)
    /// with a **300-second timeout** — if the tool doesn't complete in time, it is
    /// automatically canceled and an error is returned.
    pub fn call_exec_tool(&self, name: &str, args_json: &str, ctx_json: &str) -> Result<Value> {
        match self.call_tool_start(name, args_json, ctx_json)? {
            DynToolStart::Ready { result } => match result {
                DynToolResult::Ok(value) => Ok(value),
                DynToolResult::Err(msg) => bail!("plugin tool error: {msg}"),
            },
            DynToolStart::AsyncQueued { execution_id } => {
                let deadline = std::time::Instant::now() + Duration::from_secs(300);
                loop {
                    if std::time::Instant::now() >= deadline {
                        let _ = self.call_tool_cancel(execution_id);
                        bail!(
                            "plugin async tool timed out after 300s (execution_id={execution_id})"
                        );
                    }
                    match self.call_tool_poll(execution_id)? {
                        AsyncExecPool::Pending => std::thread::sleep(Duration::from_millis(25)),
                        AsyncExecPool::Update { value, has_more } => {
                            if !has_more {
                                return Ok(value);
                            }
                        }
                        AsyncExecPool::Error { message } => {
                            bail!("plugin async tool error: {message}")
                        }
                        AsyncExecPool::Canceled => {
                            bail!("plugin async tool execution was canceled")
                        }
                        AsyncExecPool::NotFound => {
                            bail!("plugin async execution disappeared before completion")
                        }
                    }
                }
            }
        }
    }
}

impl Drop for DynFnHandle {
    fn drop(&mut self) {
        if !self.instance.is_null() {
            unsafe { (self.fn_destroy)(self.instance) };
            self.instance = std::ptr::null_mut();
        }
    }
}
