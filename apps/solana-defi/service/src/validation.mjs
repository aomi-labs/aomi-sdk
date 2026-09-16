import { PublicKey } from '@solana/web3.js';

export class InputError extends Error {}

export function publicKey(value, label) {
  try {
    if (typeof value !== 'string') throw new Error();
    const key = new PublicKey(value);
    if (key.toBase58() !== value) throw new Error();
    return key;
  } catch { throw new InputError(`${label} must be a canonical Solana address`); }
}

export function rawAmount(value, label = 'amount_raw') {
  if (typeof value !== 'string' || !/^[1-9][0-9]{0,19}$/.test(value) || BigInt(value) > 2n**64n - 1n) {
    throw new InputError(`${label} must be a positive integer string within u64 range`);
  }
  return value;
}

export function slippageBps(value = 50) {
  if (!Number.isInteger(value) || value < 0 || value > 500) {
    throw new InputError('slippage_bps must be an integer between 0 and 500');
  }
  return value;
}

export function validateRequest(op, args) {
  const fields = op === 'venues' ? [] : ['venue', 'market', 'wallet'];
  if (op === 'prepare') fields.push('action', 'amount_raw', 'withdraw_all', 'slippage_bps');
  if (!['venues', 'market', 'position', 'prepare'].includes(op)) throw new InputError('Unknown operation');
  if (!args || typeof args !== 'object' || Array.isArray(args)) throw new InputError('Expected an object');
  if (Object.keys(args).some(key => !fields.includes(key))) throw new InputError('Unexpected request field');
  if (op === 'venues') return args;
  if (!['jupiter-lend', 'kamino-earn', 'pumpswap'].includes(args.venue)) throw new InputError('Unsupported venue');
  publicKey(args.market, 'market'); publicKey(args.wallet, 'wallet');
  if (op === 'prepare') {
    if (!['deposit', 'withdraw'].includes(args.action)) throw new InputError('Expected deposit or withdraw');
    if (args.withdraw_all !== undefined && typeof args.withdraw_all !== 'boolean') throw new InputError('withdraw_all must be boolean');
    slippageBps(args.slippage_bps);
    if (args.withdraw_all === true) {
      if (args.action !== 'withdraw' || args.amount_raw !== undefined) throw new InputError('Withdraw all takes no amount');
    } else rawAmount(args.amount_raw);
  }
  return args;
}

export function web3Instructions(ixs) {
  return ixs.map(ix => ({ program_id: ix.programId.toBase58(), data_base64: ix.data.toString('base64'),
    accounts: ix.keys.map(k => ({ pubkey: k.pubkey.toBase58(), is_signer: k.isSigner, is_writable: k.isWritable })) }));
}

export function kitInstructions(ixs) {
  return ixs.map(ix => ({ program_id: ix.programAddress, data_base64: Buffer.from(ix.data ?? []).toString('base64'),
    accounts: (ix.accounts ?? []).map(k => ({ pubkey: k.address, is_signer: (k.role & 2) !== 0, is_writable: (k.role & 1) !== 0 })) }));
}
