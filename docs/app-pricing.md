# App pricing sidecar (`pricing.toml`)

An app declares what it charges by committing a `pricing.toml` next to its
`Cargo.toml` (`apps/<dir>/pricing.toml`). No file = the app is free. The file
is pure data — the Aomi host reads it at app load and binds the prices onto
the app's tools; the platform's take rates are applied host-side and are not
declared here.

## Schema (version 1)

```toml
version = 1

# Who gets paid. Referenced by name from [resources.*].
[[beneficiaries]]
name  = "team_evm"
type  = "evm_address"
chain = "eip155:1"
value = "0x9C7a99480c59955a635123EDa064456393e519f5"

# Per-tool flat prices, in Aomi credits (1.0 = $0.01) per call.
[resources.binance_place_order]
pricing     = { flat = 1.0 }
beneficiary = "team_evm"      # optional

# On-chain outcome fees: bps of the flow, settled on the user's AA batch.
[[outcome]]
effect      = "flow"
bps         = 30
beneficiary = "team_evm"
```

## How it ships

`aomi-build compile` copies `apps/<dir>/pricing.toml` into the release bundle
as `plugins/<app-name>.pricing.toml` (keyed by the dylib's manifest name, the
same name the host uses). A malformed file, or one without an integer
`version`, fails the build — better here than refusing the app load on the
host. Deleting the file un-prices the app on the next release.

The host validates the full schema at load and **refuses to load** an app
whose sidecar is invalid (never run an app underpriced). Hosts also accept
the legacy filename `<app>.billing.toml` during the rename transition; new
apps must use `pricing.toml`.
