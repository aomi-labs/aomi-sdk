---
name: aomi-app-ux-tool-maker
description: Rewrite an Aomi app's tool layer (apps/<platform>/src/tool.rs) so it's actually usable — turning the mechanical stubs from `aomi-build gen-tool` (one tool per endpoint, names like `dune_getv1_query_queryid_results`) into a curated set of user-centric tools backed by the generated client in ext/. Optionally takes a user intent ("collect token prices from defillama"); otherwise infers typical user stories from the platform category. Composes endpoints into composite tools where useful, prunes endpoints that aren't valuable as tools, names tools after user goals, writes domain-specific PREAMBLE. Triggers include "design tools for X", "make this app usable", "rewrite tool layer", "/aomi-app-ux-tool-maker", "after gen-app design the tools", "curate tools".
---

# aomi-app-ux-tool-maker

You are taking an Aomi app from "compiles" to "actually useful." The previous step (`aomi-build gen-tool`) emitted one mechanical stub per endpoint with operationId-derived names. Your job is to **rewrite the tool layer** so:

- Tool names match user goals, not endpoint paths.
- Tool descriptions tell the LLM *when to use* them, not just what they return.
- Composite tools chain multiple endpoints when one user goal needs several.
- Useless endpoints (admin, internal, niche) are dropped.
- The app's `PREAMBLE` is domain-specific — it tells the assistant what kinds of questions it can answer.

## Inputs

Required:
- `platform` — e.g. `dune`, `defillama`, `lifi`. The app at `apps/<platform>/` must already exist (run `aomi-build new-app` or at least `aomi-build gen-tool` first).

Optional:
- `intent` — one sentence about what the user wants this app to do, e.g.
  - "I want to query token price action and TVL trends from DefiLlama"
  - "I want to swap tokens cross-chain via LiFi"
  - "I want to monitor my Hyperliquid positions and place stop-losses"

If no intent is given, ask the user for one. **Do not skip this** — without an intent, you'll guess at user stories that may not match what the user actually wants.

## Pre-reading (always)

Before designing anything, read these to ground yourself:

1. `ext/specs/<platform>.yaml` — the API surface, with operation summaries/descriptions.
2. `ext/src/<platform>/client.rs` — the generated method names (you need these to call from tool.rs).
3. `apps/<platform>/src/tool.rs` (the current generated stub) — what's there now.
4. `apps/<platform>/src/lib.rs` — current PREAMBLE and tool list.
5. **One existing well-curated app for reference**: read `apps/binance/src/tool.rs` or `apps/hyperliquid/src/tool.rs` — both are hand-crafted with good naming, descriptions, and composition. Match this quality bar.
6. **If the platform requires auth**: read one of [ext/src/binance/auth.rs](ext/src/binance/auth.rs), [ext/src/bybit/auth.rs](ext/src/bybit/auth.rs), [ext/src/okx/auth.rs](ext/src/okx/auth.rs), [apps/limitless/src/auth.rs](apps/limitless/src/auth.rs) (HMAC venues) or [apps/krexa/src/auth.rs](apps/krexa/src/auth.rs) (static `X-API-Key`), plus the shared [ext/src/hmac_auth.rs](ext/src/hmac_auth.rs) primitives. See "Auth: per-call shim, not middleware" below.

## Workflow

### 1. Categorise the platform

Pick the closest archetype. This shapes which user stories matter:

| Archetype | Examples | Typical user stories |
|---|---|---|
| Price oracle / data | DefiLlama, Dune, CoinGecko | "what's the current price/TVL of X", "show historical Y", "compare A vs B over time" |
| Swap / bridge | LiFi, CoW, 1inch, 0x, Across | "swap A for B", "find best route", "estimate fees", "build a tx I can sign" |
| Lending / yield | Morpho, Yearn, Aave | "show APY for asset", "view my positions", "deposit", "withdraw" |
| Perps / spot exchange | Binance, Bybit, OKX, dYdX, GMX, Hyperliquid | "show price/depth", "place order", "view balance", "view positions", "set stop-loss" |
| Prediction market | Kalshi, Manifold, Polymarket | "list active markets", "show market detail", "show my positions", "place bet" |
| Social | X, Neynar | "post", "search posts/users", "view profile/feed" |

Write down the archetype and 3–6 user stories. Show them to the user for sign-off if you weren't given an explicit intent.

### 2. Map user stories → tools

