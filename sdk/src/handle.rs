//! Host-side dynamic library loader for Aomi plugins.

use std::ffi::{CStr, CString, c_char};
use std::path::Path;
use std::time::Duration;

use eyre::{Context, Result, bail};
use serde_json::Value;

use crate::{
    AOMI_ABI_VERSION, AOMI_CREATE, AOMI_DESTROY, AOMI_DYN_EXEC_CANCEL, AOMI_DYN_EXEC_POLL,
    AOMI_FREE_STRING, AOMI_MANIFEST, AsyncExecPool, DYN_ABI_VERSION, DynAbiVersionFn, DynCreateFn,
    DynDestroyFn, DynExecCancel, DynFreeStringFn, DynInstancePtr, DynManifest, DynManifestFn,
    DynToolCancelFn, DynToolPollFn, DynToolResult, DynToolStart, DynToolStartFn,
    SYM_AOMI_ASYNC_TOOL_START,
};

/// Handle to a loaded dynamic plugin library.
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

        let fn_abi_version: DynAbiVersionFn = unsafe {
            *library
                .get::<DynAbiVersionFn>(AOMI_ABI_VERSION)
                .context("symbol aomi_dyn_abi_version not found")?
        };
        let abi_version = unsafe { fn_abi_version() };
        if abi_version != DYN_ABI_VERSION {
            bail!(
                "ABI version mismatch: plugin={abi_version}, host={DYN_ABI_VERSION} ({})",
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

    /// Execute a tool and wait until terminal completion.
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
