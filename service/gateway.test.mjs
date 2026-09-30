import test from 'node:test';
import assert from 'node:assert/strict';
import { createGateway } from './gateway.mjs';
import { hashGenerationContext } from './generation-policy.mjs';

function fixture({ capacity = '1000000', stream, ...options } = {}) {
  const plan = { models: { 'allowed/model': { inputMicrosPerMillionTokens: 1_000_000, outputMicrosPerMillionTokens: 1_000_000, maxInputBytes: 10000, maxInputTokens: 1000, maxOutputTokens: 100, estimateInputTokens: () => 100 } }, outputModels: { chat: 'allowed/model', website: 'allowed/model' }, maxMessages: 20, maxReserveMicros: '10000' };
  const request = { attemptId: '12e45678-e89b-42d3-a456-426614174000', conversationId: '12e45678-e89b-42d3-a456-426614174001', userMessageId: '12e45678-e89b-42d3-a456-426614174002', output: 'chat', messages: [{ role: 'user', content: 'Hello' }] };
  request.contextHash = hashGenerationContext(request);
  const rows = new Map(); let held = 0n, spent = 0n, calls = 0, settlements = 0;
  const ledger = {
    async reserve(input) {
      const key = `${input.accountId}:${input.attemptId}`;
      if (rows.has(key)) return { created: false, attempt: { ...rows.get(key) } };
      if (held + spent + BigInt(input.reserveMicros) > BigInt(capacity)) throw new Error('ALLOWANCE_EXHAUSTED');
      const row = { ...input, status: 'reserved' }; rows.set(key, row); held += BigInt(input.reserveMicros);
      return { created: true, attempt: { ...row } };
    },
    async recordGeneration(input) { Object.assign(rows.get(`${input.accountId}:${input.attemptId}`), { generationId: input.generationId }); },
    async fail(input) {
      const row = rows.get(`${input.accountId}:${input.attemptId}`);
      if (row.status === 'settled' || row.status === 'released') return { ...row };
      row.status = input.costKnownZero === true ? 'released' : 'pending';
      if (input.costKnownZero) held -= BigInt(row.reserveMicros);
      return { ...row };
    },
    async settle(input) {
      const row = rows.get(`${input.accountId}:${input.attemptId}`);
      if (row.status === 'settled') return { ...row };
      if (BigInt(input.debitMicros) > BigInt(row.reserveMicros)) throw new Error('RESERVATION_EXCEEDED');
      held -= BigInt(row.reserveMicros); spent += BigInt(input.debitMicros); settlements++;
      Object.assign(row, input, { status: 'settled' }); return { ...row };
    },
    async lookupAttempt(input) { const row = rows.get(`${input.accountId}:${input.attemptId}`); if (!row) throw new Error('ATTEMPT_NOT_FOUND'); return { ...row }; },
  };
  const provider = { streamGeneration(...args) { calls++; return stream ? stream(...args) : (async function* () { yield { type: 'started', generationId: 'gen-1' }; yield { type: 'delta', text: 'Hello back' }; yield { type: 'completed', generationId: 'gen-1', costMicros: '50' }; })(); } };
  return { gateway: createGateway({ ledger, provider, plan, ...options }), request, ledger, state: () => ({ held, spent, calls, settlements }) };
}
const invoke = (f, extra = {}) => f.gateway.generate({ accountId: 'account-1', request: f.request, ...extra });

