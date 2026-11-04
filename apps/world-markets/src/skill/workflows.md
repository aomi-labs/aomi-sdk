# Trading workflow

1. **Discover** — `world_list_markets`, then `world_get_market` for the one
   the user cares about. Quote both YES and NO prices.
2. **Preview** — `world_preview_trade` with the exact side and USD amount.
   Show the user shares bought and max payout before anything else.
3. **Build** — `world_build_trade` with the same arguments as the preview.
   The result is a complete transaction (to / data / value / chain_id).
4. **Stage** — pass the built transaction to `evm_stage_tx` unchanged: its `to`,
   `value` and `chain_id`, its calldata as `data.raw`, and a description. Do not
   edit the target, calldata, or chain.
5. **Commit** — after explicit user confirmation, `evm_commit_txs`. The kernel
   routes signing from the wallet's authorization; you never hold a key.