For each user story, decide:
- **Tool name** — verb-led, snake_case, prefixed with platform: `defillama_get_token_price`, `lifi_quote_swap`, `binance_place_limit_order`. Avoid endpoint-derived names like `dune_getv1_query_queryid_results`.
- **Args struct** — only the fields the user actually thinks about. If an endpoint has 12 params and 9 are knobs, keep the 3 that matter and hardcode reasonable defaults for the rest. Required vs optional reflects user mental model, not the spec.
- **Description** — start with "Use when…" or "Returns…". This is what the LLM sees when deciding whether to call your tool. Make it concrete.
- **Composition** — if the user goal needs multiple endpoints (e.g. quote → approve → tx for a swap), compose them in one tool. Don't make the LLM orchestrate.

Where the spec has overlapping endpoints (e.g. DefiLlama has `/protocols` and `/protocol/{id}`), pick the one that matches the user's intent. Don't expose both unless they serve genuinely different stories.

### 3. Cross-check API coverage

After drafting the tool list, walk back through the spec's operations and ask:
- Is there an op the user would clearly want that I missed?
- Is there a tool I drafted that doesn't actually serve a real user need?

Adjust. Print a one-line diff per change.

### 4. PREAMBLE

Rewrite `PREAMBLE` in `apps/<platform>/src/lib.rs`. Aim for ~25–40 lines. Sections:

- **Role** — "You are an AI assistant specialized in <platform> for <archetype use case>."
- **Capabilities** — bulleted list of what the user can do, mapping to your tools.
- **Important constraints** — auth requirements, rate limits, units, conventions (e.g. "all addresses are checksummed Ethereum addresses").
- **Workflow guidance** — when to chain tools (e.g. "for a swap: first call `lifi_quote_swap` to find the best route, then `lifi_build_tx` to get the transaction the user signs").
- **Formatting** — how to present results (e.g. "format prices in USD with 2 decimals; format APYs as percentages").

Look at `apps/binance/src/lib.rs` and `apps/hyperliquid/src/lib.rs` for length/tone reference.

### 5. Implement

Rewrite `apps/<platform>/src/tool.rs` from scratch with the curated tools. Patterns and discipline:

#### Get the workflow right (research if needed)

Before writing a composite tool body, you must know the **actual** sequence the platform expects. Spec summaries don't always say "after `execute`, poll `/status` until `STATE_COMPLETED`, then call `/results`." Read the platform's own docs / quickstart / SDK README to confirm.

- Use `WebFetch` or `WebSearch` against the platform's official docs (`docs.<platform>.com`, official GitHub README, official tutorials). Skip third-party blog posts unless nothing else exists.
- For each composite tool, explicitly verify: endpoint order, required intermediate state values (e.g. exact string of "completed" status), what counts as a terminal failure, recommended polling cadence.
- If docs disagree with the spec, the **docs win for behavior**, the **spec wins for types**. Note the discrepancy in a code comment.
- Don't guess polling intervals or timeout values. The platform usually documents them ("queries typically complete in 5–30 seconds; poll every 2 seconds"). Match that.

A composite that calls the right endpoints in the wrong order is worse than a stub that does nothing — the LLM will confidently call it and the user will see opaque errors.

#### Trim the arg surface

The spec's parameter list is an API contract; the user's mental model is much smaller. Aim for **5–6 args per tool, max**, even when the API takes 10+. Three rules of thumb:

1. **Compute, don't ask.** If a value can be derived in Rust — current timestamp, sensible page size, polling cadence, retry budget, default chain ID — compute it. Don't expose it. Every arg the LLM has to fill is a chance to fill it wrong.
2. **Keep what users actually think about.** "Token symbol" yes. "`include_metadata: bool`" no. "`order_by: Option<SortDirection>`" no. Pick a default the user would pick.
3. **Collapse related args.** If the API has `start_time` and `end_time`, consider exposing just `lookback_hours` and computing both. If it has `chain_id` and `chain_name`, expose `chain` (string) and resolve internally.

If you find yourself writing more than 6 fields on an Args struct, stop and ask: which 4 of these would the user actually care about? Hardcode or compute the rest.

#### Trim at the spec, not the tool

The generated client returns **typed** responses (e.g. `Vec<Chain>`, `SearchTokensResponse { data: Vec<Token> }`, `BuildDepositResponse`) — not `Value`. The tool body should forward the typed value directly: `ok(response)` or `ok(json!({ "k": response }))`.

If the response is too noisy for the LLM (logoURIs, audit blobs, extension maps, …), **delete those fields from `ext/specs/<platform>.yaml` and regenerate the client.** Don't add a projection layer in `tool.rs`.

