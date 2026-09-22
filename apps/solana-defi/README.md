# Solana DeFi preparation

This Aomi app reads markets and prepares unsigned Solana instructions for Jupiter
Lend, Kamino Earn and PumpSwap. A client such as Hummingbot can stage those batches
through one Aomi Executor integration, simulate, review and confirm them.
Protocol-specific preparation uses the pinned official SDKs in `service/`.
This is support for those protocol families, not automatic support for arbitrary
new programs. The example market catalog is a starting point, not a recommendation
or a complete market index. Operators can supply another market address owned by
the selected supported program.

## Deployment

The Rust app and Node preparation service are deployed together. The service has
no private keys and never signs, stages or submits transactions.

1. From `service/`, install with `npm ci --ignore-scripts` and run `npm test`.
2. Set `AOMI_SOLANA_RPC_URL` on the service to the intended Solana mainnet RPC.
   For a local demo, use an isolated mainnet mirror on loopback, with no public
   write fallback. Per-request RPC overrides are rejected.
3. Run `npm start`. The service binds only `127.0.0.1`, port `18895` by default;
   `AOMI_SOLANA_DEFI_PORT` changes the port. It is intended as a colocated service,
   not a publicly exposed API. `GET /health` reports process health only.
4. Build the Rust app against the host's matching Aomi SDK version and target.
   Register and activate its release using the normal Aomi app publication flow.
   The registered release must match the artifact manifest and checksum.
5. Set `AOMI_SOLANA_DEFI_URL=http://127.0.0.1:18895/` on the Aomi backend loading
   the app. For a path prefix, include the trailing slash. Do not embed credentials
   or query strings in this URL.
6. Invoke `solana-defi` with its registered `application_id`. Hummingbot uses
   `AOMI_PREPARATION_APPLICATION_ID` for this identity. A name-only local library
   does not satisfy dynamic app release verification.

No production deployment or successful publication is implied by these steps.

## Tools and units

| Tool | Arguments | Result |
| --- | --- | --- |
| `solana_defi_venues` | `{}` | Supported families and example addresses |
| `solana_defi_market` | `venue`, `market`, `wallet` | On-chain metadata, charges and current wallet position |
| `solana_defi_position` | `venue`, `market`, `wallet` | Current wallet position with market context |
| `solana_defi_prepare` | Above plus `action`, amount or `withdraw_all` | Unsigned batches and review metadata |

Venue IDs are `jupiter-lend`, `kamino-earn`, and `pumpswap`. Addresses must be
canonical Solana public keys. Amounts are positive integer strings in raw units;
floating point input and unexpected fields are rejected.

- Jupiter deposits and amount-based withdrawals use underlying asset units.
- Kamino deposits use asset units; amount-based withdrawals use share units.
- PumpSwap deposits specify the target quote-token contribution. Review both
  SDK-derived maximum inputs. Withdrawals use LP units and expose minimum outputs.
  `slippage_bps` defaults to 50 and accepts integers from 0 to 500.
- `action: "withdraw", withdraw_all: true` uses the current wallet position and
  must omit `amount_raw`. Kamino includes supported ordinary farm staking and
  unstaking; preparation for a first-loss-capital farm is explicitly refused.

Prepared batches expire after 120 seconds. Clients must simulate before review,
bind confirmation to the exact reviewed instructions and wallet, and reject stale
or changed plans. The response itself is not simulation evidence or permission
to spend. Fee ceilings, asset budgets, operator grants, signing and receipt checks
belong to the executor/host. Wallet balances are not controller contributions or
realized profit. Simulation cannot guarantee future state or execution success.

## Validation

From the SDK repository root:

```sh
cargo fmt --manifest-path apps/solana-defi/Cargo.toml -- --check
cargo clippy --manifest-path apps/solana-defi/Cargo.toml --lib -- -Dwarnings
cargo test --manifest-path apps/solana-defi/Cargo.toml --no-run
npm --prefix apps/solana-defi/service test
```

Service tests cover input precision, address/field validation, instruction mapping,
protocol account ownership and remote network identity. They do not replace live
simulation and independent receipt verification for each supported action. Current
dependencies emit Solana Kit peer-version warnings and can use the bigint pure-JS
fallback; validate the installed lockfile against actual protocol paths before
upgrading it.