test('concurrent same-attempt calls dispatch one provider and never replay after settlement', async () => {
  const f = fixture(); const results = await Promise.all([invoke(f), invoke(f)]);
  assert.equal(results.filter((r) => r.replay).length, 1);
  assert.equal(results.find((r) => !r.replay).content, 'Hello back');
  assert.deepEqual(f.state(), { held: 0n, spent: 50n, calls: 1, settlements: 1 });
  const replay = await invoke(f); assert.equal(replay.responseAvailable, false);
  assert.equal(f.state().calls, 1); assert.equal(f.state().settlements, 1);
});
test('cap rejection prevents dispatch', async () => {
  const f = fixture({ capacity: '1' }); await assert.rejects(invoke(f), /ALLOWANCE_EXHAUSTED/); assert.equal(f.state().calls, 0);
});
test('provider rejection cannot claim zero cost and reservation remains held', async () => {
  const f = fixture({ stream: () => { throw Object.assign(new Error('secret provider detail'), { costKnownZero: true }); } });
  await assert.rejects(invoke(f), (e) => e.code === 'GENERATION_PENDING' && !e.message.includes('secret'));
  assert.equal((await f.gateway.lookup({ accountId: 'account-1', attemptId: f.request.attemptId })).attempt.status, 'pending');
  assert.equal(f.state().held, 200n); await invoke(f); assert.equal(f.state().calls, 1);
});
test('truncated stream does not expose a complete response or settle usage', async () => {
  const f = fixture({ stream: async function* () { yield { type: 'started', generationId: 'gen-1' }; yield { type: 'delta', text: 'Partial' }; } });
  await assert.rejects(invoke(f), (e) => e.code === 'TRUNCATED_STREAM');
  assert.deepEqual(f.state(), { held: 200n, spent: 0n, calls: 1, settlements: 0 });
});
test('usage beyond reserved cap stays pending for reconciliation', async () => {
  const f = fixture({ stream: async function* () { yield { type: 'started', generationId: 'gen-1' }; yield { type: 'delta', text: 'Done' }; yield { type: 'completed', generationId: 'gen-1', costMicros: '201' }; } });
  await assert.rejects(invoke(f), (e) => e.code === 'GENERATION_PENDING'); assert.equal(f.state().held, 200n);
});
test('account-scoped lookup cannot reveal another account attempt', async () => {
  const f = fixture(); await invoke(f);
  await assert.rejects(f.gateway.lookup({ accountId: 'other-account', attemptId: f.request.attemptId }), /ATTEMPT_NOT_FOUND/);
});
test('pre-dispatch cancellation is proven zero cost and releases without provider call', async () => {
  const f = fixture(); const controller = new AbortController(); controller.abort();
  await assert.rejects(invoke(f, { signal: controller.signal }), (e) => e.code === 'CANCELLED');
  assert.equal(f.state().calls, 0); assert.equal(f.state().held, 0n);
});
test('oversized stream and completion generation mismatch retain reservation', async () => {
  for (const mismatch of [false, true]) {
    const f = fixture({ maxResponseBytes: mismatch ? 100 : 1, stream: async function* () { yield { type: 'started', generationId: 'gen-1' }; yield { type: 'delta', text: 'Done' }; yield { type: 'completed', generationId: mismatch ? 'different' : 'gen-1', costMicros: '50' }; } });
    await assert.rejects(invoke(f)); assert.equal(f.state().held, 200n); assert.equal(f.state().settlements, 0);
  }
});
test('hung provider is aborted by bounded timeout without releasing unknown cost', async () => {
  const f = fixture({ timeoutMs: 10, stream: () => new Promise(() => {}) });
  await assert.rejects(invoke(f), (e) => e.code === 'INTERRUPTED'); assert.equal(f.state().held, 200n);
});
test('trusted server model/output cap reaches adapter and request model injection is rejected', async () => {
  let received;
  const f = fixture({ stream: async function* (body, options) { received = { body, options }; yield { type: 'started', generationId: 'gen-1' }; yield { type: 'delta', text: 'Done' }; yield { type: 'completed', generationId: 'gen-1', costMicros: '50' }; } });
  await invoke(f); assert.equal(received.options.model, 'allowed/model'); assert.equal(received.options.maxOutputTokens, 100); assert.equal(received.body.maxOutputTokens, 100);
  await assert.rejects(invoke(f, { request: { ...f.request, model: 'injected/model' } })); assert.equal(f.state().calls, 1);
});
test('events after completion fail closed and iterator cleanup is attempted', async () => {
  let closed = false;
  const f = fixture({ stream: async function* () { try { yield { type: 'started', generationId: 'gen-1' }; yield { type: 'delta', text: 'Done' }; yield { type: 'completed', generationId: 'gen-1', costMicros: '50' }; yield { type: 'delta', text: 'Unexpected' }; } finally { closed = true; } } });
  await assert.rejects(invoke(f), (e) => e.code === 'INVALID_STREAM');
  assert.equal(closed, true); assert.equal(f.state().held, 200n); assert.equal(f.state().settlements, 0);
});