Why:
- **Single source of truth.** The spec describes the API surface the LLM sees. The Rust type IS the slim type.
- **No drift.** When the spec evolves, the typed struct evolves with it — no parallel `*Summary` struct to keep in sync.
- **No `From` impls to maintain.** `tool.rs` stays one line per response.
- **serde silently drops unlisted fields on deserialize**, so trimming the spec produces clean typed responses without breaking deserialization of real (richer) live JSON.

```yaml
# ❌ Spec includes everything the API actually returns
Token:
  type: object
  properties:
    chainId:    { type: integer, format: int64 }
    address:    { type: string }
    symbol:     { type: string }
    decimals:   { type: integer }
    name:       { type: string }
    logoURI:    { type: string, format: uri }      # UI-only
    extensions: { type: object, additionalProperties: true }  # loose blob

# ✅ Spec restricted to fields the LLM acts on
Token:
  type: object
  required: [address, chainId, symbol, decimals]
  properties:
    chainId:  { type: integer, format: int64 }
    address:  { type: string }
    symbol:   { type: string }
    decimals: { type: integer }
    name:     { type: string }
```

```rust
// Tool body stays trivial after trimming — the typed response IS the slim shape.
let response = client.search_tokens(...).await?.into_inner();
ok(response)
```

Workflow:
1. Spot a tool that returns more fields than the LLM needs.
2. Edit `ext/specs/<platform>.yaml` to drop the unwanted fields from the response schema.
3. `aomi-build gen-client <platform> --shared --force` (or omit `--shared` for app-local).
4. `cargo build -p <platform>` — typed access just works.

What to trim at the spec layer:
- **UI-only fields**: `logoURI`, `iconUrl`, `description` HTML, `audits`, `extensions`.
- **Dev-tooling metadata**: `rpcUrls`, `contracts`, `formatters`, `serializers`, `blockTime`.
- **Pagination metadata** the tool consumed internally: `totalCount`, `nextCursor` when the tool always returns one page.
- **Fields that duplicate** something already in the response: don't keep both `chain_id` AND a nested `chain` object with rpcUrls.

What to KEEP in the spec:
- IDs / addresses / status enums the LLM uses for follow-up tool calls.
- Numeric amounts the LLM might compare or arithmetic on.
- One human-readable label per item (`name` or `symbol`) for assistant explanations.

When the spec marks a response as `additionalProperties: true`, the client returns `Map<String, Value>` (genuinely untyped) — Value-walking is unavoidable there. **The fix is `aomi-build tighten-spec <platform>`**: capture real responses to `ext/specs/<platform>.samples/` and run tighten-spec to infer concrete schemas. Then trim what you don't want as above.

The escape hatch (one endpoint serves multiple tool intents needing different shapes) is to add a `*Summary` struct in `tool.rs` — but that's an exception that needs justification, not the default.

#### Auth: per-call shim, not middleware

> For wire-level recipes (Binance/Bybit/OKX/Limitless/Krexa), sequence diagrams, and a decision guide for new venues, see **[docs/auth-practices.md](../../../docs/auth-practices.md)**. The section below is the procedural summary; that doc is the deep-dive.

When a platform requires authentication (API key, HMAC signature, session token, on-chain signature, …) the curated tool layer follows a strict pattern: **per-call resolution in `tool.rs`, primitives in a thin `apps/<platform>/src/auth.rs` shim, headers/params passed positionally into the progenitor-generated client.** No `reqwest::Client` middleware, no global state, no implicit env reads inside the generated client.

##### Why per-call

- **The client stays generic.** `Client::new(base_url)` works for anonymous reads; the same client instance can also serve authed writes — the difference is per-call. No need for parallel "anon" and "authed" client constructors.
- **Different agents, same process.** A single Aomi runtime can serve multiple users with different keys. Globally configured auth makes that impossible.
- **Errors surface at the call site.** If `KREXA_API_KEY` isn't set, the *specific tool* that needed it returns `Err("[krexa] KREXA_API_KEY not set; …")`. Anonymous tools keep working.
- **Mirrors OpenAPI semantics.** When the spec marks an auth header as an operation parameter, progenitor types it as a required argument; the tool layer satisfies that contract per call.

##### What the shim looks like

