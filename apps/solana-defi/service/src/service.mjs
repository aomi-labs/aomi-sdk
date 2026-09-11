import { Connection, PublicKey } from '@solana/web3.js';
import { validateRequest, InputError } from './validation.mjs';
import { jupiter } from './jupiter.mjs';
import { kamino } from './kamino.mjs';
import { pumpswap } from './pumpswap.mjs';

export const VENUES = [
  { id: 'jupiter-lend', name: 'Jupiter Lend', kind: 'lending',
    program_id: 'jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9',
    example_market: '2vVYHYM8VYnvZqQWpTJSj8o8DBf1wM8pVs3bsTgYZiqJ',
    example_label: 'USDC lending market', source: 'https://developers.jup.ag/docs/lend' },
  { id: 'kamino-earn', name: 'Kamino Earn', kind: 'vault',
    program_id: 'KvauGMspG5k6rtzrqqn7WNn3oZdyKqLKwK2XWQ8FLjd',
    example_market: 'HDsayqAsDWy3QvANGqh2yNraqcD8Fnjgh73Mhb3WRS5E',
    example_label: 'USDC vault from the Kamino developer guide', source: 'https://kamino.com/docs/build/developers/earn' },
  { id: 'pumpswap', name: 'PumpSwap', kind: 'liquidity_pool',
    program_id: 'pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA',
    example_market: '6ef59PhPsXgre7d8BUB2J6GGK6RM3ABek6XSR7J3Z6kX',
    example_label: 'SOL/USDC example pool; inspect reserves before use', source: 'https://github.com/pump-fun/pump-public-docs' },
];

export class SolanaDefi {
  constructor(rpcUrl) {
    const url = new URL(rpcUrl);
    if (!['http:', 'https:'].includes(url.protocol)) throw new Error('RPC URL must use HTTP or HTTPS');
    this.rpcUrl = rpcUrl;
    this.connection = new Connection(rpcUrl, { commitment: 'confirmed', disableRetryOnRateLimit: true });
  }

  async run(op, args) {
    validateRequest(op, args);
    if (op === 'venues') return { venues: VENUES, cluster: 'mainnet-beta',
      description: 'Aomi-side SDK recipes for supported protocol families; example markets are not investment recommendations.' };
    const version = await this.connection.getVersion();
    const localMirror = Boolean(version['surfnet-version']) && ['127.0.0.1', 'localhost', '[::1]'].includes(new URL(this.rpcUrl).hostname);
    // Full genesis hash, not the truncated CAIP-2 chain identifier.
    if (!localMirror && await this.connection.getGenesisHash() !== '5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d') {
      throw new InputError('This recipe requires Solana mainnet-beta or a local mainnet mirror');
    }
    const definition = VENUES.find(venue => venue.id === args.venue);
    const account = await this.connection.getAccountInfo(new PublicKey(args.market));
    if (!account || account.executable || account.owner.toBase58() !== definition.program_id) {
      throw new InputError('Market account is not owned by the selected protocol program');
    }
    const result = await ({ 'jupiter-lend': () => jupiter(this.connection, args, op === 'prepare'),
      'kamino-earn': () => kamino(this.rpcUrl, args, op === 'prepare'),
      pumpswap: () => pumpswap(this.connection, args, op === 'prepare') })[args.venue]();
    const envelope = { venue: args.venue, wallet: args.wallet, cluster: 'mainnet-beta',
      local_mirror: localMirror, slot: await this.connection.getSlot(),
      prepared_at: Math.floor(Date.now() / 1000), ...result };
    if (op === 'position') return { ...envelope, instructions: undefined, action: undefined };
    if (op === 'prepare') {
      if (!result.instructions?.length) throw new InputError('No instructions were prepared');
      if (result.instructions.some(ix => ix.accounts.some(account => account.is_signer && account.pubkey !== args.wallet))) {
        throw new InputError('The prepared action requires an additional signer');
      }
      envelope.instructions = [{ description: `${args.action} on ${definition.name}`, version: 'v0', instructions: result.instructions }];
      envelope.expires_at = envelope.prepared_at + 120;
      envelope.signing = 'none';
    }
    return envelope;
  }
}
