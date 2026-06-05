---
name: aomi-app-e2e-tester
description: Author and run an end-to-end behavioral test for an Aomi app. Reads the seed `user_story` from `apps/<platform>/test.json` (written earlier by `aomi-app-ux-tool-maker`), expands it into a multi-turn TDD spec (prompts, expected tool calls per turn, final assertions on `UserState`), then invokes the harness at `aomi/crates/runtime/tests/local-app-e2e.rs` to run it. Reports a per-step pass/fail table. Triggers include "test this app", "/aomi-app-e2e-tester", "expand the test.json", "run e2e for X", "after curating tools, write the test."
---

# aomi-app-e2e-tester

You are turning a seed `user_story` into a complete, runnable e2e test for an Aomi app, then running it and reporting results.

## Position in the pipeline

```
aomi-build new-app → aomi-app-ux-tool-maker (curates + writes seed user_story)
                                            ↓
                                    YOU: expand test.json, run, report
```

When you start, `apps/<platform>/test.json` already exists with at minimum:
```json
{ "user_story": "..." }
```

Your job is to fill in: `wallet_seed`, `turns`, `final_assertion`. Then run and report.

## The harness contract

The harness at `aomi/crates/runtime/tests/local-app-e2e.rs` reads **only the fields documented in [docs/test-json-schema.md](../../docs/test-json-schema.md)**. Anything else is silently ignored. The reference implementation is [apps/khalani/test.json](../../apps/khalani/test.json) — match its shape exactly.

**Allowed fields, complete list:**

```jsonc
{
  "user_story": "string",                     // required — drives intent, not asserted
  "wallet_seed": {                            // optional — pre-populates UserState
    "address": "string",
    "chain_id": <number>,
    "is_connected": <bool>
  },
  "turns": [
    {
      "prompt": "string",
      "expected_tools": {
        // exactly one of:
        "must_call": ["tool_name", ...],
        "any_of":    ["tool_name", ...]
      }
    }
  ],
  "final_assertion": {
    "user_state": {                           // optional block
      "pending_txs": {
        "min_count": <number>,
        "expect_any": [
          {                                   // each field optional; tx must satisfy ALL specified
            "chain_id": <number>,
            "data_starts_with": "0x",
            "data_min_len": <number>
          }
        ]
      },
      "pending_eip712s": { "max_count": <number> }
    },
    "no_errors": <bool>,                      // fails on tool Err(...) when true
    "max_turns": <number>                     // session length cap; catches LLM loops
  }
}
```

**Do NOT add other fields.** The harness ignores them, which is worse than refusing them — looks like they work but don't.

## Inputs

Required:
- `platform` — e.g. `lifi`, `dune`. The app at `apps/<platform>/` must compile and have a seed `test.json`.

If `apps/<platform>/test.json` doesn't exist or has no `user_story`, **stop and tell the user** to run `aomi-app-ux-tool-maker` first.

## Pre-reading (always)

1. `apps/<platform>/test.json` — the seed (currently just `user_story`)
2. `apps/<platform>/src/lib.rs` — PREAMBLE + tool list (your turn budget should match what the LLM actually sees)
3. `apps/<platform>/src/tool.rs` — tool args, descriptions, what each one returns/mutates
4. `aomi/crates/tools/src/user_state.rs` — `UserState` shape you're asserting against
5. **`apps/khalani/test.json`** — the canonical reference (single test, exact shape the harness reads)
6. `docs/test-json-schema.md` — the schema doc

## Workflow

### 1. Read context

From `tool.rs`, build a mental list of:
- Tool names + their descriptions (verbatim — these are what the LLM sees)
- Which tools mutate `UserState` (build txs / sigs) vs read-only data tools
- Which composites exist and what sub-tools they wrap

Categorize the platform: price-oracle / swap-bridge / lending-yield / perps-cex / prediction / social.

### 2. Decompose the user_story into turns

Aim for **1–3 turns**. The khalani reference has just 1 turn ("Swap … and stage the bridge transactions") — that's fine when the prompt is concrete enough that the LLM does the whole flow in one shot.

| Archetype | Typical turn shape |
|---|---|
| Price oracle / data | 1–2 turns: ask → answer. No `user_state` block. |
| Swap / bridge | 1–2 turns: combined "swap X for Y on chain Z, build the tx" OR split discovery + build. Final: `pending_txs.min_count >= 1`. |
| Lending / yield | 1–2 turns. Same as swap. |
| Perps / CEX (read) | 1–2 turns. No `user_state`. |
| Perps / CEX (write) | 1–2 turns. Final: `pending_txs` for on-chain DEXes. |
| Prediction | 1–2 turns. Final: `pending_eip712s` (Polymarket-style L2) or `pending_txs`. |
| Social | 1 turn: search/lookup. No `user_state`. |

Each turn is a **terse natural-language prompt** the user might actually type.

### 3. Pick `expected_tools` per turn

