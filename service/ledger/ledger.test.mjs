import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { before, after, test } from 'node:test';
import pg from 'pg';
import { createLedger, migrateLedger } from './index.js';

const connectionString = process.env.BENCH_LEDGER_TEST_URL;
assert.ok(connectionString, 'BENCH_LEDGER_TEST_URL is required: these tests require real PostgreSQL, not fake persistence.');
const endpoint = new URL(connectionString);
assert.ok(['127.0.0.1', 'localhost', '[::1]'].includes(endpoint.hostname), 'Ledger integration tests must use a local isolated database.');
const schema = `bench_ledger_test_${randomUUID().replaceAll('-', '')}`;
const options = `-c search_path=${schema}`;
let pool;
const instant = new Date('2026-09-30T12:00:00Z');
const digest = 'a'.repeat(64);
const start = '2026-09-01T00:00:00Z', end = '2026-10-01T00:00:00Z';

before(async () => {
  pool = new pg.Pool({ connectionString, max: 30, options });
  await pool.query(`CREATE SCHEMA ${schema}`);
  await migrateLedger(pool);
});
after(async () => {
  if (pool) { await pool.query(`DROP SCHEMA IF EXISTS ${schema} CASCADE`); await pool.end(); }
});

async function fixture(allowanceMicros = '100', config = {}) {
  const accountId = randomUUID(), subscriptionId = randomUUID(), invoiceId = randomUUID();
  const ledger = createLedger(pool, { clock: () => instant, maxConcurrent: 100, ...config });
  const grant = { accountId, subscriptionId, invoiceId, planId: 'test-plan', periodStart: start, periodEnd: end, allowanceMicros };
  await ledger.grantPaidPeriod(grant);
  const request = (attemptId = randomUUID(), reserveMicros = '10') => ({ accountId, attemptId, conversationId: 'local-conversation', model: 'test-model', reserveMicros, requestDigest: digest });
  return { ledger, accountId, subscriptionId, invoiceId, grant, request };
}
const code = (expected) => (error) => error.code === expected;

test('paid invoices grant once under concurrency; alternate invoices cannot grant same period', async () => {
  const f = await fixture();
  const results = await Promise.all(Array.from({ length: 20 }, () => f.ledger.grantPaidPeriod(f.grant)));
  assert.ok(results.every((result) => !result.created));
  assert.equal((await f.ledger.entitlement(f)).availableMicros, '100');
  await assert.rejects(f.ledger.grantPaidPeriod({ ...f.grant, allowanceMicros: '200' }), code('CONFLICT'));
  await assert.rejects(f.ledger.grantPaidPeriod({ ...f.grant, invoiceId: randomUUID() }), code('CONFLICT'));
  const entries = await pool.query("SELECT count(*)::int AS count FROM allowance_ledger WHERE account_id=$1 AND kind='grant'", [f.accountId]);
  assert.equal(entries.rows[0].count, 1);
});

test('concurrent attempts atomically consume the last allowance without overdrawing', async () => {
  const f = await fixture();
  const results = await Promise.allSettled(Array.from({ length: 50 }, () => f.ledger.reserve(f.request(randomUUID(), '3'))));
  assert.equal(results.filter((r) => r.status === 'fulfilled').length, 33);
  assert.ok(results.filter((r) => r.status === 'rejected').every((r) => r.reason.code === 'ALLOWANCE_EXHAUSTED'));
  const entitlement = await f.ledger.entitlement(f);
  assert.equal(entitlement.availableMicros, '1');
  assert.equal(entitlement.reservedMicros, '99');
  const audit = (await pool.query('SELECT sum(held_delta)::text AS held FROM allowance_ledger WHERE account_id=$1', [f.accountId])).rows[0];
  assert.equal(audit.held, '99');
});

