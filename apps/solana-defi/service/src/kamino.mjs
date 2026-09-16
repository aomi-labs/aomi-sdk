import { address, createSolanaRpc, createNoopSigner } from '@solana/kit';
import { KaminoVault, getCurrentLedgerInstant } from '@kamino-finance/klend-sdk';
import { fetchFarmStateOrNull } from '@kamino-finance/klend-sdk/dist/classes/farm_utils.js';
import Decimal from 'decimal.js';
import { PublicKey } from '@solana/web3.js';
import { unpackAccount, TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { InputError, kitInstructions } from './validation.mjs';

// Farm shares are an economic position and may be fractional. Spending
// policy instead bounds the net debit of exact tokens held by this wallet.
export async function walletSharesRaw(rpc, wallet, mint) {
  const response = await rpc.getTokenAccountsByOwner(address(wallet), { mint: address(mint) },
    { encoding: 'base64' }).send();
  const seen = new Set();
  let total = 0n;
  for (const { pubkey, account } of response.value) {
    if (seen.has(pubkey)) throw new InputError('Duplicate wallet share account');
    seen.add(pubkey);
    if (!Array.isArray(account.data) || account.data[1] !== 'base64') {
      throw new InputError('Exact wallet share account data is unavailable');
    }
    const data = Buffer.from(account.data[0], 'base64');
    if (account.owner !== TOKEN_PROGRAM_ID.toBase58() || data.length !== 165) {
      throw new InputError('Unsupported wallet share account layout');
    }
    const token = unpackAccount(new PublicKey(pubkey), { owner: TOKEN_PROGRAM_ID, data });
    if (!token.owner.equals(new PublicKey(wallet)) || !token.mint.equals(new PublicKey(mint)) ||
        !token.isInitialized || ![1, 2].includes(data[108])) {
      throw new InputError('Wallet share account identity or state is invalid');
    }
    total += token.amount;
  }
  if (total > 2n ** 64n - 1n) throw new InputError('Wallet share balance exceeds supported raw units');
  return total.toString();
}

export async function kamino(rpcUrl, args, prepare) {
  const rpc = createSolanaRpc(rpcUrl);
  const vault = new KaminoVault(rpc, address(args.market), 400);
  const state = await vault.getState();
  const decimals = state.tokenMintDecimals.toNumber();
  const shareDecimals = state.sharesMintDecimals.toNumber();
  const reserves = await vault.client.loadVaultReserves(state);
  const instant = await getCurrentLedgerInstant(rpc);
  const shares = await vault.getUserShares(address(args.wallet));
  const walletShares = await walletSharesRaw(rpc, args.wallet, state.sharesMint);
  const tokensPerShare = await vault.client.getTokensPerShareSingleVault(vault, instant, reserves, instant);
  const global = await vault.client.loadKVaultGlobalConfig();
  const result = { market: { address: args.market, label: Buffer.from(state.name).toString('utf8').replace(/\0/g, '').trim(),
      asset: state.tokenMint, decimals, share_mint: state.sharesMint, share_decimals: shareDecimals,
      program_id: 'KvauGMspG5k6rtzrqqn7WNn3oZdyKqLKwK2XWQ8FLjd', farm: state.vaultFarm,
      created_at: state.creationTimestamp.toString(), minimum_deposit_raw: state.minDepositAmount.toString(),
      fees: { management_bps: state.managementFeeBps.toString(), performance_bps: state.performanceFeeBps.toString(),
        withdrawal_raw: state.withdrawalPenaltyLamports.toString(), withdrawal_bps: state.withdrawalPenaltyBps.toString(),
        global_withdrawal_raw: global.withdrawalPenaltyLamports.toString(), global_withdrawal_bps: global.withdrawalPenaltyBps.toString() } },
    position: { wallet_shares_raw: walletShares, shares: shares.totalShares.toFixed(), unstaked_shares: shares.unstakedShares.toFixed(),
      staked_shares: shares.stakedShares.toFixed(), underlying: shares.totalShares.mul(tokensPerShare).toFixed(), scope: 'wallet' },
    warnings: ['Vault share value is an estimate; withdrawals depend on available liquidity and incur applicable charges.'] };
  if (!prepare) return result;
  if (state.firstLossCapitalFarm !== '11111111111111111111111111111111') {
    throw new InputError('First-loss-capital vault actions are not supported by this preparation recipe');
  }
  const hasFarm = state.vaultFarm !== '11111111111111111111111111111111';
  const farm = hasFarm ? await fetchFarmStateOrNull(rpc, state.vaultFarm) : null;
  if (hasFarm && !farm) throw new InputError('Configured vault farm is unavailable');
  const signer = createNoopSigner(address(args.wallet));
  let instructions, amount;
  if (args.action === 'deposit') {
    if (BigInt(args.amount_raw) < BigInt(state.minDepositAmount.toString())) throw new InputError('Amount is below the vault minimum deposit');
    amount = new Decimal(args.amount_raw).div(new Decimal(10).pow(decimals));
    const p = await vault.depositIxs(signer, amount, reserves, farm, null);
    instructions = [...p.depositIxs, ...p.stakeInFarmIfNeededIxs, ...p.stakeInFlcFarmIfNeededIxs];
  } else {
    amount = args.withdraw_all ? shares.totalShares : new Decimal(args.amount_raw).div(new Decimal(10).pow(shareDecimals));
    if (amount.lte(0) || amount.gt(shares.totalShares)) throw new InputError('Insufficient vault shares');
    const p = await vault.withdrawIxs(signer, amount, instant, reserves, farm, null);
    instructions = [...p.unstakeFromFarmIfNeededIxs, ...p.withdrawIxs, ...p.postWithdrawIxs];
  }
  result.instructions = kitInstructions(instructions);
  result.action = { kind: args.action, amount: amount.toFixed(), denomination: args.action === 'deposit' ? 'asset' : 'shares',
    asset: args.action === 'deposit' ? state.tokenMint : state.sharesMint };
  return result;
}
