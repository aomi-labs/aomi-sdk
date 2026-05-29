# Handover: Discord activation-request setup

This doc hands off the **`aomi-git activate --request`** Discord integration. When
a contributor publishes an app they don't have the activation token (only
platform ops do — ADR 0009). So instead of activating, they run:

```bash
aomi-git activate --request
```

…which posts a message into our Discord activation channel, tagging the ops
admin with the repo / app / release tag so ops can activate on their behalf.

The webhook URL is a **credential**, so it is *not* hardcoded — the CLI reads it
from an environment variable at runtime. This doc covers (1) setting up the
Discord channel + webhook, and (2) wiring those values in via env vars.

---

## Part 1 — Discord setup (one-time)

You need two things out of Discord: an **incoming webhook URL** (where the CLI
POSTs the message) and the **admin mention ID** (who gets pinged).

### 1.1 Pick / create the channel

Use the Aomi apps server (public invite: `https://discord.gg/VF5Zq8ddu`).
Create or choose a channel for activation requests, e.g. `#activation-requests`.
This is the single destination — the CLI only ever posts here.

### 1.2 Create an incoming webhook

1. **Server Settings → Integrations → Webhooks → New Webhook** (or, on the
   channel itself: **Edit Channel → Integrations → Webhooks → New Webhook**).
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
(recommended — survives people leaving) or a **single user**.

First enable Developer Mode so you can copy IDs:
**Settings (gear) → Advanced → Developer Mode → ON.**

- **For a role (recommended):** Server Settings → Roles → right-click the ops
  role → **Copy Role ID**. The mention token is `<@&ROLE_ID>`
  (note the `&`), e.g. `<@&123456789012345678>`.
- **For a user:** right-click the person → **Copy User ID**. The mention token
  is `<@USER_ID>` (no `&`), e.g. `<@123456789012345678>`.

This token is the value for `DISCORD_ADMIN` in the code.

### 1.4 (Optional) sanity-check the webhook by hand

Before touching code, you can confirm the webhook works with curl:

```bash
curl -X POST "https://discord.com/api/webhooks/<id>/<token>" \
  -H 'Content-Type: application/json' \
  -d '{"content":"<@&ROLE_ID> webhook test — please ignore","allowed_mentions":{"parse":["users","roles"]}}'
```

A `204 No Content` response and a message appearing in the channel (with the
role pinged) means you're good. If the ping doesn't fire, double-check the ID
and that you used `<@&...>` for a role vs `<@...>` for a user.

---

## Part 2 — Configuration (env vars)

The Discord logic lives in **one file** (`sdk/bin/git/discord.rs`), but you
**don't edit any code** — the webhook is a credential, so it's read from the
environment at runtime. There are two env vars:

| Env var | Required? | Value |
|---|---|---|
| `AOMI_DISCORD_WEBHOOK_URL` | yes (to post) | the webhook URL from step 1.2 |
| `AOMI_DISCORD_ADMIN_MENTION` | optional | the mention token from step 1.3 (`<@&ROLE_ID>` / `<@USER_ID>`) |

Set them wherever the person running `aomi-git activate --request` works —
your shell profile, a `.env`, or CI secrets:

```bash
export AOMI_DISCORD_WEBHOOK_URL='https://discord.com/api/webhooks/<id>/<token>'
export AOMI_DISCORD_ADMIN_MENTION='<@&123456789012345678>'   # optional ops-role ping
```

Behaviour:
- **Webhook set** → `aomi-git activate --request` posts to the channel (pinging
  the admin if the mention var is set).
- **Webhook unset** → the command tells you to run with `--dry-run` and post the
  message manually. It never fails silently.
- **Mention unset** → it still posts, just without an `@` ping.

> Treat `AOMI_DISCORD_WEBHOOK_URL` like any other secret — keep it out of
> committed files and public chats. The public invite link (`DISCORD_INVITE`
> in `discord.rs`) is *not* a secret and is already set correctly.

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

The leading `<@&ROLE_ID>` only appears when `AOMI_DISCORD_ADMIN_MENTION` is set;
otherwise the message posts without it. `allowed_mentions` is scoped to
`["users","roles"]` — the post **can** ping the admin role/user, but **can
never** ping `@everyone`/`@here`. Don't change that.

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
message **without** posting — use it to verify formatting:

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

1. Contributor: `aomi-git deploy` → pushes source, writes `.aomi/deployment.json`.
2. Contributor: `aomi-git status` → polls CI, tells them when the release is ready.
3. Contributor: `aomi-git activate --request` → posts to our Discord, pings ops.
4. Ops (you/devrel): see the ping, run the real `aomi-git activate <release> ...`
   with the activation token (which only ops hold).

Steps 2 and 3 are also printed automatically as "Next steps" after a successful
`aomi-git deploy`.

---

## Security notes

- **The webhook is a credential and is never committed.** It lives only in the
  `AOMI_DISCORD_WEBHOOK_URL` env var, so it never ships inside the published
  `aomi-sdk` binary or the source tree. Keep it out of committed files, issues,
  and public chats.
- **Rotation:** if the webhook leaks or gets abused, delete it in Discord
  (Part 1.2), create a new one, and update `AOMI_DISCORD_WEBHOOK_URL` wherever
  it's set. No code change or release needed.
- **Blast radius if leaked:** an incoming-webhook can only post into this one
  channel — no data access, no other channels, no server control. Worst case is
  spam, fixed by rotating.
- **Never widen `allowed_mentions`.** Keep it `["users","roles"]`; never add
  `everyone`.

---

## Quick reference

| Thing | Where |
|---|---|
| Discord code | `sdk/bin/git/discord.rs` |
| CLI flag impl | `ActivateArgs::request_activation` in `sdk/bin/git/cli.rs` |
| Webhook env var | `AOMI_DISCORD_WEBHOOK_URL` (required to post) |
| Admin mention env var | `AOMI_DISCORD_ADMIN_MENTION` (optional ping) |
| Public invite | `DISCORD_INVITE` in `discord.rs` (already set) |
| Command | `aomi-git activate --request` (`--dry-run` to preview) |
| Role mention format | `<@&ROLE_ID>` |
| User mention format | `<@USER_ID>` |
