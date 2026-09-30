import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';

const MAX_INT64 = 9223372036854775807n;

export class LedgerError extends Error {
  constructor(code, message) {
    super(message);
    this.name = 'LedgerError';
    this.code = code;
  }
}

function reject(code, message) { throw new LedgerError(code, message); }
function id(value, label) {
  if (typeof value !== 'string' || !value.trim() || value.length > 200 || /[\u0000-\u001f]/.test(value)) {
    reject('INVALID_INPUT', `Invalid ${label}.`);
  }
  return value;
}
function micros(value, positive = false) {
  if ((typeof value !== 'bigint' && typeof value !== 'string') || !/^\d+$/.test(String(value))) {
    reject('INVALID_INPUT', 'Amounts must be exact integer micro-units.');
  }
  const amount = BigInt(value);
  if (amount < (positive ? 1n : 0n) || amount > MAX_INT64) reject('INVALID_INPUT', 'Amount is outside the supported range.');
  return amount.toString();
}
function timestamp(value) {
  const time = value instanceof Date ? value : new Date(value);
  if (typeof value !== 'string' && !(value instanceof Date) || !Number.isFinite(time.getTime())) {
    reject('INVALID_INPUT', 'Invalid billing timestamp.');
  }
  return time;
}

async function transaction(pool, work) {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');
    const result = await work(client);
    await client.query('COMMIT');
    return result;
  } catch (error) {
    await client.query('ROLLBACK').catch(() => {});
    throw error;
  } finally { client.release(); }
}

export async function migrateLedger(pool) {
  const sql = await readFile(new URL('./001-ledger.sql', import.meta.url), 'utf8');
  return transaction(pool, async (client) => {
    await client.query('SELECT pg_advisory_xact_lock(1847019264)');
    await client.query(sql);
  });
}

function attemptView(row) {
  return {
    accountId: row.account_id, attemptId: row.attempt_id, periodId: row.period_id,
    conversationId: row.conversation_id, model: row.model, status: row.status,
    requestDigest: row.request_digest,
    reserveMicros: row.reserve_micros, debitMicros: row.debit_micros,
    costMicros: row.cost_micros, generationId: row.generation_id,
  };
}

