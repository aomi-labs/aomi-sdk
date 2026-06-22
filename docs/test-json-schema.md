# `apps/<platform>/test.json` schema

End-to-end behavioral test spec for an Aomi app. Authored by the
`aomi-app-e2e-tester` skill (seeded by `aomi-app-ux-tool-maker`),
consumed by the harness at `aomi/crates/runtime/tests/local-app-e2e.rs`.

**One test per app**. Reference implementation: [apps/khalani/test.json](../apps/khalani/test.json).

## Top-level shape

```jsonc
{
  "user_story": "string",                // one-sentence happy-path scenario
  "wallet_seed": { ... },                // optional — UserState seeded before turn 1
  "turns": [ ... ],                      // 1–N turns; prompt + expected_tools per turn
  "final_assertion": { ... }             // assertions after all turns finish
}
```

The harness only reads the fields documented below. It uses strict serde
deserialization, so **unknown fields fail spec loading**. Don't introduce new
ones without updating `product-mono/aomi/crates/runtime/tests/app_e2e/spec.rs`.

## `user_story` (required)

Terse natural-language scenario the user might actually type. Drives test-author intent; not asserted on.

## `wallet_seed` (optional)

Pre-populates `UserState` before the first turn. Many tools assume a connected wallet.

```json
{
  "address": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045",
  "chain_id": 1,
  "is_connected": true
}
```

Fields supported: `address`, `chain_id`, `is_connected`. Omit the block entirely for read-only / data apps that don't read wallet state.

## `turns[]` (required)

Ordered list of user prompts. The harness sends each prompt to the LLM in sequence and asserts which tools were called during that turn.

```jsonc
{
  "prompt": "Swap 100 USDC for ETH on Optimism via Khalani",
  "expected_tools": {
    // Exactly one of:
    "must_call": ["khalani_quote", "khalani_build_deposit"],
    "any_of":    ["khalani_quote", "khalani_build_deposit"]
  }
}
```

### `expected_tools` modes

| Mode | Semantics | When to use |
|---|---|---|
| `must_call: [...]` | Every listed tool must be called (subset; others may also be called). | **Action turns** — specific tools are non-negotiable. |
| `any_of: [...]`    | At least one listed tool must be called. | **Discovery turns** — LLM may legitimately pick different tools. |

Default pattern: `any_of` for early/discovery turns, `must_call` for the terminal action turn. Single-turn tests usually use `must_call`.

## `final_assertion` (required)

Run after all turns complete. Sub-blocks below are optional.

### `final_assertion.user_state.pending_txs`

```jsonc
{
  "min_count": 1,                              // at least N txs queued
  "max_count": 2,                              // at most N txs queued
  "expect_any": [
    {
      "kind": "swap",                          // exact match (optional)
      "chain_id": 1,                           // exact match (optional)
      "data_starts_with": "0x",                // calldata prefix (optional)
      "data_min_len": 10                       // calldata length sanity (optional)
    }
  ]
}
```

A tx matches an `expect_any` entry if it satisfies every field specified in that
entry. Supported predicate fields are `kind`, `chain_id`, `data_starts_with`,
and `data_min_len`. `min_count` / `max_count` run before `expect_any` matches
against the resulting set.

### `final_assertion.user_state.pending_eip712s`

```json
{
  "max_count": 0
}
```

Use `max_count: 0` to assert no signatures were prompted (catches stray sig flows on swap apps).

### Cross-cutting

```json
{
  "tool_responses": {
    "any_returned_substring": "USDC"
  },
  "no_errors": true,
  "max_turns": 6
}
```

- `tool_responses.any_returned_substring` — passes when at least one
  `tool_result` body contains the substring.
- `no_errors` — fails the test if any tool returned `Err(...)`. Set `false` to disable the check (rare; only for tests where errors are part of the scenario).
- `max_turns` — upper bound on session length. Catches runaway LLM loops. Set to `len(turns) * 1.5` rounded up, with a minimum of 6 to allow re-asks.

## Anti-patterns

- **Don't add fields the harness doesn't read.** No `topics_emitted`, `max_cost_usd`, `name`, `expect_all`, `to`, `value_non_zero`, etc. Unknown fields fail spec parsing.
- **Don't wrap in a `stories[]` array.** Single test per file.
- **Don't assert on volatile values** in `expect_any`: prices, gas, exact `to:` (router upgrades), exact `value:`, exact `data:`.
- **Don't set `no_errors: false`** unless the test scenario specifically needs to tolerate errors (very rare).

## Example: complete swap test (khalani)

```json
{
  "user_story": "Swap 100 USDC from Ethereum to ETH on Optimism via Khalani Hyperstream",
  "wallet_seed": {
    "address": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045",
    "chain_id": 1,
    "is_connected": true
  },
  "turns": [
    {
      "prompt": "Swap 100 USDC from Ethereum to ETH on Optimism via Khalani. Use the recommended route, build the deposit, and stage the bridge transactions.",
      "expected_tools": {
        "must_call": ["khalani_quote", "khalani_build_deposit"]
      }
    }
  ],
  "final_assertion": {
    "user_state": {
      "pending_txs": {
        "min_count": 1,
        "expect_any": [
          { "chain_id": 1, "data_starts_with": "0x", "data_min_len": 10 }
        ]
      },
      "pending_eip712s": { "max_count": 0 }
    },
    "no_errors": false,
    "max_turns": 30
  }
}
```

## Example: read-only data test (defillama)

```json
{
  "user_story": "Find the biggest lending protocol on Ethereum and show me its current TVL and recent trend",
  "turns": [
    {
      "prompt": "Which lending protocols have the most TVL right now?",
      "expected_tools": {
        "any_of": ["defillama_list_protocols"]
      }
    },
    {
      "prompt": "Great — give me a deep dive on the top one: its current TVL, per-chain breakdown, and recent history.",
      "expected_tools": {
        "must_call": ["defillama_get_protocol_tvl"]
      }
    }
  ],
  "final_assertion": {
    "no_errors": true,
    "max_turns": 6
  }
}
```

Read-only flows have no `wallet_seed` (no wallet needed) and no `user_state` block in `final_assertion` (no UserState mutations expected).