// Optional real PostgreSQL exercise. The test creates an isolated schema and
// removes only that schema; it never uses an application production database.
test('PostgreSQL coordinator claims concurrent attempt once and keeps ambiguous costs held', { skip: !process.env.BENCH_LEDGER_TEST_URL }, async () => {
  const { default: pg } = await import('pg');
  const { randomUUID } = await import('node:crypto');
  const { createLedger, migrateLedger } = await import('./ledger/index.js');
  const schema = `gateway_test_${randomUUID().replaceAll('-', '')}`;
  const admin = new pg.Pool({ connectionString: process.env.BENCH_LEDGER_TEST_URL });
  let pool;
  try {
    await admin.query(`CREATE SCHEMA ${schema}`);
    pool = new pg.Pool({ connectionString: process.env.BENCH_LEDGER_TEST_URL, options: `-c search_path=${schema}` });
    await migrateLedger(pool);
    const ledger = createLedger(pool);
    await ledger.grantPaidPeriod({ accountId: 'account-1', subscriptionId: 'subscription-1', invoiceId: 'invoice-1', planId: 'test-plan', allowanceMicros: '500', periodStart: new Date(Date.now() - 10000), periodEnd: new Date(Date.now() + 3600000) });
    const fixtureData = fixture(); let calls = 0;
    const plan = { models: { 'allowed/model': { inputMicrosPerMillionTokens: 1_000_000, outputMicrosPerMillionTokens: 1_000_000, maxInputBytes: 10000, maxInputTokens: 1000, maxOutputTokens: 100, estimateInputTokens: () => 100 } }, outputModels: { chat: 'allowed/model', website: 'allowed/model' }, maxMessages: 20, maxReserveMicros: '10000' };
    const provider = { async *streamGeneration(request) { calls++; yield { type: 'started', generationId: request.attemptId }; yield { type: 'delta', text: 'Done' }; if (calls === 1) yield { type: 'completed', generationId: request.attemptId, costMicros: '50' }; } };
    const gateway = createGateway({ ledger, provider, plan });
    const request = fixtureData.request;
    const results = await Promise.all([gateway.generate({ accountId: 'account-1', request }), gateway.generate({ accountId: 'account-1', request })]);
    assert.equal(calls, 1); assert.equal(results.filter((r) => r.replay).length, 1);
    assert.equal((await ledger.entitlement({ accountId: 'account-1' })).spentMicros, '50');
    const incomplete = { ...request, attemptId: randomUUID() };
    await assert.rejects(gateway.generate({ accountId: 'account-1', request: incomplete }), (e) => e.code === 'TRUNCATED_STREAM');
    const held = await ledger.entitlement({ accountId: 'account-1' }); assert.equal(held.reservedMicros, '200'); assert.equal(held.availableMicros, '250');
    await gateway.generate({ accountId: 'account-1', request: incomplete }); assert.equal(calls, 2);
    await assert.rejects(gateway.lookup({ accountId: 'other-account', attemptId: incomplete.attemptId }));
    await assert.rejects(gateway.generate({ accountId: 'account-1', request: { ...request, attemptId: randomUUID() } }), (e) => e.code === 'TRUNCATED_STREAM');
    await assert.rejects(gateway.generate({ accountId: 'account-1', request: { ...request, attemptId: randomUUID() } }), (e) => ['ALLOWANCE_EXHAUSTED', 'CONCURRENCY_LIMIT'].includes(e.code));
    assert.equal(calls, 3);
  } finally {
    if (pool) await pool.end();
    await admin.query(`DROP SCHEMA IF EXISTS ${schema} CASCADE`);
    await admin.end();
  }
});
