# Repo Structure

The recommended layout for a new app crate is:

```text
apps/my-app/
├─ Cargo.toml
└─ src/
   ├─ lib.rs
   ├─ client.rs
   └─ tool.rs
```

## File Roles

- `lib.rs`
  - Defines the app preamble.
  - Registers the app with `dyn_aomi_app!`.
  - Keeps the manifest surface easy to scan.

- `client.rs`
  - Owns HTTP client setup, auth headers, data models, and response normalization.
  - Keeps third-party API details out of tool implementations.

- `tool.rs`
  - Implements `DynAomiTool`.
  - Contains typed tool args and user-facing descriptions.
  - Maps client responses into stable JSON results.

## Authoring Guidelines

- Prefer one app crate per external product or ecosystem.
- Keep tool args typed and documented with `JsonSchema`.
- Normalize upstream API errors into short actionable strings.
- Avoid leaking host-specific or private operational assumptions into prompts.
- If your app needs signing or execution, depend only on the public host conventions in `docs/host-interop.md`.
