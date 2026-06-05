---
name: aomi-app-client-api-gen
description: Draft an OpenAPI 3.0 spec for a third-party platform (e.g. Binance, Dune, DefiLlama) by reading its public documentation. By default writes to `apps/<platform>/openapi.yaml` (app-local mode); writes to `ext/specs/<platform>.yaml` when the platform is a shared library (Binance, Bybit, etc. — many Aomi apps may wrap it). Use when the user wants to add or update an Aomi app for a platform that does NOT have an official OpenAPI spec discoverable via `aomi-build gen-specs`. Output feeds `aomi-build gen-client` downstream. Triggers include "draft openapi spec for X", "/aomi-app-client-api-gen", "no spec found, run the skill", or asking to add an Aomi app for an API without a discoverable OpenAPI surface.
---

# aomi-app-client-api-gen

You are drafting an OpenAPI 3.1 spec for a third-party platform from its docs. The output feeds the `aomi-build gen-client` step downstream — quality matters because Rust client codegen will rely on it.

## Scope

This skill is the FALLBACK for when `aomi-build gen-specs <platform>` fails (no APIs.guru / GitHub / Postman match). It is for HTTP REST APIs. **Do not use it for**:

- GraphQL APIs (different paradigm — say so and stop)
- WebSocket-only APIs
- Pure on-chain integrations (no HTTP surface)
- Platforms whose entire API is one POST endpoint with a discriminated `type` field — these can technically be modelled with `oneOf` discriminators but the codegen output is ugly. Surface this to the user and ask if they want to proceed anyway or keep the client hand-written.

## Inputs you need

Required:
- `platform` — short name used as the file stem (e.g. `dune`, `defillama`).
- At least one source: a docs URL, a Postman collection JSON, or a TypeScript/Python SDK source path.

If the user only gave a platform name, ask for at least a docs URL before doing anything else. Do not guess URLs — confirm with the user.

Preferred sources, in order of quality:
1. **Official TS/Python SDK source** — types in the SDK encode the API surface more precisely than markdown ever will. If the user can point you at one, use it as primary.
2. **Postman collection** — already has structured request/response examples.
3. **Reference docs page** with curl examples.
4. **Free-form docs** — last resort, lowest quality, hardest to get right.

## Workflow

### 1. Plan the surface area