// This module is an internal service boundary, not an unauthenticated HTTP API.
// Caller authenticates accountId, verifies paid invoices/current subscription
// state, and enforces model/output/spend bounds before invoking these methods.
export function createLedger(pool, { clock = () => new Date(), maxConcurrent = 2 } = {}) {
  if (!Number.isSafeInteger(maxConcurrent) || maxConcurrent < 1 || maxConcurrent > 100) reject('INVALID_INPUT', 'Invalid concurrency limit.');
  const now = () => timestamp(clock());
  const lockAccount = async (client, accountId) => {
    const result = await client.query('SELECT account_id FROM usage_accounts WHERE account_id=$1 FOR UPDATE', [accountId]);
    if (!result.rowCount) reject('NO_PAID_PERIOD', 'Managed usage requires a current paid period.');
  };
  const entry = (client, period, reference, kind, allowance = '0', held = '0', spent = '0') => client.query(
    'INSERT INTO allowance_ledger(account_id,period_id,reference,kind,allowance_delta,held_delta,spent_delta) VALUES($1,$2,$3,$4,$5,$6,$7)',
    [period.account_id, period.period_id, reference, kind, allowance, held, spent],
  );
  const getAttempt = async (client, accountId, attemptId) => {
    const result = await client.query('SELECT * FROM generation_attempts WHERE account_id=$1 AND attempt_id=$2', [accountId, attemptId]);
    if (!result.rowCount) reject('ATTEMPT_NOT_FOUND', 'This generation attempt is unavailable.');
    return result.rows[0];
  };
  const currentPeriod = async (client, accountId) => {
    const result = await client.query(
      `SELECT p.* FROM usage_periods p JOIN usage_subscriptions s USING(subscription_id,account_id)
       WHERE p.account_id=$1 AND NOT p.revoked AND p.starts_at <= $2 AND p.ends_at > $2
       AND (s.ends_at IS NULL OR s.ends_at > $2) ORDER BY p.starts_at DESC LIMIT 1`, [accountId, now()],
    );
    return result.rows[0];
  };

  async function grantPaidPeriod(input) {
    const accountId = id(input.accountId, 'account');
    const subscriptionId = id(input.subscriptionId, 'subscription');
    const invoiceId = id(input.invoiceId, 'invoice');
    const planId = id(input.planId, 'plan');
    const allowance = micros(input.allowanceMicros, true);
    const start = timestamp(input.periodStart), end = timestamp(input.periodEnd);
    if (end <= start) reject('INVALID_INPUT', 'Billing period must have a positive duration.');
    return transaction(pool, async (client) => {
      await client.query('INSERT INTO usage_accounts(account_id) VALUES($1) ON CONFLICT DO NOTHING', [accountId]);
      await lockAccount(client, accountId);
      await client.query('INSERT INTO usage_subscriptions(subscription_id,account_id) VALUES($1,$2) ON CONFLICT DO NOTHING', [subscriptionId, accountId]);
      const subscription = await client.query('SELECT account_id FROM usage_subscriptions WHERE subscription_id=$1', [subscriptionId]);
      if (subscription.rows[0].account_id !== accountId) reject('CONFLICT', 'Subscription already belongs to another account.');
      const existing = await client.query('SELECT * FROM usage_periods WHERE invoice_id=$1', [invoiceId]);
      if (existing.rowCount) {
        const period = existing.rows[0];
        if (period.account_id !== accountId || period.subscription_id !== subscriptionId || period.plan_id !== planId || period.allowance_micros !== allowance || +period.starts_at !== +start || +period.ends_at !== +end) {
          reject('CONFLICT', 'Paid invoice does not match the recorded allowance.');
        }
        return { periodId: period.period_id, created: false };
      }
      const overlap = await client.query('SELECT 1 FROM usage_periods WHERE account_id=$1 AND starts_at < $3 AND ends_at > $2', [accountId, start, end]);
      if (overlap.rowCount) reject('CONFLICT', 'An allowance already covers this billing period.');
      const period = (await client.query(
        `INSERT INTO usage_periods(period_id,account_id,subscription_id,invoice_id,plan_id,starts_at,ends_at,allowance_micros)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING *`, [randomUUID(), accountId, subscriptionId, invoiceId, planId, start, end, allowance],
      )).rows[0];
      await entry(client, period, 'grant', 'grant', allowance);
      return { periodId: period.period_id, created: true };
    });
  }

  async function entitlement({ accountId }) {
    id(accountId, 'account');
    const client = await pool.connect();
    try {
      const period = await currentPeriod(client, accountId);
      if (!period) return { active: false, availableMicros: '0', reservedMicros: '0', spentMicros: '0' };
      return {
        active: true, periodId: period.period_id, planId: period.plan_id,
        periodStart: period.starts_at.toISOString(), periodEnd: period.ends_at.toISOString(),
        allowanceMicros: period.allowance_micros, reservedMicros: period.held_micros, spentMicros: period.spent_micros,
        availableMicros: (BigInt(period.allowance_micros) - BigInt(period.held_micros) - BigInt(period.spent_micros)).toString(),
      };
    } finally { client.release(); }
  }

  async function reserve(input) {
    const accountId = id(input.accountId, 'account'), attemptId = id(input.attemptId, 'attempt');
    const conversationId = id(input.conversationId, 'conversation'), model = id(input.model, 'model');
    const requestDigest = input.requestDigest;
    if (typeof requestDigest !== 'string' || !/^[a-f0-9]{64}$/.test(requestDigest)) reject('INVALID_INPUT', 'Invalid request digest.');
    const reserve = micros(input.reserveMicros, true);
    return transaction(pool, async (client) => {
      await lockAccount(client, accountId);
      const existing = await client.query('SELECT * FROM generation_attempts WHERE account_id=$1 AND attempt_id=$2', [accountId, attemptId]);
      if (existing.rowCount) {
        const attempt = existing.rows[0];
        if (attempt.conversation_id !== conversationId || attempt.model !== model || attempt.reserve_micros !== reserve || attempt.request_digest !== requestDigest) reject('CONFLICT', 'Attempt ID was already used for another request.');
        // Never contact the provider again for created:false, including pending
        // or released attempts. A user retry must have a fresh attempt ID.
        return { attempt: attemptView(attempt), created: false };
      }
      const period = await currentPeriod(client, accountId);
      if (!period) reject('NO_PAID_PERIOD', 'Managed usage requires a current paid period.');
      const active = await client.query("SELECT count(*)::int AS count FROM generation_attempts WHERE account_id=$1 AND status IN ('reserved','pending')", [accountId]);
      if (active.rows[0].count >= maxConcurrent) reject('CONCURRENCY_LIMIT', 'Wait for earlier usage to finish or reconcile.');
      const updated = await client.query(
        `UPDATE usage_periods SET held_micros=held_micros+$2 WHERE period_id=$1
         AND $2 <= allowance_micros-spent_micros-held_micros RETURNING *`, [period.period_id, reserve],
      );
      if (!updated.rowCount) reject('ALLOWANCE_EXHAUSTED', 'This request exceeds the remaining included usage.');
      const attempt = (await client.query(
        `INSERT INTO generation_attempts(account_id,attempt_id,period_id,conversation_id,model,reserve_micros,request_digest,status)
         VALUES($1,$2,$3,$4,$5,$6,$7,'reserved') RETURNING *`, [accountId, attemptId, period.period_id, conversationId, model, reserve, requestDigest],
      )).rows[0];
      await entry(client, period, `reserve:${attemptId}`, 'reserve', '0', reserve);
      return { attempt: attemptView(attempt), created: true };
    });
  }

  async function settle(input) {
    const accountId = id(input.accountId, 'account'), attemptId = id(input.attemptId, 'attempt');
    const debit = micros(input.debitMicros), cost = micros(input.costMicros);
    const generationId = input.generationId == null ? null : id(input.generationId, 'generation');
    return transaction(pool, async (client) => {
      await lockAccount(client, accountId);
      const attempt = await getAttempt(client, accountId, attemptId);
      if (['settled', 'released'].includes(attempt.status)) {
        if (attempt.debit_micros !== debit || attempt.cost_micros !== cost || generationId && attempt.generation_id !== generationId) reject('CONFLICT', 'Attempt was already settled with different usage.');
        return attemptView(attempt);
      }
      if (BigInt(debit) > BigInt(attempt.reserve_micros)) reject('RESERVATION_EXCEEDED', 'Actual usage exceeds its reserved bound; reconciliation is required.');
      if (generationId && attempt.generation_id && generationId !== attempt.generation_id) reject('CONFLICT', 'Provider generation does not match this attempt.');
      await client.query('UPDATE usage_periods SET held_micros=held_micros-$2,spent_micros=spent_micros+$3 WHERE period_id=$1', [attempt.period_id, attempt.reserve_micros, debit]);
      const final = (await client.query(
        `UPDATE generation_attempts SET status=$3,debit_micros=$4,cost_micros=$5,generation_id=COALESCE($6,generation_id),updated_at=now()
         WHERE account_id=$1 AND attempt_id=$2 RETURNING *`, [accountId, attemptId, debit === '0' && cost === '0' ? 'released' : 'settled', debit, cost, generationId],
      )).rows[0];
      await entry(client, attempt, `final:${attemptId}`, final.status === 'released' ? 'release' : 'settle', '0', `-${attempt.reserve_micros}`, debit);
      return attemptView(final);
    });
  }

  async function fail(input) {
    if (input.costKnownZero === true) return settle({ ...input, debitMicros: '0', costMicros: '0' });
    const accountId = id(input.accountId, 'account'), attemptId = id(input.attemptId, 'attempt');
    const generationId = input.generationId == null ? null : id(input.generationId, 'generation');
    return transaction(pool, async (client) => {
      await lockAccount(client, accountId);
      const attempt = await getAttempt(client, accountId, attemptId);
      if (generationId && attempt.generation_id && generationId !== attempt.generation_id) reject('CONFLICT', 'Provider generation does not match this attempt.');
      if (['settled', 'released'].includes(attempt.status)) return attemptView(attempt);
      const updated = await client.query("UPDATE generation_attempts SET status='pending',generation_id=COALESCE($3,generation_id),updated_at=now() WHERE account_id=$1 AND attempt_id=$2 RETURNING *", [accountId, attemptId, generationId]);
      return attemptView(updated.rows[0]);
    });
  }

  async function endSubscription({ accountId, subscriptionId, effectiveAt }) {
    id(accountId, 'account'); id(subscriptionId, 'subscription');
    const end = timestamp(effectiveAt);
    return transaction(pool, async (client) => {
      await lockAccount(client, accountId);
      const result = await client.query('UPDATE usage_subscriptions SET ends_at=LEAST(ends_at,$3::timestamptz) WHERE account_id=$1 AND subscription_id=$2 RETURNING subscription_id', [accountId, subscriptionId, end]);
      if (!result.rowCount) reject('SUBSCRIPTION_NOT_FOUND', 'Subscription is unavailable.');
    });
  }

  async function revokePeriod({ accountId, invoiceId, reason }) {
    id(accountId, 'account'); id(invoiceId, 'invoice'); id(reason, 'revocation reason');
    return transaction(pool, async (client) => {
      await lockAccount(client, accountId);
      const result = await client.query('SELECT * FROM usage_periods WHERE account_id=$1 AND invoice_id=$2', [accountId, invoiceId]);
      if (!result.rowCount) reject('PERIOD_NOT_FOUND', 'Paid period is unavailable.');
      const period = result.rows[0];
      if (period.revoked) return;
      await client.query('UPDATE usage_periods SET revoked=true,revoke_reason=$2 WHERE period_id=$1', [period.period_id, reason]);
      await entry(client, period, 'revoke', 'revoke');
    });
  }

  async function recordGeneration({ accountId, attemptId, generationId }) {
    id(accountId, 'account'); id(attemptId, 'attempt'); id(generationId, 'generation');
    return transaction(pool, async (client) => {
      await lockAccount(client, accountId);
      const attempt = await getAttempt(client, accountId, attemptId);
      if (attempt.generation_id && attempt.generation_id !== generationId) reject('CONFLICT', 'Provider generation does not match this attempt.');
      const updated = await client.query('UPDATE generation_attempts SET generation_id=$3,updated_at=now() WHERE account_id=$1 AND attempt_id=$2 RETURNING *', [accountId, attemptId, generationId]);
      return attemptView(updated.rows[0]);
    });
  }

  async function pendingAttempts({ before, limit = 100 }) {
    const cutoff = timestamp(before);
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 1000) reject('INVALID_INPUT', 'Invalid reconciliation batch size.');
    const result = await pool.query("SELECT * FROM generation_attempts WHERE status IN ('reserved','pending') AND updated_at <= $1 ORDER BY updated_at,account_id,attempt_id LIMIT $2", [cutoff, limit]);
    return result.rows.map(attemptView);
  }

  async function lookupAttempt({ accountId, attemptId }) {
    id(accountId, 'account'); id(attemptId, 'attempt');
    const client = await pool.connect();
    try { return attemptView(await getAttempt(client, accountId, attemptId)); }
    finally { client.release(); }
  }

  return { grantPaidPeriod, entitlement, reserve, settle, fail, reconcile: settle, endSubscription, revokePeriod, recordGeneration, pendingAttempts, lookupAttempt };
}
