import pg from 'pg';
import { createLedger } from './index.js';

const pool = new pg.Pool({ connectionString: process.env.BENCH_LEDGER_TEST_URL, options: process.env.BENCH_LEDGER_TEST_OPTIONS });
try {
  const request = JSON.parse(process.argv[2]);
  const ledger = createLedger(pool, { clock: () => new Date('2026-09-30T12:00:00Z') });
  await ledger.reserve(request);
  await ledger.fail({ ...request, generationId: request.attemptId });
} finally { await pool.end(); }