- **`must_call`** for action turns where specific tools are non-negotiable (typical for the terminal turn)
- **`any_of`** for discovery turns where the LLM may pick any of a few related tools

Single-turn tests usually use `must_call` with the composite tool name (e.g. `["khalani_quote", "khalani_build_deposit"]`).

### 4. Build `final_assertion`

#### Tx-building app
- `user_state.pending_txs.min_count: 1` (or higher if the flow naturally produces multiple).
- `expect_any` with the most stable fields:
  - `chain_id` — include if user_story implies a chain
  - `data_starts_with: "0x"` + `data_min_len: 10` — sanity check (calldata is non-empty, hex-shaped)
- `pending_eip712s.max_count: 0` if no sigs expected (catches stray sig flows)

**Do not add** `kind`, `to`, `to_contains`, `value_non_zero`, `max_count` on `pending_txs`, or `expect_all` — the harness doesn't read them.

#### Sig-building app
- Currently no per-sig fields beyond `max_count` are supported by the harness. If a flow builds an EIP-712 sig, assert `pending_eip712s.max_count: <some N>` is wrong (max_count caps, doesn't require). Until the harness adds `min_count` for sigs, just omit the `pending_eip712s` block (the test will pass on the sig flow but won't enforce it).

#### Read-only app
- Skip `user_state` block entirely (e.g. defillama)
- Rely on `no_errors: true` + `max_turns` for the cross-cutting checks

#### Cross-cutting (always include)
- `no_errors: true` — tool errors fail the test (set `false` only if the scenario explicitly tolerates errors; rare)
- `max_turns: <N>` — `len(turns) * 2 + 4` rounded up; minimum 6. Khalani uses 30 for a 1-turn flow which is generous; aim lower for tighter feedback.

### 5. Write the expanded test.json

Write to `apps/<platform>/test.json` as plain JSON (no JSONC comments — the harness uses serde_json). 2-space indent. Preserve the original `user_story` verbatim.

### 6. Run the harness

```
cd /Users/cecilia/Code/aomi-sdk && cargo test \
  -p aomi-runtime --test local-app-e2e -- --nocapture <test-fn>
```

Check `aomi/crates/runtime/tests/local-app-e2e.rs` for the actual generic-runner entry point name and required env vars (`ANTHROPIC_API_KEY` is universal; per-app keys like `LIFI_API_KEY` may also be needed).

### 7. Report (markdown table)

After the harness completes, emit a per-step results table:

```
| Step                                    | Result | Detail |
|---|---|---|
| Build app dylib                         | ✅     | `cargo build -p lifi` |
| Load app (manifest validates)           | ✅     | 6 tools registered |
| Turn 1: discovery                       | ✅     | called `lifi_get_swap_quote` (matched `any_of`) |
| Turn 2: build                           | ✅     | called `lifi_build_swap_tx` (matched `must_call`) |
| `pending_txs.min_count >= 1`            | ✅     | actual: 1 |
| `pending_txs.expect_any[0]` matches     | ❌     | tx data starts with "0x00..." (expected non-empty calldata) |
| `pending_eip712s.max_count <= 0`        | ✅     | actual: 0 |
| `no_errors`                             | ✅     | 0 tool errors |
| `max_turns <= 6`                        | ✅     | actual: 2 |
```

If anything failed:
- Show the actual values vs expected
- Don't auto-fix the test.json — surface the failure to the user (might be a real app bug, a flaky LLM, or a too-strict assertion)

If everything passed: print one-line summary `✅ <platform> e2e: all assertions pass`.

## Anti-patterns

- **Don't add fields outside the documented set.** The harness ignores them, which gives a false sense of test coverage. No `tool_responses`, `topics_emitted`, `max_cost_usd`, `name`, `expect_all`, `max_count` on `pending_txs`, `kind`/`to`/`value_non_zero` on `expect_any` entries.
- **Don't wrap in `stories[]`.** One test per app.
- **Don't assert on volatile values** in `expect_any`: prices, gas, exact `to:` addresses (router contracts upgrade), exact `value:` (depends on slippage).
- **Don't use `exactly` for tool calls.** Not a supported mode.
- **Don't run the test if the app doesn't build first.** Explicit `cargo build -p <platform>` before invoking the harness.
- **Don't fix failing assertions silently.** A failure means: real bug, flaky LLM, or wrong assertion. The user reads the table and decides which.

## Refusal cases

Stop and tell the user if:
- `apps/<platform>/test.json` doesn't exist or has no `user_story` (run `aomi-app-ux-tool-maker` first)
- The app doesn't compile (`cargo build -p <platform>` fails)
- `ANTHROPIC_API_KEY` isn't set (the harness needs it)
- The user_story is fundamentally untestable e2e (e.g., asks for live blockchain interaction we can't sandbox)
- Required platform credentials missing (e.g. `LIFI_API_KEY`) and the app won't function without them
