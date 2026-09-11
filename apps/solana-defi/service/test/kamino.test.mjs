import assert from 'node:assert/strict';
import test from 'node:test';
import { Keypair } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { walletSharesRaw } from '../src/kamino.mjs';

const wallet = Keypair.generate().publicKey;
const mint = Keypair.generate().publicKey;
function holding(amount, owner = wallet, asset = mint) {
  const data = Buffer.alloc(165);
  asset.toBuffer().copy(data, 0);
  owner.toBuffer().copy(data, 32);
  data.writeBigUInt64LE(amount, 64);
  data[108] = 1;
  return { pubkey: Keypair.generate().publicKey.toBase58(), account: {
    owner: TOKEN_PROGRAM_ID.toBase58(), data: [data.toString('base64'), 'base64'],
  } };
}
function rpc(value) {
  return { getTokenAccountsByOwner(owner, filter, encoding) {
    assert.equal(owner, wallet.toBase58());
    assert.deepEqual(filter, { mint: mint.toBase58() });
    assert.deepEqual(encoding, { encoding: 'base64' });
    return { send: async () => ({ value }) };
  } };
}

test('wallet share metadata preserves exact raw holdings across accounts, beyond Number precision', async () => {
  assert.equal(await walletSharesRaw(rpc([holding(9007199254740993n), holding(7n)]),
    wallet.toBase58(), mint.toBase58()), '9007199254741000');
});
test('a fully staked wallet can have zero net wallet share spending capacity', async () => {
  assert.equal(await walletSharesRaw(rpc([]), wallet.toBase58(), mint.toBase58()), '0');
  assert.equal(await walletSharesRaw(rpc([holding(0n)]), wallet.toBase58(), mint.toBase58()), '0');
});
test('wrong account identity, duplicate holdings, and unavailable raw state fail closed', async () => {
  const duplicate = holding(1n);
  const cases = [[holding(1n, Keypair.generate().publicKey)],
    [holding(1n, wallet, Keypair.generate().publicKey)], [duplicate, duplicate],
    [{ ...holding(1n), account: { owner: TOKEN_PROGRAM_ID.toBase58(), data: ['AA==', 'base64'] } }]];
  for (const value of cases) await assert.rejects(walletSharesRaw(rpc(value), wallet.toBase58(), mint.toBase58()));
});
