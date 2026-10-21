# FFI Compatibility

Apps and hosts negotiate `ffi_version`, independently of the `aomi-sdk` package
version. SDK 6.0.0 introduces FFI version 1; the SDK major bump reflects added Rust
manifest fields, not a breaking change to the existing native interface.

## Loading contract

- `aomi_sdk_version()` remains diagnostic build metadata.
- `aomi_ffi_version() -> u32` is read before instance creation or tool dispatch.
- The plugin manifest must repeat that FFI version and its actual SDK version.
- Unknown FFI versions and unknown `required_interfaces` reject loading.
- SDK 5.1.1 and 5.1.2 binaries without the new export use a verified legacy table.
  Every other unversioned SDK is rejected; neither semver proximity nor missing
  metadata establishes compatibility.

FFI 1 covers all existing C signatures, opaque-instance ownership, plugin-owned
strings and their free function, JSON context/result/route envelopes, async
start/poll/cancel semantics, and destruction. Additive SDK helpers do not change
it. An incompatible change to any of those contracts must introduce a new FFI
version and an explicit host support decision. Supporting an interface never
changes runtime authorization or execution policy.

## Authoring and deployment

Keep reproducible SDK pins and a committed workspace lockfile. The CLI reads the
resolved SDK package's `package.metadata.aomi.ffi_version`; historical SDKs use
the same narrow legacy table. When the backend advertises `ffi_version`, its SDK
package version is informational. Old backends without that field retain their
exact SDK requirement during rollout. Build and activation validate the actual
artifact again; source metadata alone does not authorize loading.

Deploy the compatible host before plugins built with the new SDK. An old host
still rejects a different SDK. Rollback on a new host is permitted only when the
artifact's FFI contract remains supported, regardless of its SDK release number.

## Regression evidence

`cargo test -p aomi-sdk --test ffi_compatibility` builds frozen plugin source
against immutable SDK 5.1.1 and 5.1.2 revisions in separate Cargo graphs, then
loads those binaries in the candidate host. It exercises manifest, sync, async,
error, cancellation and cleanup behavior. Standalone probes test unrelated SDK
versions, unsupported FFI versions, missing/inconsistent declarations and unknown
required interfaces. The ordinary SDK FFI robustness suites remain required.

The loader is not a process isolation boundary: panic handling cannot recover
from memory corruption, invalid native pointers, process aborts or library load
constructors. Only trusted native plugins may be loaded in process.
