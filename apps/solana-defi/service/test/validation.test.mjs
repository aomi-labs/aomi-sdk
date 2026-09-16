import assert from 'node:assert/strict';
import test from 'node:test';
import { rawAmount, validateRequest, slippageBps, InputError, web3Instructions, kitInstructions } from '../src/validation.mjs';
import { SolanaDefi, VENUES } from '../src/service.mjs';
import { PublicKey } from '@solana/web3.js';

const args = { venue: 'kamino-earn', market: VENUES[1].example_market,
  wallet: 'HQXTpSdMnY1WgyoAXgczyBmwkmajQVUdSe6fpraNaG1F' };

test('raw amounts preserve precision and reject coercion, negatives and overflow', () => {
  assert.equal(rawAmount('18446744073709551615'), '18446744073709551615');
  for (const value of [1, true, '0', '-1', '0.1', '1e6', '01', '18446744073709551616', null]) {
    assert.throws(() => rawAmount(value), InputError);
  }
});

test('preparation rejects RPC overrides and ambiguous withdrawal units before any RPC', () => {
  assert.throws(() => validateRequest('prepare', { ...args, action: 'deposit', amount_raw: '1', rpc: 'https://example.com' }), InputError);
  assert.throws(() => validateRequest('prepare', { ...args, action: 'withdraw', amount_raw: '1', withdraw_all: true }), InputError);
  assert.throws(() => validateRequest('prepare', { ...args, action: 'deposit', withdraw_all: true }), InputError);
  assert.throws(() => validateRequest('prepare', { ...args, action: 'withdraw', withdraw_all: 'true' }), InputError);
  assert.deepEqual(validateRequest('prepare', { ...args, action: 'withdraw', withdraw_all: true }), { ...args, action: 'withdraw', withdraw_all: true });
  assert.throws(() => validateRequest('market', { ...args, wallet: 'not a key' }), InputError);
  assert.throws(() => validateRequest('sign', args), InputError);
  for (const value of [-1, 501, 0.5, '50']) assert.throws(() => slippageBps(value), InputError);
});

test('instruction conversion preserves bytes, account order and signer/writable roles', () => {
  const owner = new PublicKey(args.wallet), program = new PublicKey('11111111111111111111111111111111');
  const expected = [{ program_id: program.toBase58(), data_base64: 'AAH/', accounts: [
    { pubkey: owner.toBase58(), is_signer: true, is_writable: false },
    { pubkey: program.toBase58(), is_signer: false, is_writable: true },
  ] }];
  assert.deepEqual(web3Instructions([{ programId: program, data: Buffer.from([0, 1, 255]), keys: [
    { pubkey: owner, isSigner: true, isWritable: false }, { pubkey: program, isSigner: false, isWritable: true },
  ] }]), expected);
  assert.deepEqual(kitInstructions([{ programAddress: program.toBase58(), data: new Uint8Array([0, 1, 255]), accounts: [
    { address: owner.toBase58(), role: 2 }, { address: program.toBase58(), role: 1 },
  ] }]), expected);
});

test('a selected venue cannot prepare an account owned by another protocol', async () => {
  const service = new SolanaDefi('http://127.0.0.1:8899');
  service.connection = { getVersion: async () => ({ 'surfnet-version': 'test' }),
    getAccountInfo: async () => ({ executable: false, owner: new PublicKey(VENUES[0].program_id) }) };
  await assert.rejects(service.run('prepare', { ...args, action: 'deposit', amount_raw: '1000000' }), /not owned/);
});

test('remote non-mainnet RPC cannot be mislabeled as mainnet', async () => {
  const service = new SolanaDefi('https://example.com');
  service.connection = { getVersion: async () => ({ 'surfnet-version': 'not-local' }), getGenesisHash: async () => 'devnet' };
  await assert.rejects(service.run('market', args), /requires Solana mainnet/);
});
