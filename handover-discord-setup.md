# Handover: Discord activation-request setup

This doc hands off the **`aomi-git activate --request`** Discord integration. When
a contributor publishes an app they don't have the activation token (only
platform ops do, per ADR 0009). So instead of activating, they run:

```bash
aomi-git activate --request
```

This posts a message into our Discord activation channel, tagging the ops
admin with the repo / app / release tag so ops can activate on their behalf.

The Discord destination is intentionally **code-owned**. Contributors should
not configure where activation requests go. This doc covers (1) setting up the
Discord channel + webhook, and (2) pasting those values into the CLI constants.

---

## Part 1 - Discord setup (one-time)

You need two things out of Discord: an **incoming webhook URL** (where the CLI
POSTs the message) and the **admin mention ID** (who gets pinged).

### 1.1 Pick / create the channel

Use the Aomi apps server (public invite: `https://discord.gg/VF5Zq8ddu`).
Create or choose a channel for activation requests, e.g. `#activation-requests`.
This is the single destination; the CLI only ever posts here.

### 1.2 Create an incoming webhook

1. **Server Settings > Integrations > Webhooks > New Webhook** (or, on the
   channel itself: **Edit Channel > Integrations > Webhooks > New Webhook**).
2. Name it something obvious like `aomi-git activation requests`.
3. Set its channel to the one from 1.1.
4. Click **Copy Webhook URL**. It looks like:
   ```
   https://discord.com/api/webhooks/<id>/<token>
   ```
   This is the value for `DISCORD_WEBHOOK` in the code (Part 2).

> A webhook can only **post into that one channel**. It can't read messages,
> can't join servers, can't do anything else. That's why we're comfortable
> hardcoding it (see "Security" below).

### 1.3 Get the admin mention ID

The message pings an admin so ops actually see it. You can ping a **role**
(recommended, because it survives people leaving) or a **single user**.

First enable Developer Mode so you can copy IDs:
**Settings (gear) > Advanced > Developer Mode > ON.**

- **For a role (recommended):** Server Settings > Roles > right-click the ops
  role > **Copy Role ID**. The mention token is `<@&ROLE_ID>`
  (note the `&`), e.g. `<@&123456789012345678>`.
- **For a user:** right-click the person > **Copy User ID**. The mention token
  is `<@USER_ID>` (no `&`), e.g. `<@123456789012345678>`.

This token is the value for `DISCORD_ADMIN` in the code.

### 1.4 (Optional) sanity-check the webhook by hand

Before touching code, you can confirm the webhook works with curl:

```bash
curl -X POST "https://discord.com/api/webhooks/<id>/<token>" \
  -H 'Content-Type: application/json' \
  -d '{"content":"<@&ROLE_ID> webhook test - please ignore","allowed_mentions":{"parse":["users","roles"]}}'
```

A `204 No Content` response and a message appearing in the channel (with the
role pinged) means you're good. If the ping doesn't fire, double-check the ID
and that you used `<@&...>` for a role vs `<@...>` for a user.

---

## Part 2 - Code setup

The Discord logic lives in **one file**: `sdk/bin/git/discord.rs`.

Replace these constants before shipping:

```rust
const DISCORD_WEBHOOK: &str = "https://discord.com/api/webhooks/<id>/<token>";
const DISCORD_ADMIN: &str = "<@&123456789012345678>";
```

Behaviour:
- `aomi-git activate --request` posts to that channel and pings `DISCORD_ADMIN`.
- `aomi-git activate --request --dry-run` prints the exact message without
  posting.
- If either constant still contains `REPLACE_ME`, the real post path fails with
  an explicit error instead of silently posting nowhere.

The public invite link (`DISCORD_INVITE` in `discord.rs`) is not a secret and is
already set correctly.

### 2.1 What the message looks like

The CLI builds and posts this (formatted in `ActivationRequest::message`):

```
<@&ROLE_ID> **Activation requested**
- app: `my-bot`
- repo: `aomi-labs/community-apps`
- release: `apps-my-bot-abc1234`
- target tags: `staging`
Please activate when you have a chance.
```

`allowed_mentions` is scoped to `["users","roles"]`, so the post can ping the
admin role/user but can never ping `@everyone`/`@here`. Don't change that.

### 2.2 Build + test

From the repo root (`/Users/cecilia/Code/aomi-sdk`):

```bash
cargo build  --features cli --bin aomi-git
cargo test   --features cli --bin aomi-git
cargo clippy --features cli --bin aomi-git
```

The relevant tests are `discord::tests::*` (message format + mention scoping)
and `tests::activate_request_*` (the CLI flow). All should stay green.

### 2.3 End-to-end test of the real command

`--dry-run` resolves everything from `.aomi/deployment.json` and prints the
message **without** posting - use it to verify formatting:

```bash
# from inside an app dir that has already run `aomi-git deploy`
aomi-git activate --request --dry-run
```

Then drop `--dry-run` to actually post to Discord:

```bash
aomi-git activate --request
```

On success it prints `Posted activation request for `<release>` to the Aomi
apps Discord.` and the message appears in the channel with the admin pinged.

If you don't have a real deployed app handy, you can hand-craft a
`.aomi/deployment.json` via `aomi-git deploy --dry-run` in any app dir, which
writes the state file without pushing.

---

## How it fits together (the contributor flow)

1. Contributor: `aomi-git deploy` pushes source and writes `.aomi/deployment.json`.
2. Contributor: `aomi-git status` polls CI and tells them when the release is ready.
3. Contributor: `aomi-git activate --request` posts to our Discord and pings ops.
4. Ops (you/devrel): see the ping, run the real `aomi-git activate <release> ...`
   with the activation token (which only ops hold).

Steps 2 and 3 are also printed automatically as "Next steps" after a successful
`aomi-git deploy`.

---

## Security notes

- **The webhook ships inside the published binary.** `aomi-sdk` is a published
  crate, so the hardcoded `DISCORD_WEBHOOK` is extractable by anyone who
  installs the CLI. This is intentional for this integration.
- **Rotation:** if the webhook leaks or gets abused, delete it in Discord
  (Part 1.2), create a new one, paste it into `DISCORD_WEBHOOK`, and publish a
  new CLI build.
- **Blast radius if leaked:** an incoming webhook can only post into this one
  channel - no data access, no other channels, no server control. Worst case is
  spam, fixed by rotating.
- **Never widen `allowed_mentions`.** Keep it `["users","roles"]`; never add
  `everyone`.

---

## Quick reference

| Thing | Where |
|---|---|
| Discord code | `sdk/bin/git/discord.rs` |
| CLI flag impl | `ActivateArgs::request_activation` in `sdk/bin/git/cli.rs` |
| Webhook constant | `DISCORD_WEBHOOK` in `discord.rs` |
| Admin mention constant | `DISCORD_ADMIN` in `discord.rs` |
| Public invite | `DISCORD_INVITE` in `discord.rs` (already set) |
| Command | `aomi-git activate --request` (`--dry-run` to preview) |
| Role mention format | `<@&ROLE_ID>` |
| User mention format | `<@USER_ID>` |
