# lp-manager research — Uniswap v3 LP mechanics on Ethereum

Phase-1 research for an Aomi app that manages Uniswap v3 liquidity positions on
Ethereum mainnet: protocol mechanics, contract entry points, risks, the
pool-data surfaces available in this SDK (geckoterminal / defillama / dune),
and the concrete user actions the agent should support.

Sources are cited inline. Contract signatures were verified against the
Uniswap v3 interface sources on GitHub; addresses against the official
deployments page.

---

## 1. Core mechanism: concentrated liquidity

Uniswap v3 replaces v2's uniform x·y=k curve with **concentrated liquidity**:
each LP supplies liquidity only inside a chosen price range `[tickLower,
tickUpper)`. Because positions are heterogeneous, they are non-fungible — each
is an ERC-721 NFT minted by the periphery `NonfungiblePositionManager` ("NPM").
([Uniswap v3 whitepaper](https://app.uniswap.org/whitepaper-v3.pdf))

### Ticks and price representation

- Price space is divided into **ticks**: `price(i) = 1.0001^i`, i.e. each tick
  is 1 basis point of price. Tick range is `MIN_TICK = -887272` to
  `MAX_TICK = 887272` (covering prices 2⁻¹²⁸ … 2¹²⁸).
  ([Uniswap v3 math primer](https://blog.uniswap.org/uniswap-v3-math-primer),
  [Uniswap support: what is a tick](https://support.uniswap.org/hc/en-us/articles/21069524840589-What-is-a-tick-when-providing-liquidity))
- On-chain price is stored as `sqrtPriceX96 = sqrt(token1/token0) * 2^96`
  (Q64.96 fixed point). Convert: `price = (sqrtPriceX96 / 2^96)^2`, then adjust
  for decimals: human price of token0 in token1 =
  `price * 10^(decimals0 - decimals1)`.
  ([math primer](https://blog.uniswap.org/uniswap-v3-math-primer))
- `tick = floor(log_1.0001(price))`; `sqrtPriceX96 = sqrt(1.0001^tick) * 2^96`.
- **token ordering**: in every pool `token0 < token1` by address (lexicographic
  on the 20-byte value). All prices/ticks are token1-per-token0. The agent must
  sort user-supplied token pairs before building calls.

### Fee tiers and tick spacing

Positions may only start/end on ticks divisible by the pool's `tickSpacing`,
fixed per fee tier in `UniswapV3Factory.feeAmountTickSpacing`
([RareSkills: tick spacing and fees](https://rareskills.io/post/uniswap-v3-tick-spacing)):

| fee (uint24) | fee %  | tickSpacing | typical use              |
|--------------|--------|-------------|--------------------------|
| 100          | 0.01%  | 1           | stable-stable (USDC/USDT)|
| 500          | 0.05%  | 10          | correlated (ETH/stables) |
| 3000         | 0.30%  | 60          | standard volatile pairs  |
| 10000        | 1.00%  | 200         | exotic / long-tail       |

Governance can enable more tiers via `factory.enableFeeAmount`. One pool exists
per (token0, token1, fee) triple; discover it with
`factory.getPool(tokenA, tokenB, fee)` (returns `address(0)` if not created).

### Liquidity math (what the agent needs for previews)

For a position with liquidity `L` over `[sqrtPa, sqrtPb]` at current `sqrtP`
(all Q64.96), token amounts are
([math primer pt. 2 / v3 whitepaper §6](https://blog.uniswap.org/uniswap-v3-math-primer)):

- price below range (`sqrtP ≤ sqrtPa`): all token0,
  `amount0 = L * (sqrtPb - sqrtPa) / (sqrtPa * sqrtPb)`
- price in range: `amount0 = L * (sqrtPb - sqrtP) / (sqrtP * sqrtPb)`,
  `amount1 = L * (sqrtP - sqrtPa)`
- price above range (`sqrtP ≥ sqrtPb`): all token1,
  `amount1 = L * (sqrtPb - sqrtPa)`

Consequences:
- The deposit ratio is dictated by where the current price sits inside the
  range — the agent should compute the expected token0:token1 split *before*
  asking the user for amounts, and warn when a range is one-sided.
- An out-of-range position is 100% one asset and earns **zero fees** until
  price re-enters the range.

### Fee accounting

Swap fees accrue to `feeGrowthGlobal{0,1}X128` (per unit of liquidity, Q128).
Each tick tracks `feeGrowthOutside`; a position's uncollected fees are derived
from `feeGrowthInside` deltas since the position's last touch, then credited to
`tokensOwed{0,1}` when the position is next modified ("poked"). Fees do **not
auto-compound** — they sit as tokensOwed until `collect` is called.
([IUniswapV3PoolState.sol](https://github.com/Uniswap/v3-core/blob/main/contracts/interfaces/pool/IUniswapV3PoolState.sol),
whitepaper §6.3)

Practical read path for "how much fees have I earned": the NPM's stored
`tokensOwed` is stale until a poke, so the standard trick is an `eth_call`
**static simulation of `collect`** with `amount0Max = amount1Max =
type(uint128).max` — the returned `(amount0, amount1)` is the live uncollected
total without sending a transaction. (Alternative: recompute from
`pool.positions(keccak256(abi.encodePacked(NPM, tickLower, tickUpper)))` +
`feeGrowthInside`, but the static-call is simpler and what most dashboards do.)

---

## 2. Key contracts and entry points (Ethereum mainnet)

Addresses from the official deployments page
([developers.uniswap.org — Ethereum deployments](https://developers.uniswap.org/contracts/v3/reference/deployments/ethereum-deployments));
Uniswap warns addresses are **not identical across chains** — resolve per chain,
never hardcode cross-chain.

| Contract | Address | Role for lp-manager |
|---|---|---|
| UniswapV3Factory | `0x1F98431c8aD98523631AE4a59f267346ea31F984` | `getPool(tokenA,tokenB,fee)` pool discovery |
| **NonfungiblePositionManager** | `0xC36442b4a4522E871399CD717aBDD847Ab11FE88` | all position writes + `positions()` reads ([Etherscan](https://etherscan.io/address/0xc36442b4a4522e871399cd717abdd847ab11fe88)) |
| TickLens | `0xbfd8137f7d1516D3ea5cA83523914859ec47F573` | batched `ticks()` reads (liquidity distribution around current price) |
| QuoterV2 | `0x61fFE014bA17989E743c5F6cB21bF9697530B21e` | swap quotes (only needed if the app also rebalances via swap) |
| SwapRouter02 | `0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45` | swaps during rebalance (optional, v1 scope can omit) |
| WETH9 | `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` | NPM's wrapped-native token (`NPM.WETH9()`) |

### NonfungiblePositionManager — write entry points

Verified against
[INonfungiblePositionManager.sol](https://github.com/Uniswap/v3-periphery/blob/main/contracts/interfaces/INonfungiblePositionManager.sol).
All four are `payable` (to support native-ETH flows via multicall).

```solidity
struct MintParams {
    address token0; address token1; uint24 fee;
    int24 tickLower; int24 tickUpper;
    uint256 amount0Desired; uint256 amount1Desired;
    uint256 amount0Min; uint256 amount1Min;   // slippage floor
    address recipient; uint256 deadline;
}
function mint(MintParams) external payable
    returns (uint256 tokenId, uint128 liquidity, uint256 amount0, uint256 amount1);

struct IncreaseLiquidityParams {
    uint256 tokenId;
    uint256 amount0Desired; uint256 amount1Desired;
    uint256 amount0Min; uint256 amount1Min; uint256 deadline;
}
function increaseLiquidity(IncreaseLiquidityParams) external payable
    returns (uint128 liquidity, uint256 amount0, uint256 amount1);

struct DecreaseLiquidityParams {
    uint256 tokenId; uint128 liquidity;       // liquidity units to burn, NOT token amounts
    uint256 amount0Min; uint256 amount1Min; uint256 deadline;
}
function decreaseLiquidity(DecreaseLiquidityParams) external payable
    returns (uint256 amount0, uint256 amount1); // credited to tokensOwed, NOT transferred

struct CollectParams {
    uint256 tokenId; address recipient;
    uint128 amount0Max; uint128 amount1Max;   // pass type(uint128).max for "all"
}
function collect(CollectParams) external payable
    returns (uint256 amount0, uint256 amount1); // actually transfers tokens

function burn(uint256 tokenId) external payable; // requires liquidity == 0 && tokensOwed == 0
```

Critical flow detail: **`decreaseLiquidity` does not send tokens** — it only
moves principal into `tokensOwed`. A withdrawal is always
`decreaseLiquidity` → `collect` (usually batched in one `multicall`). Full exit
is `decreaseLiquidity(all)` → `collect(max)` → `burn(tokenId)`.

Helpers (periphery `Multicall` / `PeripheryPayments`):
- `multicall(bytes[] data)` — batch several NPM calls in one tx (the canonical
  way to do decrease+collect, or mint with ETH).
- `refundETH()`, `unwrapWETH9(amountMin, recipient)`, `sweepToken(token,
  amountMin, recipient)` — native-ETH ergonomics. To deposit ETH:
  `multicall([mint, refundETH])` with `msg.value`; to withdraw as ETH:
  `collect` to the NPM itself (recipient = address(0) routes to the manager),
  then `unwrapWETH9` + `sweepToken` for the non-WETH side.
- ERC-721 surface: `balanceOf(owner)`, `tokenOfOwnerByIndex(owner, i)`
  (Enumerable) — the on-chain way to enumerate a wallet's position tokenIds;
  `ownerOf(tokenId)` to verify custody before staging writes.

Approvals: user must `approve(NPM, amount)` on **both** ERC-20s before
`mint`/`increaseLiquidity`. No approval is needed for decrease/collect/burn
(NFT ownership gates those).

### Read entry points

`NPM.positions(tokenId)` returns
([interface](https://github.com/Uniswap/v3-periphery/blob/main/contracts/interfaces/INonfungiblePositionManager.sol)):

```
(uint96 nonce, address operator, address token0, address token1, uint24 fee,
 int24 tickLower, int24 tickUpper, uint128 liquidity,
 uint256 feeGrowthInside0LastX128, uint256 feeGrowthInside1LastX128,
 uint128 tokensOwed0, uint128 tokensOwed1)
```

Pool state
([IUniswapV3PoolState.sol](https://github.com/Uniswap/v3-core/blob/main/contracts/interfaces/pool/IUniswapV3PoolState.sol)):

```
slot0() → (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex,
           uint16 observationCardinality, uint16 observationCardinalityNext,
           uint8 feeProtocol, bool unlocked)
liquidity() → uint128                       // in-range liquidity only
feeGrowthGlobal0X128() / feeGrowthGlobal1X128() → uint256
ticks(int24) → (uint128 liquidityGross, int128 liquidityNet,
                uint256 feeGrowthOutside0X128, uint256 feeGrowthOutside1X128,
                int56 tickCumulativeOutside, uint160 secondsPerLiquidityOutsideX128,
                uint32 secondsOutside, bool initialized)
positions(bytes32 key) → (uint128 liquidity, feeGrowthInside{0,1}LastX128,
                          tokensOwed{0,1})   // key = keccak(owner,tickLower,tickUpper); owner = NPM for NPM positions
observations(uint256) → (blockTimestamp, tickCumulative, …)  // TWAP oracle
```

The minimum read set for "show my position": `positions(tokenId)` +
`slot0()` of the pool (compare `slot0.tick` against `[tickLower, tickUpper)` for
in-range status) + static-call `collect(max)` for live uncollected fees +
amount math from §1 for principal value, then spot USD prices for valuation.

---

## 3. Risks the agent must surface

1. **Impermanent/divergence loss, amplified.** Concentration multiplies both
   fee income and IL versus v2; a narrow range behaves like a leveraged LP. If
   price exits the range the position converts entirely into the depreciating
   asset and stops earning. The agent should always state the range width, the
   break-even fee assumption, and what happens at the range edges.
2. **Out-of-range ≠ paused safely.** It means 100% one-sided inventory and 0
   fees; "rebalancing" back in realizes the loss. Never present rebalancing as
   free.
3. **Slippage/MEV on mint & increase.** The pool price can be moved in the
   same block; `amount0Min`/`amount1Min` must be computed from a trusted price
   (default ~0.5–1% below expected amounts) and `deadline` set (e.g. now+300s).
   Zero-min mints are a known sandwich target.
4. **`slot0` is spot and manipulable.** Fine for display, not for on-chain
   decision logic. If the app ever needs a robust price, use the pool's
   `observe`-based TWAP or cross-check the GeckoTerminal price off-chain and
   refuse to proceed on large divergence.
5. **Fee-on-transfer / rebasing tokens are not supported** by the v3 periphery
   (transfers of exact computed amounts are assumed); mints will revert or
   mis-account. Warn on exotic tokens.
6. **Ordering/decimals foot-guns.** token0/token1 order is by address, so the
   "price" may be the inverse of what the user thinks (e.g. USDC/WETH on
   mainnet has USDC = token0). All amount fields are raw base units.
7. **Lifecycle invariants.** `burn` reverts unless liquidity and tokensOwed are
   both zero; `collect` after `decreaseLiquidity` must be in the same multicall
   or a later tx; `deadline` in the past reverts everything.
8. **Approval hygiene.** Approvals are to the NPM only (`0xC364…FE88`); the
   agent should never stage approvals to pool addresses or routers it didn't
   derive from the deployments table, and should prefer exact-amount approvals.
9. **NFT custody.** Transferring the position NFT transfers the whole position;
   `operator`/`approve` on the NFT delegates full control. Check `ownerOf`
   before staging writes for a tokenId the user pasted.

---

## 4. Mapping pool-data needs onto existing SDK apps

The manager needs four data surfaces: **pool discovery/trending**, **pool
economics (TVL/volume/fees→yield)**, **position-level & historical analytics**,
and **live on-chain state**. The first three map onto existing apps
(kaito excluded per scope):

### apps/geckoterminal — discovery, market stats, monitoring

10 curated read-only tools over the public GeckoTerminal API
(`apps/geckoterminal/src/tool.rs`): `list_networks` (network + DEX ids —
`uniswap_v3` is a DEX id on network `eth`), `get_trending_pools`,
`get_top_pools` (per network or per DEX, sort by 24h volume/tx count),
`get_new_pools`, `search_pools` (symbol/pair/address → pool address),
`get_pool` (price, 24h/6h/1h volume with buy/sell split, `reserve_usd`
liquidity, FDV, price changes), `get_pool_ohlcv`, `get_pool_trades`,
`get_token` (profile + top pools), `get_token_price` (batch spot, up to 30).

Covers: "find me a pool", trending/new-pool scouting scoped to
`dex=uniswap_v3`, per-pool volume+TVL for **naive fee-APR estimation**
(`24h_volume × fee_tier / reserve_usd × 365` — a *pool-level* average that
understates what a concentrated in-range position earns and ignores IL; label
it as an estimate), price history for range-picking, spot prices for position
valuation.

### apps/defillama — TVL, yields/APY

6 tools (`apps/defillama/src/tool.rs`): `get_token_price`,
`get_price_history` (historical price + % change), `list_protocols`,
`get_protocol_tvl` (uniswap-v3 protocol TVL + per-chain series),
`top_yield_pools` (yields.llama.fi `/pools`, filter `project="uniswap-v3"`,
`chain="Ethereum"`, min-TVL; returns per-pool APY — DefiLlama computes
`apyBase` from trailing fees — plus stablecoin/IL-risk flags), `get_chain_tvl`.

Covers: measured (not estimated) fee APY per Uniswap v3 pool, cross-protocol
yield comparison ("is this better than Aave?"), protocol/chain health.
**Caveat found in code:** `top_yield_pools`'s description points to
`defillama_get_yield_pool_history` for APY time-series, but that tool is
**not registered** in `apps/defillama/src/lib.rs` — historical APY for a pool
is currently unavailable through the app (gap G4 below). Also note DefiLlama
yields identifies pools by its own UUID, not contract address; matching a
GeckoTerminal pool to a DefiLlama pool entry requires symbol+project+chain
heuristics.

### apps/dune — custom onchain analytics

5 tools (`apps/dune/src/tool.rs`): `run_query` (saved query by id),
`run_sql` (ad-hoc SQL — the workhorse), `get_latest_results`,
`get_execution_status`, `list_my_queries`.

Covers everything the curated APIs can't, via SQL over decoded tables
(`uniswap_v3_ethereum.*`): wallet's historical mints/burns/collects, realized
fee earnings per position, liquidity distribution near the current tick,
volume-in-range for honest range-specific yield estimates, LP-cohort
performance. Costs credits and has minutes-level freshness — use for analysis,
never for pre-trade state.

### Live on-chain state — via the `evm-core` host namespace, not a data app

All three data apps are indexed/off-chain; none can read `positions(tokenId)`,
`slot0`, or live uncollected fees at tx-building time. The SDK's host targets
(`sdk/src/builder.rs`) provide the write path used by peer apps (see
`apps/across`): `host::StageTx` with host-side ABI encoding via `data.encode`,
then `host::SimulateBatch` → `host::CommitTxs` as routed continuations, plus
`view_state`, `run_tx`, `get_contract`, `get_account_info`. Note the across
precedent (`apps/across/src/tool.rs`): it approves unconditionally because
there was no host `EthCall` target for an allowance pre-check — lp-manager
must verify whether `view_state`/`run_tx` can serve arbitrary `eth_call`
reads (positions/slot0/static-collect); if not, that's the biggest gap (G1).

### Gaps not covered by geckoterminal + defillama + dune

- **G1 — live position reads**: `positions(tokenId)`, `slot0`, static-call
  `collect` for uncollected fees, `balanceOf`/`tokenOfOwnerByIndex`
  enumeration. Must come from host EVM reads (or embedded RPC calls in the
  app's client); confirm the host read surface early.
- **G2 — tick-level liquidity distribution** (for suggesting ranges):
  GeckoTerminal/DefiLlama don't expose it; Dune can approximate (delayed);
  live source is `TickLens`/`pool.ticks()` on-chain.
- **G3 — range-specific yield**: both GT (naive volume×fee/TVL) and DefiLlama
  (pool-average apyBase) understate/average away concentration. A Dune query
  over in-range volume is the honest offline answer; otherwise label
  estimates clearly.
- **G4 — pool APY history**: referenced-but-missing
  `defillama_get_yield_pool_history`; either add it to the defillama app or
  serve via Dune.
- **G5 — GT↔DefiLlama pool identity join** (address vs UUID) needs heuristics
  in the lp-manager tool layer.

---

## 5. Concrete user actions the Aomi agent should support

Read/analytics (compose the three data apps + host reads):

1. **Find pools** — "best USDC/ETH pool", "trending v3 pools":
   `geckoterminal_get_top_pools(network=eth, dex=uniswap_v3)` /
   `search_pools`, enriched with `defillama_top_yield_pools(project=
   uniswap-v3)` APY; present fee tier, TVL, 24h volume, measured APY.
2. **Estimate yield for a candidate range** — pool stats (GT) + fee-APR math +
   OHLCV volatility → "range covers X% of last 30d closes"; optionally a Dune
   in-range-volume query for a serious answer. Always alongside IL caveat.
3. **List my positions** — NPM enumeration (host read or Dune fallback on
   Transfer events) → tokenIds.
4. **Position health report** — `positions(tokenId)` + `slot0` + static
   `collect` + spot prices (GT/DefiLlama) → principal split, USD value,
   uncollected fees, in/out-of-range, distance-to-edge in ticks and %.

Writes (staged through host `stage_tx` + `data.encode`, simulate, commit —
the across pattern; every flow previews amounts, mins, and deadline before
signing):

5. **Open position** — resolve pool via `factory.getPool`; order tokens; align
   ticks to `tickSpacing`; compute deposit ratio; stage `approve(token0)`,
   `approve(token1)`, `NPM.mint(MintParams)` (or multicall+`refundETH` for
   ETH); report tokenId + actual amounts from the receipt.
6. **Add liquidity** — `increaseLiquidity` on an owned tokenId (re-check
   allowances, same slippage discipline).
7. **Collect fees** — `collect(tokenId, recipient=user, max, max)`; optionally
   the unwrap-to-ETH multicall variant.
8. **Reduce / exit** — `multicall([decreaseLiquidity(liquidity·pct, mins,
   deadline), collect(max)])`; full exit appends `burn(tokenId)` only when
   both drains are complete.
9. **Rebalance (v1: guided, not atomic)** — exit (8) then open (5) around the
   current tick; state realized IL explicitly. Atomic swap-assisted rebalance
   via SwapRouter02 is a v2 candidate.
10. **Monitor** — on demand ("did my position drift?") via (4); recurring
    alerts can ride the platform's cron/loop rather than app code.

Preamble must encode the §3 invariants: token ordering, tick alignment,
decrease→collect pairing, burn preconditions, min-amount slippage floors,
deadlines, NPM-only approvals, fee-on-transfer warning, and "estimates are
pool-average, your range differs".

---

## 6. Source index

- Uniswap v3 Ethereum deployments — https://developers.uniswap.org/contracts/v3/reference/deployments/ethereum-deployments
- INonfungiblePositionManager (structs/signatures) — https://github.com/Uniswap/v3-periphery/blob/main/contracts/interfaces/INonfungiblePositionManager.sol
- NonfungiblePositionManager implementation — https://github.com/Uniswap/v3-periphery/blob/main/contracts/NonfungiblePositionManager.sol
- NPM on Etherscan (`0xC364…FE88`) — https://etherscan.io/address/0xc36442b4a4522e871399cd717abdd847ab11fe88
- IUniswapV3PoolState (slot0/ticks/positions/observations) — https://github.com/Uniswap/v3-core/blob/main/contracts/interfaces/pool/IUniswapV3PoolState.sol
- Uniswap v3 math primer (sqrtPriceX96, tick↔price, amounts) — https://blog.uniswap.org/uniswap-v3-math-primer
- Uniswap v3 whitepaper (fee growth, tick accounting) — https://app.uniswap.org/whitepaper-v3.pdf
- Fee tier ↔ tickSpacing mapping — https://rareskills.io/post/uniswap-v3-tick-spacing
- Ticks explainer — https://support.uniswap.org/hc/en-us/articles/21069524840589-What-is-a-tick-when-providing-liquidity
- DefiLlama yields API (pool APY incl. uniswap-v3) — https://yields.llama.fi/pools
- SDK internals inspected: `apps/geckoterminal/src/tool.rs`, `apps/defillama/src/tool.rs` + `lib.rs`, `apps/dune/src/tool.rs`, `apps/across/src/tool.rs` (stage_tx/route pattern), `sdk/src/builder.rs` (host targets)