- Read the docs landing page to get the list of endpoints (or the SDK's exported methods).
- Group by resource (e.g. `/v1/markets/*`, `/v1/orders/*`).
- Ask the user which groups to include. Do NOT spec endpoints you weren't asked for — drafting 100 endpoints when the user only needs 5 is waste.

### 2. Draft iteratively, not all at once

For each endpoint group:
- Fetch the relevant doc page (WebFetch).
- Extract: HTTP method, path, path/query/body params (with types), response schema, auth requirement.
- Append to the spec. Do not try to draft the whole spec in one pass — iterate.

### 3. Auth and signing

OpenAPI's `securitySchemes` only describes *what header to send*, not *how to compute it*. Use the standard OpenAPI security types where they fit:
- Bearer token: `securitySchemes.bearerAuth = { type: http, scheme: bearer }`
- API key in header: `securitySchemes.apiKey = { type: apiKey, in: header, name: X-API-KEY }`

If the platform requires HMAC signing or any other computed credential (Binance, OKX, Polymarket-style), describe what header(s) to send in `securitySchemes` and add a top-level comment under `info.description` like:

> **For codegen to work**, the same header must also appear as an explicit `parameters:` entry on each authed operation (typically `$ref: '#/components/parameters/ApiKeyHeader'` etc.) — progenitor ignores `securitySchemes` when generating positional args. See **[docs/auth-practices.md](../../../docs/auth-practices.md)** for the wire-level recipes Aomi already supports (Binance / Bybit / OKX / Limitless / Krexa static-bearer) and how the spec needs to be shaped for each.


```
## Auth
This API uses HMAC-SHA256 over the query string with appended `&timestamp=...&signature=...`.
The signing logic is hand-written in `<auth-shim path>` — the spec only describes
the resulting header (`X-MBX-APIKEY`) for codegen purposes.
```

The auth shim path depends on mode: for app-local providers it's
`apps/<platform>/src/auth.rs`; for shared providers (--shared) it's
`ext/src/<platform>/auth.rs`.

Do not invent OpenAPI extensions unless absolutely necessary.

### 4. Validate before writing

Before writing the file, sanity-check yourself:
- Does it parse as YAML? (Mentally: are colons + indentation correct?)
- Does it have `openapi: 3.0.3`, `info: { title, version }`, and `paths`?
  **Use 3.0.3, not 3.1.x.** The downstream codegen (progenitor) only supports OpenAPI 3.0.x. If you write 3.1, `aomi-build gen-client` will downgrade it textually with a warning, but you should produce 3.0.3 directly.
- Does every operation have at least a 200 response with a schema?
- Are response schemas defined inline or referenced from `components/schemas`? Prefer `components/schemas` for any type used by more than one operation.
- Does every signed endpoint declare its `security` requirement?

### 5. Write the files

Choose mode first:

- **App-local (default)**: spec lives at `apps/<platform>/openapi.yaml`. Use this for one-off integrations or platforms whose client is unlikely to be shared across multiple Aomi apps. The whole crate (spec, generated client, tool layer) is then self-contained.
- **Shared (`--shared` downstream)**: spec lives at `ext/specs/<platform>.yaml`. Use this only for big-party platforms (Binance, Bybit, OKX) whose client is genuinely shared by multiple Aomi apps.

If you don't know, default to **app-local**. The user can always promote later by moving files and passing `--shared` to subsequent gen commands.

Write two files (paths below assume app-local mode; for shared, swap to `ext/specs/<platform>.{yaml,meta.json}`):

**`apps/<platform>/openapi.yaml`** — the spec itself.

**`apps/<platform>/openapi.meta.json`** — provenance sidecar with this exact shape:

```json
{
  "platform": "<platform>",
  "source": "skill",
  "source_url": "<primary docs URL or 'multiple'>",
  "fetched_at": "<ISO-8601 UTC timestamp>",
  "upstream_version": null,
  "completeness": {
    "endpoints_specced": <number>,
    "endpoints_skipped": <number>,
    "auth_described": <true|false>,
    "notes": "<one-liner about what's missing or rough>"
  }
}
```

The `completeness` block is skill-only — it tells future-you (and the human) what's incomplete so they don't trust the spec blindly.

Use `mkdir -p` on the parent dir as needed before writing.

### 6. Report back

Print a short summary: how many endpoints, what was skipped, what's rough. End with the suggested next command:

```
Next: aomi-build gen-client <platform>
```

## Naming components — avoid progenitor enum-name collisions

If you name a top-level component schema something a progenitor-generated inline enum is also likely to be named, the resulting client won't compile (panic in typify).

progenitor synthesizes inline enum types from `enum:` constraints on string fields. The generated names look like `<OwningType><FieldName>` (e.g. a field `notification_type` on a response collapses to a type literally called `NotificationType`).

To stay safe:
- **Don't name components** `*Type`, `*Status`, `*Kind`, `*Mode`, `*Role` if a string field of the same root word elsewhere in the spec has an `enum:` constraint. Either drop the `enum:` (use plain string) or rename the component (e.g. `Notification` → `NotificationKind` won't help; pick a more specific name like `NotificationCategory`).
- **Symptom**: progenitor panics during gen-client with a type-redeclaration error. The fix is to rename the component (and update its `$ref` sites). Note the rename in `completeness.notes`.

## Anti-patterns

- **Don't fabricate.** If the docs are unclear about a response field's type, mark it `type: object` with `additionalProperties: true` and note it in `completeness.notes`. Never guess.
- **Don't over-spec.** This is a starting point for codegen, not a contract. 80% accurate covering the endpoints the user wants is better than 95% accurate covering endpoints they don't.
- **Don't run `cargo build` or codegen here.** This skill ends at the spec file. The user will run `aomi-build gen-client` (and `gen-tool`) next.
- **Don't co-locate when shared, don't centralize when app-local.** A spec in the wrong place breaks gen-client's path resolution.

## Example invocations

```
/aomi-app-client-api-gen dune https://docs.dune.com/api-reference/overview
/aomi-app-client-api-gen defillama https://defillama.com/docs/api
/aomi-app-client-api-gen binance  # → you ask for docs URL
```
