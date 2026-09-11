import { PublicKey } from '@solana/web3.js';
import { getLendingProgram, getDepositContext, getDepositIxs, getWithdrawIxs,
  getRedeemIxs, getUserLendingPositionByAsset } from '@jup-ag/lend/earn';
import BN from 'bn.js';
import { InputError, web3Instructions } from './validation.mjs';

export async function jupiter(connection, args, prepare) {
  const signer = new PublicKey(args.wallet);
  const lending = new PublicKey(args.market);
  const details = await getLendingProgram({ connection, market: 'main' }).account.lending.fetch(lending);
  const asset = details.mint;
  const ctx = await getDepositContext({ connection, signer, asset, market: 'main' });
  if (!ctx.lending.equals(lending)) throw new InputError('Market does not match the Jupiter main lending market for this asset');
  const held = await getUserLendingPositionByAsset({ connection, user: signer, asset, market: 'main' });
  const result = { market: { address: args.market, label: 'Jupiter Lend', asset: asset.toBase58(),
      decimals: details.decimals, share_mint: ctx.fTokenMint.toBase58(), program_id: 'jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9' },
    position: { shares_raw: held.lendingTokenShares.toString(), underlying_raw: held.underlyingAssets.toString(),
      wallet_asset_raw: held.underlyingBalance.toString(), scope: 'wallet' }, warnings: [] };
  if (!prepare) return result;
  let built;
  let amount;
  if (args.withdraw_all) {
    if (held.lendingTokenShares.isZero()) throw new InputError('No Jupiter lending shares to withdraw');
    amount = held.lendingTokenShares.toString();
    built = await getRedeemIxs({ connection, signer, asset, shares: new BN(amount), market: 'main' });
  } else {
    amount = args.amount_raw;
    built = await (args.action === 'deposit' ? getDepositIxs : getWithdrawIxs)(
      { connection, signer, asset, amount: new BN(amount), market: 'main' });
  }
  result.instructions = web3Instructions(built.ixs);
  result.action = { kind: args.action, amount_raw: amount, denomination: args.withdraw_all ? 'shares' : 'asset',
    asset: args.withdraw_all ? result.market.share_mint : result.market.asset };
  return result;
}
