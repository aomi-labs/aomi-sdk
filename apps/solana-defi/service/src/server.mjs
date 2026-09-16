import { createServer } from 'node:http';
import { SolanaDefi } from './service.mjs';
import { InputError } from './validation.mjs';

const rpcUrl = process.env.AOMI_SOLANA_RPC_URL;
if (!rpcUrl) throw new Error('AOMI_SOLANA_RPC_URL is required');
const service = new SolanaDefi(rpcUrl);
const port = Number(process.env.AOMI_SOLANA_DEFI_PORT || 18895);
let active = 0;
const server = createServer(async (req, res) => {
  const reply = (status, value) => { res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store' }); res.end(JSON.stringify(value)); };
  if (req.method === 'GET' && req.url === '/health') return reply(200, { status: 'ok', signing: false });
  if (req.method !== 'POST' || !/^\/(venues|market|position|prepare)$/.test(req.url ?? '')) return reply(404, { error: 'Not found' });
  if (active >= 4) return reply(503, { error: 'Preparation service busy' });
  active++;
  try {
    let body = '';
    for await (const chunk of req) {
      body += chunk.toString();
      if (Buffer.byteLength(body) > 16384) throw new InputError('Request too large');
    }
    let args;
    try { args = JSON.parse(body); } catch { throw new InputError('Invalid JSON'); }
    reply(200, await service.run(req.url.slice(1), args));
  } catch (error) {
    // RPC errors can contain a credential-bearing URL. Never return or log them.
    reply(error instanceof InputError ? 400 : 502, { error: error instanceof InputError ? error.message : 'Unable to read or prepare this market' });
  } finally { active--; }
});
server.requestTimeout = 120000;
server.headersTimeout = 10000;
server.listen(port, '127.0.0.1', () => console.log(`Solana DeFi preparation listening on loopback port ${port}; no signing`));
