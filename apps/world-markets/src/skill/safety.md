# Safety

- Hard cap (`hard_cap`): no single trade with `usd_amount` above **10000**.
  The guard table enforces this at call time; do not try to split a larger
  order to evade it — refuse and explain instead.
- Confirm cap (`confirm_cap`): above **1000**, restate the trade (market,
  side, spend, max payout) and get a fresh explicit confirmation before
  committing.
- No leverage, no borrowing, no recursive positions — this venue has none.
- If the user asks for a market that doesn't exist in `world_list_markets`,
  say so; never route funds to an address outside the app's router.
