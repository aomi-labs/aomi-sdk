# Resource interface migration to SDK 6.0.0

This branch starts from `origin/publish` at
`62a1442879993c4421e4b00fe08c0f6fded1a973`. It is a major source interface
migration; no package publication is part of this change.

`DynToolMetadata` adds optional serialized `resource_output` declarations.
Previously serialized metadata without this field still deserializes and emits
no declaration. Rust consumers constructing this public struct with a literal
must explicitly add `resource_output: None` (or a declaration). Existing literal
source is therefore not source compatible. Tools using the established
`DynAomiTool::descriptor` path inherit the default absent declaration.

Declarations select whole raw values with fixed JSON Pointers. They do not
supply URIs, authenticated scope, provenance, or executable authority. The host
validates executable and evidence exports with a trusted domain adapter;
unsupported declarations cannot grant executable authority. Secret values are
omitted by the host. Registration validation enforces bounded pointers and
unique named exports.

`ToolReturn.value`, full raw routes, callback fields, and route ordering remain
unchanged. Model projection is a separate host responsibility. EVM route markers
now name `evm_stage_tx` and `evm_commit_txs`; SVM commit markers name the unified
`svm_commit_txs` host tool. Historical Rust marker types remain aliases for these
current tool names. Host adaptation of bound staged artifacts to authorized
resource arguments is required; renaming a marker alone does not prove a routed
operation works end to end.

Final backend consumers must pin the reviewed SDK commit and resolve its lockfile
portably. Temporary local path overrides are development configuration, not the
final dependency contract.