test('one attempt starts once, binds exact request, and cannot cross accounts', async () => {
  const f = await fixture();
  const request = f.request();
  const results = await Promise.all(Array.from({ length: 20 }, () => f.ledger.reserve(request)));
  assert.equal(results.filter((result) => result.created).length, 1);
  for (const patch of [{ model: 'other' }, { conversationId: 'other' }, { reserveMicros: '11' }, { requestDigest: 'b'.repeat(64) }]) {
    await assert.rejects(f.ledger.reserve({ ...request, ...patch }), code('CONFLICT'));
  }
  const other = await fixture();
  await assert.rejects(other.ledger.lookupAttempt({ accountId: other.accountId, attemptId: request.attemptId }), code('ATTEMPT_NOT_FOUND'));
  await f.ledger.fail({ ...request, costKnownZero: true });
  assert.equal((await f.ledger.reserve(request)).created, false);
  assert.equal((await f.ledger.entitlement(f)).availableMicros, '100');
});

test('integer micro-units remain exact beyond JavaScript safe integers and settlement is idempotent', async () => {
  const f = await fixture('9007199254740993');
  const request = f.request(randomUUID(), 9007199254740993n);
  await f.ledger.reserve(request);
  const settlement = { ...request, debitMicros: '9007199254740000', costMicros: '9007199254740000', generationId: randomUUID() };
  await Promise.all(Array.from({ length: 12 }, () => f.ledger.settle(settlement)));
  const entitlement = await f.ledger.entitlement(f);
  assert.equal(entitlement.availableMicros, '993');
  assert.equal(entitlement.spentMicros, '9007199254740000');
  assert.equal(entitlement.reservedMicros, '0');
  await assert.rejects(f.ledger.settle({ ...settlement, debitMicros: '1' }), code('CONFLICT'));
  await assert.rejects(f.ledger.reserve({ ...f.request(), reserveMicros: 1.1 }), code('INVALID_INPUT'));
});

test('unknown-cost failure holds reserve and concurrency slot until trusted reconciliation', async () => {
  const f = await fixture('100', { maxConcurrent: 1 });
  const request = f.request(randomUUID(), '80');
  await f.ledger.reserve(request);
  await f.ledger.recordGeneration({ ...request, generationId: 'generation-held' });
  await f.ledger.fail(request);
  assert.equal((await f.ledger.entitlement(f)).reservedMicros, '80');
  await assert.rejects(f.ledger.reserve(f.request()), code('CONCURRENCY_LIMIT'));
  const pending = await f.ledger.pendingAttempts({ before: new Date('2100-01-01') });
  assert.ok(pending.some((attempt) => attempt.attemptId === request.attemptId && attempt.generationId === 'generation-held'));
  await assert.rejects(f.ledger.reconcile({ ...request, debitMicros: '81', costMicros: '81' }), code('RESERVATION_EXCEEDED'));
  assert.equal((await f.ledger.entitlement(f)).reservedMicros, '80');
  await f.ledger.reconcile({ ...request, debitMicros: '7', costMicros: '7', generationId: 'generation-held' });
  assert.equal((await f.ledger.entitlement(f)).availableMicros, '93');
  assert.equal((await f.ledger.lookupAttempt(request)).status, 'settled');
});

test('provider cost evidence cannot be assigned to two attempts', async () => {
  const f = await fixture();
  const first = f.request(), second = f.request();
  await f.ledger.reserve(first); await f.ledger.reserve(second);
  const generationId = randomUUID();
  await f.ledger.recordGeneration({ ...first, generationId });
  await assert.rejects(f.ledger.recordGeneration({ ...second, generationId }), code('23505'));
  assert.equal((await f.ledger.lookupAttempt(second)).generationId, null);
  assert.equal((await f.ledger.entitlement(f)).reservedMicros, '20');
});

