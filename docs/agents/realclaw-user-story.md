# RealClaw parity checklist — re-scored 2026-07-10

RealClaw is byreal's Telegram-native autonomous trading agent (self-hosted
Claude-Code-style runtime "openclaw", one server per user, Privy-managed
wallets, skills: `byreal-agent-skills` + `RealClaw-Skills` + `byreal-perps-cli`
+ `evm-cli`). **The business goal of this checklist: walk into the byreal
meeting able to say our Telegram bot covers every RealClaw user story —
without a per-user server.** End state = every box `[x]`.

Legend: `[x]` covered · `[~]` partial · `[ ]` gap.
**Proof key** (honesty flag): `M` = merged code · `L` = live-verified against
the real venue/API · `D` = demo'd end-to-end on a running stack. The pitch
needs `D` on the headline flows; most boxes below are `M`/`L` today —
converting the top flows to `D` is tracked as P0 demo work.

**Scoreboard: 31 [x] · 7 [~] · 7 [ ] of 45 — ~76% weighted** (was 18/12/15,
~53%, at first scoring on 2026-07-09). The 7 remaining gaps: 1 blocked by
byreal's own API (bbSOL — a concrete ask FOR the meeting), 4 are P2 apps
(kamino, rent-reclaim, consolidate, tier-persona), 2 are launchpad/binary
(low value, deliberately unpicked).

---

## 1. Onboarding & Identity

- [x] As a user, I can onboard entirely inside Telegram and get a working
  agent. — RealClaw: Telegram bot + BOOTSTRAP.md. **aomi: exceeds** —
  multi-tenant bring-your-own-bot registration
  (`bin/backend/src/handler/bot_registration.rs`) + Telegram mini-app wallet
  UI; RealClaw runs ONE bot per deployment. Proof: M (Telegram platform live
  in prod for existing flows).
- [x] As a user, I get an agent-managed wallet without touching a seed
  phrase. — `DbUser::create_wallet` (Privy managed, non-exportable). Proof: M.
- [~] As a user, the bot speaks my language automatically (EN/中文). —
  RealClaw: explicit detect at onboarding. aomi: the model handles either
  language natively but there's no explicit locale detection/persistence.
  Adequate in practice; unverified as a flow.
- [~] As a user, I'm guided to fund the agent wallet (deposit/bridge). —
  RealClaw: shared "agent-token" skill. aomi: `across`/`lifi` apps can bridge,
  but no guided deposit funnel in onboarding. FE funnel work (P1).

## 2. Custody & Signing

- [x] My keys are non-exportable and never leave the custodian. — Privy
  managed keys; apps never hold keys (kernel signs). Proof: M/L.
- [x] The agent can sign unattended when I've armed it. —
  `SigningMode::Auto` → `auto_sign_evm/svm` 🟢 lane. Proof: L (live SVM e2e on
  the AA branch; real EVM/SVM signs in prior sessions).
- [x] I can require human approval per transaction instead. —
  `SigningMode::HumanSync` → FE/Telegram sign panel. Proof: D (existing
  attended flows in prod).
- [x] I can hard-deny a key. — `SigningMode::Denied`, kernel-enforced at the
  signing gate. Proof: M (unit-tested fail-closed).

## 3. Autonomy & Triggers

