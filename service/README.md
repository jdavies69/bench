# Bench service accounting

This private Node service package contains internal generation admission, coordination, and PostgreSQL accounting modules. It is not deployed and exposes no HTTP listener, sign-in, Checkout, or live inference. Desktop BYOK remains the working path.

Install with `npm ci` in this directory. `npm run test:unit` uses fake provider adapters. `npm run test:ledger` requires the isolated test PostgreSQL database described by the ledger test instructions; `npm test` runs both. Never point integration tests at customer or production data.

The application layer must authenticate the account before calling these modules. Only a verified paid invoice may grant allowance. Browser return URLs cannot grant usage. Trusted server configuration chooses models, full-prompt token admission, prices, and output limits; the client cannot override them. Amounts are exact integer micro-units represented by decimal strings.

Reservations precede provider dispatch. Reusing an attempt cannot dispatch again; uncertain failures retain their reservation until reconciliation. A trusted provider adapter must honor cancellation, enforce the admitted output cap, identify the provider generation, and provide verified cost. No live rates or model defaults are configured here.

Before managed access can ship, implement and verify account authorization, Stripe event verification and ordering, provider cost reconciliation, durable bounded response recovery, migration operations, hosted HTTPS configuration, monitoring, and the Rust desktop contract. The public UI must not advertise working managed access before these paths are usable. See [managed usage](../docs/managed-usage-milestone.md), [desktop contract](../docs/managed-desktop-contract.md), and [release gates](../docs/public-release.md).

Transaction and locking design follows the [node-postgres transaction contract](https://node-postgres.com/features/transactions) and [PostgreSQL row locking](https://www.postgresql.org/docs/17/explicit-locking.html).
