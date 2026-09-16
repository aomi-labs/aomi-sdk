import { PublicKey } from '@solana/web3.js';
import { OnlinePumpAmmSdk, PUMP_AMM_SDK, depositLpToken } from '@pump-fun/pump-swap-sdk';
import { getMint } from '@solana/spl-token';
import BN from 'bn.js';
import { InputError, slippageBps, web3Instructions } from './validation.mjs';

export async function pumpswap(connection, args, prepare) {
  const state = await new OnlinePumpAmmSdk(connection).liquiditySolanaState(new PublicKey(args.market), new PublicKey(args.wallet));
  const [baseInfo, quoteInfo, lpInfo] = await Promise.all([state.pool.baseMint, state.pool.quoteMint, state.pool.lpMint].map(async mint => {
    const account = await connection.getAccountInfo(mint);
    if (!account) throw new InputError('Pool mint account is unavailable');
    return getMint(connection, mint, 'confirmed', account.owner);
  }));
  const userAccount = await connection.getAccountInfo(state.userPoolTokenAccount);
  const lpBalance = userAccount ? (await connection.getTokenAccountBalance(state.userPoolTokenAccount)).value.amount : '0';
  const result = { market: { address: args.market, label: 'PumpSwap liquidity pool',
      asset: state.pool.quoteMint.toBase58(), decimals: quoteInfo.decimals,
      base_asset: state.pool.baseMint.toBase58(), base_decimals: baseInfo.decimals,
      share_mint: state.pool.lpMint.toBase58(), share_decimals: lpInfo.decimals,
      program_id: 'pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA',
      base_reserve_raw: state.poolBaseTokenAccount.amount.toString(), quote_reserve_raw: state.poolQuoteTokenAccount.amount.toString() },
    position: { shares_raw: lpBalance, scope: 'wallet' },
    warnings: ['Liquidity provision exposes both assets to price changes and impermanent loss. Input amount is a target quote contribution; review both maximum inputs.'] };
  if (!prepare) return result;
  const slippage = slippageBps(args.slippage_bps) / 100;
  let ixs, bounds, lp;
  if (args.action === 'deposit') {
    ({ lpToken: lp } = PUMP_AMM_SDK.depositAutocompleteBaseAndLpTokenFromQuote(state, new BN(args.amount_raw), slippage));
    if (lp.isZero()) throw new InputError('Deposit is too small to mint LP shares');
    bounds = depositLpToken(lp, slippage, new BN(state.poolBaseTokenAccount.amount.toString()),
      new BN(state.poolQuoteTokenAccount.amount.toString()), state.pool.lpSupply);
    ixs = await PUMP_AMM_SDK.depositInstructions(state, lp, slippage);
    result.maximum_inputs = [{ asset: result.market.base_asset, amount_raw: bounds.maxBase.toString(), decimals: baseInfo.decimals },
      { asset: result.market.asset, amount_raw: bounds.maxQuote.toString(), decimals: quoteInfo.decimals }];
  } else {
    lp = new BN(args.withdraw_all ? lpBalance : args.amount_raw);
    if (lp.isZero() || lp.gt(new BN(lpBalance))) throw new InputError('Insufficient LP shares');
    bounds = PUMP_AMM_SDK.withdrawInputs(state, lp, slippage);
    ixs = await PUMP_AMM_SDK.withdrawInstructions(state, lp, slippage);
    result.minimum_outputs = [{ asset: result.market.base_asset, amount_raw: bounds.minBase.toString(), decimals: baseInfo.decimals },
      { asset: result.market.asset, amount_raw: bounds.minQuote.toString(), decimals: quoteInfo.decimals }];
  }
  result.instructions = web3Instructions(ixs);
  result.action = { kind: args.action, shares_raw: lp.toString(), target_quote_raw: args.action === 'deposit' ? args.amount_raw : null };
  return result;
}
