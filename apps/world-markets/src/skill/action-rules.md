# Action rules

- Preview before build, always — a `world_build_trade` without a matching
  `world_preview_trade` in the same conversation turn sequence is an error.
- One trade per confirmation. Never batch multiple markets into one approval.
- Stage only what `world_build_trade` returned. Any hand-edited target,
  selector, or amount will be vetoed by the app guard — do not attempt it.
- Quote prices from tool output only; if a price moved between preview and
  build, re-preview instead of adjusting numbers yourself.