test('paid-period expiry, failed renewal and later out-of-order payment never reset old usage', async () => {
  const f = await fixture();
  await f.ledger.reserve(f.request(randomUUID(), '25'));
  const future = createLedger(pool, { clock: () => new Date(end) });
  assert.equal((await future.entitlement(f)).active, false);
  await assert.rejects(future.reserve(f.request()), code('NO_PAID_PERIOD'));
  const next = { ...f.grant, invoiceId: randomUUID(), periodStart: end, periodEnd: '2026-11-01T00:00:00Z' };
  await f.ledger.grantPaidPeriod(next);
  assert.equal((await future.entitlement(f)).availableMicros, '100');
  await f.ledger.grantPaidPeriod(f.grant);
  assert.equal((await f.ledger.entitlement(f)).reservedMicros, '25');
});

test('period-end cancellation preserves paid access; immediate cancellation wins delayed events', async () => {
  const f = await fixture();
  await f.ledger.endSubscription({ ...f, effectiveAt: end });
  assert.equal((await f.ledger.entitlement(f)).active, true);
  await f.ledger.reserve(f.request());
  const races = await Promise.allSettled([
    f.ledger.reserve(f.request()),
    f.ledger.endSubscription({ ...f, effectiveAt: instant }),
  ]);
  assert.equal(races[1].status, 'fulfilled');
  await f.ledger.endSubscription({ ...f, effectiveAt: end });
  assert.equal((await f.ledger.entitlement(f)).active, false);
  await assert.rejects(f.ledger.reserve(f.request()), code('NO_PAID_PERIOD'));
});

test('unfunded revocation races safely with reservations and cannot be undone by invoice replay', async () => {
  const f = await fixture();
  const request = f.request();
  await f.ledger.reserve(request);
  await Promise.allSettled([f.ledger.reserve(f.request()), f.ledger.revokePeriod({ ...f, reason: 'disputed' })]);
  await f.ledger.grantPaidPeriod(f.grant);
  assert.equal((await f.ledger.entitlement(f)).active, false);
  await assert.rejects(f.ledger.reserve(f.request()), code('NO_PAID_PERIOD'));
  await f.ledger.settle({ ...request, debitMicros: '3', costMicros: '3' });
  assert.equal((await f.ledger.lookupAttempt(request)).status, 'settled');
});

test('a failure during ledger append rolls back attempt and balance together', async () => {
  const f = await fixture();
  await pool.query(`CREATE FUNCTION reject_test_reserve() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.account_id='${f.accountId}' AND NEW.kind='reserve' THEN RAISE EXCEPTION 'test write failure'; END IF; RETURN NEW; END $$`);
  await pool.query('CREATE TRIGGER reject_test_reserve BEFORE INSERT ON allowance_ledger FOR EACH ROW EXECUTE FUNCTION reject_test_reserve()');
  const request = f.request();
  try {
    await assert.rejects(f.ledger.reserve(request));
    assert.equal((await f.ledger.entitlement(f)).availableMicros, '100');
    await assert.rejects(f.ledger.lookupAttempt(request), code('ATTEMPT_NOT_FOUND'));
  } finally {
    await pool.query('DROP TRIGGER reject_test_reserve ON allowance_ledger');
    await pool.query('DROP FUNCTION reject_test_reserve()');
  }
  assert.equal((await f.ledger.reserve(request)).created, true);
});

test('a pending attempt survives a real worker process exit and reconciles through a new connection', async () => {
  const f = await fixture();
  const request = f.request(randomUUID(), '60');
  const child = spawnSync(process.execPath, [new URL('./restart-worker.mjs', import.meta.url).pathname, JSON.stringify(request)], {
    env: { ...process.env, BENCH_LEDGER_TEST_OPTIONS: options }, encoding: 'utf8', timeout: 15000,
  });
  assert.equal(child.status, 0, child.stderr);
  const restartedPool = new pg.Pool({ connectionString, options });
  try {
    const restarted = createLedger(restartedPool, { clock: () => instant });
    assert.equal((await restarted.lookupAttempt(request)).status, 'pending');
    assert.equal((await restarted.entitlement(f)).reservedMicros, '60');
    await restarted.reconcile({ ...request, debitMicros: '9', costMicros: '9', generationId: request.attemptId });
    assert.equal((await restarted.entitlement(f)).availableMicros, '91');
  } finally { await restartedPool.end(); }
});
