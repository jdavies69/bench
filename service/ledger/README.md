# Managed usage ledger

Internal PostgreSQL service module; no production account or payment is configured. `migrateLedger(pool)` applies the initial transaction-safe schema; `createLedger(pool, { clock, maxConcurrent })` accepts a `pg.Pool`-compatible dependency. Keep PostgreSQL `bigint` values as strings with the default `pg` parser. Amount arguments accept decimal strings or BigInt; floating-point Number amounts are rejected.

`grantPaidPeriod` is called **only after a trusted payment adapter confirms a paid invoice and reconciles current subscription state**. It grants once per invoice and rejects overlapping periods. All mutating operations lock the account row, so concurrent reservations, revocation, cancellation and settlement share one transaction order. No renewal without a confirmed paid invoice, no rollover and no overage billing.

`reserve` binds account, attempt ID, conversation ID, model, request SHA-256 digest and reservation amount. Its `created` flag is the exclusive permission to dispatch the provider request. A replay returns metadata with `created:false` even after settlement; a changed request conflicts. Only internal trusted code may supply authenticated account IDs or reserve amounts. The gateway must bound provider spending to the reserve before dispatch.

`recordGeneration` stores the provider generation ID immediately when available, uniquely across attempts. `settle`/`reconcile` accept verified provider cost and allowance debit; the gateway owns the approved billing conversion and these values must never come from a desktop request. The initial cost-denominated policy uses identical USD micro-units for both. Debit above the reserved bound fails closed and retains the hold for investigation. `fail` holds unknown cost as pending; only explicitly verified zero-cost failure releases the hold. `pendingAttempts` includes both pending and reserved attempts, including workers that crashed before recording a generation ID. There is no timeout-based refund.

`lookupAttempt` is account-scoped. Entitlement reads only current, funded, unrevoked periods. `endSubscription` sets an effective end and never extends an earlier cancellation from a delayed event; reactivation needs an explicit future adapter design. `revokePeriod` disables unfunded usage without discarding outstanding cost reconciliation. A duplicate paid invoice cannot reverse revocation. The ledger stores billing metadata and request digests, never prompt/response bodies or credentials.

Run integration tests against a **temporary, local** PostgreSQL cluster. They fail if `BENCH_LEDGER_TEST_URL` is absent and refuse remote hosts. Each run creates/drops its own random schema. For example, with a cluster started on port 55437:

```sh
BENCH_LEDGER_TEST_URL=postgresql://jfd@127.0.0.1:55437/postgres node --test ledger/ledger.test.mjs
```

Tests use real transaction races, exact amounts above JavaScript's safe-integer limit, injected database-write failure, payment/cancellation/revocation boundaries, unknown-cost holds, and a separate worker process that exits before reconciliation. Production deployment still needs authenticated adapters, service-role grants that prevent arbitrary ledger mutation, versioned incremental migration execution beyond this initial schema, reconciliation workers/provider evidence, and approved plan economics. This module alone does not authorize live subscriptions or inference.
