CREATE TABLE IF NOT EXISTS usage_accounts (
  account_id text PRIMARY KEY,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS usage_subscriptions (
  subscription_id text PRIMARY KEY,
  account_id text NOT NULL REFERENCES usage_accounts(account_id),
  ends_at timestamptz,
  UNIQUE(subscription_id, account_id)
);
CREATE TABLE IF NOT EXISTS usage_periods (
  period_id uuid PRIMARY KEY,
  account_id text NOT NULL REFERENCES usage_accounts(account_id),
  subscription_id text NOT NULL,
  invoice_id text NOT NULL UNIQUE,
  plan_id text NOT NULL,
  starts_at timestamptz NOT NULL,
  ends_at timestamptz NOT NULL CHECK (ends_at > starts_at),
  allowance_micros bigint NOT NULL CHECK (allowance_micros > 0),
  held_micros bigint NOT NULL DEFAULT 0 CHECK (held_micros >= 0),
  spent_micros bigint NOT NULL DEFAULT 0 CHECK (spent_micros >= 0),
  revoked boolean NOT NULL DEFAULT false,
  revoke_reason text,
  CHECK (spent_micros <= allowance_micros - held_micros),
  FOREIGN KEY(subscription_id, account_id) REFERENCES usage_subscriptions(subscription_id, account_id),
  UNIQUE(subscription_id, starts_at, ends_at),
  UNIQUE(period_id, account_id)
);
CREATE INDEX IF NOT EXISTS usage_periods_account_idx ON usage_periods(account_id, starts_at, ends_at);
CREATE TABLE IF NOT EXISTS generation_attempts (
  account_id text NOT NULL REFERENCES usage_accounts(account_id),
  attempt_id text NOT NULL,
  period_id uuid NOT NULL,
  conversation_id text NOT NULL,
  model text NOT NULL,
  request_digest text NOT NULL CHECK (request_digest ~ '^[a-f0-9]{64}$'),
  reserve_micros bigint NOT NULL CHECK (reserve_micros > 0),
  status text NOT NULL CHECK (status IN ('reserved', 'pending', 'settled', 'released')),
  debit_micros bigint CHECK (debit_micros >= 0 AND debit_micros <= reserve_micros),
  cost_micros bigint CHECK (cost_micros >= 0),
  generation_id text UNIQUE,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY(account_id, attempt_id),
  FOREIGN KEY(period_id, account_id) REFERENCES usage_periods(period_id, account_id),
  CHECK ((status IN ('settled', 'released')) = (debit_micros IS NOT NULL AND cost_micros IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS generation_attempts_pending_idx ON generation_attempts(account_id, status);
CREATE TABLE IF NOT EXISTS allowance_ledger (
  entry_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  account_id text NOT NULL REFERENCES usage_accounts(account_id),
  period_id uuid NOT NULL,
  reference text NOT NULL,
  kind text NOT NULL CHECK (kind IN ('grant', 'reserve', 'settle', 'release', 'revoke')),
  allowance_delta bigint NOT NULL DEFAULT 0,
  held_delta bigint NOT NULL DEFAULT 0,
  spent_delta bigint NOT NULL DEFAULT 0,
  created_at timestamptz NOT NULL DEFAULT now(),
  FOREIGN KEY(period_id, account_id) REFERENCES usage_periods(period_id, account_id),
  UNIQUE(period_id, reference)
);