- [x] I can schedule recurring actions ("DCA daily at 9am"). —
  `schedule_cron` + `AomiClock`; **anchored recurrence** (fires on
  `trigger_at + k·period`, no execution drift). Proof: M (merged in
  product-mono #785).
- [~] Recurring actions respect my local timezone across DST. — BE merged
  (`chrono-tz` local-wall-clock slots via `preferences.schedule.tz`);
  **client tz picker not built** → unset = UTC. FE follow-up.
- [x] The agent wakes on a CONDITION, not just a clock ("sell if SOL ≤
  $180"). — `wake_on_condition`: host-side guard read (no LLM), poll-sampled,
  claim/fire, never dead-letters on unmet guards. Proof: M + L (guard read
  proven against live byreal price JSON). D pending — headline demo item.
- [x] A standing watchdog re-alerts on re-cross, not every poll. — `armed`
  hysteresis on recurring condition jobs. Proof: M.
- [x] A fire can be gated by an agentic judgment ("...and sentiment is
  bearish"). — read-only judge thread on the same app, joined on exit status,
  fail-closed verdict. Proof: M (LLM-free enforcement unit-tested; judge
  answer quality is model-in-the-loop).
- [x] The agent notifies me proactively on Telegram when something fires. —
  cron/condition-fired threads report via telegram notifications. Proof: M.

## 4. Venue Coverage

- [x] byreal spot swaps (AMM + RFQ). — `byreal` app (now
  `aomi-labs/byreal-apps`). Proof: L (venue-broadcast lane live-verified in
  its build).
- [x] byreal perps on Hyperliquid (order/cancel/leverage + reads). — `byreal`
  app perps namespace. Proof: L (real $11 order placed+canceled during
  original build).
- [x] Open / increase / close byreal CLMM positions. — **`byreal-lp`** (new):
  zap-in open/increase, zap-out, position/pool/unclaimed reads. Proof: M + L
  (live quote→build contract verified; ephemeral position-NFT co-sign
  unit-proven; funded write smoke pending).
- [x] Single-token position ops (zap / auto-swap). — `byreal-lp`, byreal's
  AutoSwap router (~30s HMAC quote TTL handled by re-quote-in-write). Proof:
  M + L.
- [x] Copy a top farmer's position one-tap. — **`byreal-copy-trade`** (new):
  leaderboard → provider positions → mirror tick bounds → sized open. Proof:
  M + L (read side); execute shares byreal-lp's write lane.
- [x] Claim accrued LP rewards. — `byreal` app claim flow (v1 single-tx;
  multi-tx batching still a v2 note). Proof: M.
- [ ] bbSOL liquid staking. — **BLOCKED BY BYREAL**: their public API has
  only `stats` + `send-bbsol-tx` (submit-only; expects a client-built signed
  tx). No build/quote endpoint exists; covering this would mean
  reverse-engineering their stake program. **Concrete ask for the meeting:
  "expose a build-tx endpoint for bbSOL and we cover it in a day."**
- [x] Best-execution routed swaps (Jupiter). — **`jupiter`** app (new).
  Proof: M + L (live quote→swap contract verified).
- [ ] Kamino lending (deposit/borrow/repay). — P2; recon not started.
- [x] EVM transfers (ethereum, mantle). — aomi's full EVM stack + AA
  (4337/7702) **exceeds** RealClaw's evm-cli (transfers only). Proof: D (EVM
  is aomi's original prod surface).
- [ ] byreal launchpad / binary options. — deliberately unpicked (low
  strategy value); byreal-apps platform makes them a bounded add if asked.

## 5. Standing Strategies (RealClaw-Skills → `byreal-strategies` on byreal-apps)

All six recipes are **platform-owned** (byreal-apps repo, byreal's operator
key can edit them) and compile to `schedule_cron`/`wake_on_condition` args —
one durable row each, no per-user runtime. Proof status M; D pending deploy.

- [x] `dca` — cadence buys with budget/reserve/impact guardrails.
- [x] `idle-yield` — sweep idle stables into top byreal CLMM pool
  (byreal-lp).
- [x] `lp-copy-trading` — mirror a top farmer (byreal-copy-trade).
- [x] `watchdog` — standing price alert with hysteresis re-arm.
- [x] `lp-limit-order` — open/exit position when price crosses a level
  (wake_on_condition + byreal-lp).
- [x] `stable-yield-farm` — depeg-guarded stable farming (watchdog +
  byreal-lp).
- [ ] `tier-switch` (risk persona switch) — P2, pairs with persona work
  below.
- [ ] `rent-reclaim` — P2 utility app (close empty token accounts).
- [ ] `token-consolidate` — P2 (near-trivial now via `jupiter`).
- [ ] `kamino-credit` — with the Kamino app (P2).
- [~] `byreal-onboarding` — our bot registration exists; the strategy-picker
  funnel FE is the missing glue (P1).
- [x] `skill-review` (supply-chain gate for community skills) — **N/A,
  superior**: our apps are compiled, code-reviewed crates loaded by manifest,
  not runtime-installed scripts; authority is kernel-enforced regardless of
  app content.

## 6. Access Control & Guardrails

- [x] The agent can be made READ-ONLY (observe, never transact). — thread
  credentials: `ThreadInput.read_only` → `aomi.read_only` envelope → signing
  gate denies before key lookup (`effective = min(key mode, thread cred)`),
  sticky across children/recovery/app-swap. **Exceeds RealClaw** (their
  guardrails are prompt-level SOUL.md text). Proof: M (unit-tested,
  LLM-free).
- [x] Every write is gated by explicit approval or standing consent. —
  HumanSync sign panels; `PRE-AUTHORIZED` convention for armed automations;
  per-app confirmation gates in preambles. Proof: M/D.
- [~] Risk tiers (safe/balanced/aggressive) as a one-tap persona. — the
  underlying knobs (SigningMode × app grants × thread creds × recipe
  guardrails) are a **superset** of RealClaw's tiers, but there's no packaged
  tier selector yet. P2 packaging work, not infra.
- [~] Hard spend caps per period. — guardrails live in recipe intent rules
  (read balance, cap per run); kernel-level per-tx/per-period caps
  (cgroup-style budgets) are a designed follow-up, not built.
- [x] Keys isolated from app/venue code. — apps never see keys; venue apps
  emit route plans, kernel signs. Proof: M (architecture) + L.

## 7. Reporting & Telemetry

- [x] I get a Telegram message with the result of every autonomous action. —
  fired-thread reports via notifications. Proof: M.
- [~] I can ask for my P&L / performance. — positions + unclaimed + balances
  are all readable so the agent can compose a report on demand, but there's
  no standing P&L ledger/cost-basis tracking (RealClaw's "grow the balance"
  loop). P2.
- [x] Operational telemetry for the operator. — metrics/tracing on the
  backend; byreal-apps platform CI + release pipeline. (RealClaw's
  Sensors/神策 is growth analytics — different goal, deliberately not
  copied.)

## 8. Multi-agent & Runtime

- [x] The agent can spawn sub-agents for side tasks. — `spawn_thread` +
  cron/condition children. Proof: M/D.
- [x] A parent can JOIN a child and branch on its result. — `thread_return`
  exit status + join (the judge is the first consumer). **Exceeds openclaw**
  (no equivalent). Proof: M.
- [x] One deployment serves all users. — multi-tenant kernel; strategies are
  durable rows on a shared clock. **The** structural edge over
  RealClaw's server-per-user. Proof: D (existing prod).
- [x] No user-supplied LLM key needed. — aomi backend provides the model
  (BYOK optional). Proof: D.
- [x] A second bot/personality can serve the same account (Hermes-clone
  story). — bring-your-own-bot registration supports multiple bots; same
  account, different personality per bot. Different mechanism, same user
  story. Proof: M.
- [x] Vendor can own/edit their agent's skills (openclaw skills ecosystem).
  — byreal-apps platform repo: byreal's team edits preambles/recipes in
  their own repo, deploys via platform CI, no aomi release needed. Proof: M
  (repo staged; somm-finance-apps precedent in prod).

---

## What stands between this table and the demo (`D` on headline flows)

1. **Deploy the stack** — merge magical-maxwell → main, push byreal-apps,
   register the app dylibs (deferred by decision 2026-07-10: local-first).
2. **Funded-wallet write smoke** — one real zap-in open on byreal-lp + one
   jupiter swap (proves the two new write lanes incl. NFT co-sign).
3. **Autonomy demo** — arm a watchdog + a DCA on a real Telegram bot; show
   the fire → judge → auto-sign → Telegram report loop with no human.
4. **The pitch ask for byreal**: expose bbSOL build-tx (closes their last
   product surface); optionally hand them byreal-apps repo ownership.

*Re-scored from the 2026-07-09 baseline after: product-mono #785 (condition
wakes, exit-status/join, thread creds, judge, watchdog hysteresis, DST
recurrence, DB migrations applied), byreal-apps platform repo (byreal,
byreal-lp, byreal-copy-trade, byreal-strategies), and the aomi-sdk `jupiter`
app.*