`apps/<platform>/src/auth.rs` is **stateless, pure functions, no client wrapping**. Existing references (read these for tone and length — they're all 40–80 lines):

| File | Auth style | Shape |
|---|---|---|
| [ext/src/binance/auth.rs](ext/src/binance/auth.rs) | HMAC-SHA256 over query string, hex | `sign(secret, query) -> Result<String, String>`, `current_timestamp_ms() -> i64`, `build_query(pairs) -> String` |
| [ext/src/bybit/auth.rs](ext/src/bybit/auth.rs) | HMAC over `ts+key+recv+payload`, hex | `sign_query`, `sign_body`, `current_timestamp_ms`, alphabetical `build_query`, `RECV_WINDOW` constant |
| [ext/src/okx/auth.rs](ext/src/okx/auth.rs) | HMAC over `ts+method+path+body`, base64 | `sign(secret, ts, method, path, body)`, `iso_timestamp()` |
| [apps/limitless/src/auth.rs](apps/limitless/src/auth.rs) | HMAC over `ts\nmethod\npath\nbody`, base64 (secret itself base64) | `sign(secret_b64, ts, method, path, body)`, `iso_timestamp()` |
| [apps/krexa/src/auth.rs](apps/krexa/src/auth.rs) | Static `X-API-Key` (no signing) | `api_key() -> Result<String, String>` only |

All four HMAC variants delegate primitives to shared **[ext/src/hmac_auth.rs](ext/src/hmac_auth.rs)** (`hmac_sha256_hex` / `hmac_sha256_base64`, `current_timestamp_ms` / `iso_timestamp_ms`, `urlencode`, `build_query`, `base64_decode`). Pull it in for any HMAC venue:

```toml
aomi-ext = { path = "../../ext", features = ["hmac-auth"] }
```

The per-app shim then owns ONLY the venue-specific bits:
1. **Prehash format** (where the timestamp/method/path/body get concatenated, with what separators).
2. **Secret handling quirks** (is the dashboard secret already base64? does the recv-window go in the prehash?).
3. **Query-ordering rules** (Bybit sorts alphabetically; Binance preserves insertion order).
4. **Return-type names** the tool layer expects to import.

If the venue's prehash recipe is identical to an existing one, reuse the shared helpers and write a five-line shim. Don't reimplement HMAC or base64 anywhere outside `ext/src/hmac_auth.rs`.

##### How the tool layer calls it

For HMAC venues, every authed tool computes the signature inline:

```rust
// apps/okx/src/tool.rs (place_order)
let timestamp = iso_timestamp();
let signature = sign(
    &secret_key, &timestamp, "POST", "/api/v5/trade/order", &body_str,
)?;
runtime.block_on(async move {
    let client = OkxClient::new(BASE_URL);
    client.place_order(
        api_key.as_str(),
        passphrase.as_str(),
        signature.as_str(),
        timestamp.as_str(),
        &body,
    ).await
})
```

For static-bearer venues (Krexa, anything with a plain `Authorization: Bearer` or `X-API-Key`), it collapses to one line:

```rust
// apps/krexa/src/tool.rs (pay_api_call)
let api_key = auth::api_key()?;
client().paysh_call(args.agent.as_str(), api_key.as_str(), &body).await
```

The api_key argument is positional because the spec declares `X-API-Key` as an operation parameter (see "Spec setup" below). Without that declaration, progenitor doesn't emit a slot for it and you'd have to fall back to middleware — don't.

##### Spec setup (so progenitor emits positional args)

The auth header has to be declared **as an operation parameter**, not just under `securitySchemes`. Progenitor ignores `securitySchemes` for codegen — it only generates positional args for entries in each operation's `parameters:`.

The clean pattern: define the header once under `components.parameters` and `$ref` it from each authed operation.

```yaml
# components.parameters
ApiKeyHeader:
  in: header
  name: X-API-Key
  required: true              # progenitor: `&str`; if false: `Option<&str>`
  schema: { type: string }
  description: kx_-prefixed key from POST /access/provision-key

# paths./solana/paysh/{agent}/call.post.parameters
- $ref: '#/components/parameters/AgentPath'
- $ref: '#/components/parameters/ApiKeyHeader'
```

After regenerating with `aomi-build gen-client <platform> --force`, the client method gains an `x_api_key: &'a str` slot positioned between path params and the body — exactly where the tool layer's `auth::api_key()?` plugs in.

##### What NOT to do

- **Don't wrap `reqwest::Client` with header-injecting middleware.** Even though `Client::new_with_client(...)` makes it possible, none of the existing venues do this and it breaks the per-call model.
- **Don't put the env-var read inside the generated client.** That file is regenerated by `aomi-build gen-client` — any hand-edit gets wiped on the next regen.
- **Don't expose the secret as a tool argument the LLM fills in.** Resolve it from `auth::*` or `aomi_sdk::resolve_secret_value` inside the tool body. The LLM should never see the secret.
- **Don't reimplement HMAC/base64 in a per-app `auth.rs`.** Use the shared `ext/src/hmac_auth.rs` primitives. The per-app file owns only venue-specific composition.
- **Don't add an `auth.rs` for a fully public API.** It's overhead without value. Krexa only got one because its Pay.sh write surface requires `X-API-Key`; the read surface does not.

#### Wallet handoff (on-chain sig + off-chain submit)

Many platforms have this shape: **build something off-chain → user signs on-chain → confirm/submit off-chain**. Examples: Khalani (`build_deposit` → `stage_tx` → `submit_order`), Across (`get_bridge_quote` → SpokePool tx → `get_deposit_status`), 1inch (`get_quote` → `approve_tx` + `swap_tx`), CoW (build order → EIP-712 signature → submit signed order), LiFi (quote → approve + swap → status), GMX/dYdX (place position → on-chain settlement).

**Wrong way (LLM orchestration):** the tool returns raw JSON like `{ approve_tx, swap_tx, status_url }` and the description says "stage these via stage_tx, then poll status." The LLM is now orchestrating a multi-step protocol it can get subtly wrong (forgets to wait for receipts, swaps order of approve+swap, copies calldata wrong, skips the post-submit step).

**Right way (`ToolReturn::route(...)` chain):** the tool emits a typed routed envelope. The host runs the wallet steps automatically, binds the resulting tx hash / signature, and fires the follow-up tool. The LLM just sees the final result.

The mechanics — these are SDK primitives in `aomi_sdk::*` and `host::*`:

```rust
ToolReturn::route(ok(preview)?)            // preview = JSON the LLM sees while waiting
    .next(|next| {
        // Sequential wallet-bound steps. For approve+swap, add both;
        // only the last one needs `.enforce(...)` since commit_txs
        // batches whatever's been staged.
        for (i, args) in stage_args.iter().enumerate() {
            let step = next.add::<host::StageTx>(args.clone());
            if i == last_index {
                step.note("Stage the deposit. Copy data.raw and to BYTE-FOR-BYTE.")
                    .enforce(EnforcementPolicy::Continue, |enforce| {
                        enforce.add::<host::SimulateBatch>(json!({}));
                        enforce
                            .add::<host::CommitTxs>(json!({ "aa_preference": "auto" }))
                            .bind_as("transaction_hash");      // ← what we await
                    });
            } else {
                step.note("Stage the ERC-20 approval. Copy data.raw and to byte-for-byte.");
            }
        }
    })
    .after::<SubmitOrder>(submit_template)   // tool that runs after wallet returns
    .awaits("transaction_hash")              // alias from .bind_as above
    .note("Deposit landed on-chain — register the order.")
    .try_build()
    .map_err(|e| format!("[platform] route build: {e}"))
```

**Variants by signing primitive:**

| When the wallet step is | Use | What it binds | Follow-up |
|---|---|---|---|
| Send one or more EVM transactions | `host::StageTx` × N → `enforce(SimulateBatch + CommitTxs.bind_as("transaction_hash"))` | tx hash | `.after::<SubmitOrder>(...)` for off-chain confirm; or omit for swap-only flows |
| Sign EIP-712 typed data (Permit2, CoW, Polymarket L2) | `host::CommitEip712` with `.bind_as("signature")` directly (no SimulateBatch/CommitTxs) | signature hex | `.after::<SubmitSignedOrder>(...)` to POST the signature |
| Solana sign-and-broadcast | `host::SignTxSolana` with `.bind_as("signed_tx")` | base64 signed tx bytes | `.after::<SubmitTx>(...)` to POST to the venue's RPC |

**`stage_tx` args shape (must be exact):**
```json
{
  "to": "0x...",
  "description": "Human-readable: 'Khalani USDC deposit'",
  "data": { "raw": "0xdeadbeef..." },     // raw is REQUIRED if not using `encode`
  "value": "0",                              // string, wei
  "gas_limit": null,                         // optional hint
  "kind": "erc20_approve"                    // optional semantic tag
}
```

If the API gives you fully-encoded calldata, use `data: { raw }`. If it gives you (function_signature, args), use `data: { encode: { signature, args } }` and the host ABI-encodes for you.

**Critical discipline:**

- **`bind_as` is the contract.** The follow-up tool's args struct must have a field whose name matches the bound alias (e.g. `transaction_hash: Option<String>`), and the runtime fills it in. Tools that bind `transaction_hash` need their follow-up to accept that field.
- **Note text matters.** The LLM sees `note(...)` strings. Tell it: "Copy `data.raw` and `to` byte-for-byte. Do not abbreviate, reformat, or truncate calldata." Without this, models hallucinate calldata fragments.
- **Use the typed `host::*` markers**, not stringly-typed names. `host::StageTx`, `host::CommitTxs`, `host::SimulateBatch`, `host::CommitEip712`, `host::SignTxSolana` — the macro turns these into the canonical tool names; passing wrong strings silently breaks the route.
- **`EnforcementPolicy::Continue` vs `::Stop`**: `Continue` lets the route move on to `.after::<>` after enforcement runs. `Stop` halts the chain (used when one of the enforced steps is itself the terminal outcome).
- **Don't put the wallet handoff in a side-tool.** A separate `submit_signed_tx` that the LLM has to remember to call manually defeats the whole point. The submit is `.after::<>(...)` of the build tool.

**When NOT to use this pattern:**
- The platform has no on-chain step (pure REST API).
- The transaction is fully off-chain (database update). `stage_tx` is for EVM signing.
- You're returning a quote only and the user explicitly didn't ask to execute. (`get_quote` is read-only; the routed pattern belongs on the `_swap` / `_bridge` / `_place_order` composite.)

#### Code patterns

- Keep the `<Platform>App` marker struct (it's the type the macro references).
- Keep helpers: `ok()` (wraps result with `source`), `rt()` (tokio runtime), `resolve_key()` (env-var fallback). Copy from current generated tool.rs or from `apps/binance/src/tool.rs`.
- Auth: resolve secrets per-call (env var `<PLATFORM>_API_KEY` via `auth::api_key()?` for static bearers, or via `aomi_sdk::resolve_secret_value` for SDK-managed secrets). Compute any signatures by calling into `apps/<platform>/src/auth.rs`. **Pass the resulting headers/signatures positionally into the generated client method** — see the "Auth: per-call shim, not middleware" section above.
- Calls into the generated client are async — wrap with `runtime.block_on(async move { client.method(args).await }).map_err(...)?`.
- Composite tools sequence multiple `.await` calls and combine the results in the JSON return.

#### Safety rails (no infinite loops, no bloat)

These are non-negotiable for any composite or polling tool:

- **Every loop has a deadline.** Polling loops use `Instant::now() + Duration::from_secs(N)` and check on each iteration. Default max-wait should be small (30–120s); expose as an Args field only if the user genuinely needs to extend it.
- **Every loop has a max iteration count too.** Even with a deadline, cap iterations as a belt-and-suspenders guard against `tokio::time::sleep` mocking weirdness or system clock skew.
- **Composite tools have a step budget.** A tool that orchestrates "execute → poll → fetch" is fine. A tool that calls 8 endpoints and dispatches based on intermediate results probably isn't a tool — it's an agent. Don't write that.
- **No new dependencies without strong justification.** The standard kit (aomi-sdk, aomi-ext, schemars, serde, serde_json, tokio) covers 99% of cases. If you reach for `regex`, `chrono`, `url`, etc., ask whether you can do it with `std`. If you must add a dep, mention it explicitly in the report.
- **Don't construct Tokio runtimes inside loops.** One per tool call, at the top.
- **Sleep with `tokio::time::sleep`, not `std::thread::sleep`** — never block the runtime thread.
- **Sanity-check error paths.** If an endpoint returns non-success, the tool returns `Err(format!("[platform] op X: {e}"))`. Don't retry indefinitely on errors. Don't swallow errors and return empty results.

Update `apps/<platform>/src/lib.rs` with the new PREAMBLE and the new tool list in `dyn_aomi_app!`.

Don't touch `Cargo.toml` unless you're adding a dep (rare — usually the gen-tool Cargo.toml is fine).

### 5.5. Reconcile PREAMBLE with the implementation

After step 5, go back to the PREAMBLE you drafted in step 4 and check:

- **Does every tool you actually shipped have a line in Capabilities?** Add missing ones; remove lines for tools you ended up dropping.
- **Does the Workflow guidance match what your composite tools actually do?** If `dune_run_query` waits for completion internally, the PREAMBLE shouldn't tell the LLM to call status separately.
- **Are the conventions in PREAMBLE consistent with the args you exposed?** If you renamed `query_id` to just `query`, fix the PREAMBLE example.
- **Did pruning change the platform's "shape"?** A platform you described as "full SQL platform" might now be "read-mostly SQL runner" if you dropped the CRUD endpoints. Update the Role line.

The PREAMBLE is the LLM's only orientation to your tools. Drift between PREAMBLE and tool reality is the most common cause of LLMs picking the wrong tool.

### 6. Verify

```
cd apps/<platform> && cargo build
```

If it fails:
- Most likely cause: arg type mismatch with the generated client's positional API. Look at the client.rs signature for the method you called and adjust your `client.method(...)` call.
- Second most likely: missing `&` / `.as_str()` / `.as_deref()` for string args, since progenitor's API takes `&'a str` not `String`.
- Fix and re-run. Iterate until clean.

Optional smoke-chat with the curated tools before formalising the e2e test:

```
aomi-run target/debug/lib<platform>.dylib
```

`aomi-run` loads the cdylib, registers every tool from the manifest, and
opens a local REPL against the LLM (provider via `--provider`, key from
env). Worth doing at least once per app — you'll catch obvious naming /
description issues and unclear PREAMBLE wording faster than via the
formal e2e harness. Wallet-bound tools (`commit_tx`, `commit_eip712`,
`stage_tx`, etc.) are stubbed in `aomi-run`, so route-driven flows still
need the e2e test or the deployed backend.

### 6.5. Seed the e2e test

Write `apps/<platform>/test.json` with **just** the `user_story` portion. **One test per app** — the harness runs a single test; pick the highest-value happy path.

```json
{
  "user_story": "<one-sentence happy-path scenario the user might actually type>"
}
```

Pick the scenario that exercises the **most important composite tool** you just built:
- Swap/bridge: "Swap X token for Y on chain Z" (or bridge variant)
- Lending/yield: "Find the best APY for asset X and supply"
- Price oracle / data: "Get the current TVL of protocol X" or "Find the highest-APY pools for asset Y"
- Perps / CEX: "Show price+depth for X-PERP" or "Place a limit order to buy 1 ETH at $X"
- Prediction: "Search markets about topic X and place a YES bet on the top one"
- Social: "Find user @X and show their recent posts"

Don't write the full test (turns, expected_tools, final_assertion) — the `aomi-app-e2e-tester` skill expands those. Your job is just the seed.

If `apps/<platform>/test.json` already exists with a `user_story`, leave it alone — preserve any human/test-author refinement.

#### Example seed (DefiLlama)

```json
{
  "user_story": "Find the biggest lending protocol on Ethereum and show me its current TVL and recent trend"
}
```

### 7. Report

Print a tight summary:
- Number of tools before/after.
- Names of new composite tools (these are the highest-value additions).
- Names of dropped endpoints (and one-line reasons).
- One sentence on what the user can now do that they couldn't before.
- The seed user_story you wrote to `test.json`.

Suggest the user run **`/aomi-app-e2e-tester <platform>`** to expand the test and verify the app works end-to-end.

## Anti-patterns

- ❌ **Preserving all generated stubs.** Most APIs have 30–60+ ops. A useful Aomi app is usually 5–12 tools. Be aggressive about pruning.
- ❌ **Endpoint-derived names.** `dune_getv1_query_queryid_results_csv` tells the LLM nothing. `dune_get_query_results` does.
- ❌ **Generic descriptions.** "Get protocol detail" — useless. "Returns TVL, chains, and historical TVL series for a protocol slug. Use when the user asks about a specific protocol's size or trend." — useful.
- ❌ **Letting the LLM orchestrate composites.** If a user goal needs three endpoints, the tool wraps all three. Don't make the model figure out the sequence.
- ❌ **Exposing admin/internal endpoints.** If the spec includes `/admin/users`, `/internal/health`, etc., skip them.
- ❌ **Args structs that mirror the spec exactly.** The spec's params are an API surface; the user's mental model is different. Translate.
- ❌ **Skipping the user-intent question.** Without intent, tools will be generic and probably miss the point.
- ❌ **Touching `ext/src/<platform>/`.** Generated client is regenerated by `aomi-build gen-client`; never hand-edit it.
- ❌ **Asking the user (LLM) for values you can compute.** Timestamps, polling intervals, page sizes, default chains, retry budgets — derive these in Rust.
- ❌ **Adding `*Summary` projection structs in `tool.rs` to drop fields the LLM doesn't need.** Trim those fields **at the spec layer** instead — delete them from `ext/specs/<platform>.yaml` and `aomi-build gen-client --force`. The typed Rust struct then IS the slim shape, and `tool.rs` stays `ok(response)`. The exception (one endpoint serving multiple tool intents needing different shapes) is rare and needs justification.
- ❌ **Returning raw `{ approve_tx, swap_tx, status_url }` JSON for the LLM to orchestrate.** When an off-chain build returns a transaction (or signature payload) the user must sign, wrap the wallet step in a `ToolReturn::route(...)` chain with `host::StageTx` (or `host::CommitEip712` / `host::SignTxSolana`) and `.enforce(SimulateBatch + CommitTxs.bind_as("transaction_hash"))`. Use `.after::<NextTool>(args).awaits("transaction_hash")` for the off-chain confirm step. The LLM should never see "now stage these txs in this order" instructions in your tool description — that's exactly what the route handles. See "Wallet handoff" section above.
- ❌ **Wrapping `reqwest::Client` with auth middleware.** Even if `Client::new_with_client(...)` would let you, don't. Auth is per-call: resolve the secret in `tool.rs`, compute any signature via `apps/<platform>/src/auth.rs` helpers, and pass headers positionally into the generated client method. See "Auth: per-call shim, not middleware" above.
- ❌ **Declaring auth only under `securitySchemes`.** Progenitor ignores `securitySchemes` for arg generation — the header has to appear in each authed operation's `parameters:` list (typically via `$ref: '#/components/parameters/ApiKeyHeader'`). Otherwise the generated client has nowhere to plug the key in.
- ❌ **Reimplementing HMAC, base64, or timestamps in a per-app `auth.rs`.** Those primitives live in shared [ext/src/hmac_auth.rs](ext/src/hmac_auth.rs); the per-app shim owns only the venue-specific prehash format + quirks.
- ❌ **Exposing API keys / secrets as LLM-callable tool args.** The LLM should never see a credential. Resolve from env or SDK secrets store inside the tool body.
- ❌ **Unbounded loops.** Every poll loop has a deadline AND an iteration cap. No `loop { … sleep(2).await }` without an exit condition that fires on a wall-clock deadline.
- ❌ **PREAMBLE drift.** Don't write tool descriptions and forget to update PREAMBLE. After each tool change, re-read PREAMBLE and confirm it still describes what you shipped.

## Examples (illustrative, don't blindly copy)

### DefiLlama, intent = "track token prices and TVL trends"

**Before** (gen-tool stubs): 21 mechanical tools — `defillama_getProtocols`, `defillama_getProtocolDetail`, `defillama_getCurrentPrices`, `defillama_getHistoricalPrices`, `defillama_getPriceChangePercentage`, `defillama_getYieldPools`, …

**After** (curated): 6 tools

| Tool | Wraps | Description |
|---|---|---|
| `defillama_get_token_price` | `getCurrentPrices` | Use when the user asks the current price of a token. Accepts `chain:address` or `coingecko:id`. |
| `defillama_get_price_history` | `getHistoricalPrices` + `getPriceChangePercentage` | Returns price at a past timestamp and % change since. Use when the user asks "how much has X moved". |
| `defillama_list_protocols` | `getProtocols` | Lists DeFi protocols by current TVL, optionally filtered by category. |
| `defillama_get_protocol_tvl` | `getProtocolDetail` | TVL + historical series for one protocol. |
| `defillama_top_yield_pools` | `getYieldPools` | Top yield opportunities, filterable by chain/asset/min APY. |
| `defillama_get_chain_tvl` | `getChainsTvl` + `getHistoricalChainTvl` | Current and historical TVL for a chain. |

Dropped: `getStablecoinHistory`, `getStablecoinChains`, `getStablecoins`, `getDexOverview`, `getFeesOverview`, etc. — out of scope for the intent.

### LiFi, intent = "swap tokens cross-chain"

Likely curated tools: `lifi_get_quote`, `lifi_build_swap_tx` (composite: quote + approve + swap), `lifi_check_status`, `lifi_list_chains`, `lifi_list_tokens_on_chain`. Drop everything else.

## When to refuse

If the platform is not categorisable (no clear user stories), or if the user's intent doesn't fit the platform's actual API surface, surface this and ask the user to either:
- Reframe the intent
- Pick a different platform
- Accept that the existing generated stubs will stay until intent is clearer

Don't fabricate a "use case" to justify pretty tool names on a misfit API.
