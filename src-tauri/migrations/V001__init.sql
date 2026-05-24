-- =========================================================
-- V001: initial schema
-- =========================================================
-- All monetary values are stored as INTEGER (yen, no decimals).
-- Soft-delete via `archived_at` (ISO 8601 string); never DELETE rows that
-- may have history (categories / accounts).
-- =========================================================

PRAGMA foreign_keys = ON;

-- ---------------------------------------------------------
-- accounts: cash / bank / credit_card / e_money / investment
-- ---------------------------------------------------------
CREATE TABLE accounts (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  name            TEXT    NOT NULL,
  kind            TEXT    NOT NULL CHECK(kind IN ('cash','bank','credit_card','e_money','investment')),
  currency        TEXT    NOT NULL DEFAULT 'JPY',
  initial_balance INTEGER NOT NULL DEFAULT 0,
  display_order   INTEGER NOT NULL DEFAULT 0,
  note            TEXT    NOT NULL DEFAULT '',
  archived_at     TEXT,
  created_at      TEXT    NOT NULL,
  updated_at      TEXT    NOT NULL
);

-- ---------------------------------------------------------
-- categories: income / expense
-- ---------------------------------------------------------
CREATE TABLE categories (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  name          TEXT    NOT NULL,
  type          TEXT    NOT NULL CHECK(type IN ('income','expense')),
  color         TEXT,
  icon          TEXT,
  display_order INTEGER NOT NULL DEFAULT 0,
  archived_at   TEXT,
  UNIQUE(name, type)
);

-- ---------------------------------------------------------
-- recurring_rules: must come before `transactions` because
-- `transactions.recurring_id` references it.
-- ---------------------------------------------------------
CREATE TABLE recurring_rules (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  name               TEXT    NOT NULL,
  type               TEXT    NOT NULL CHECK(type IN ('income','expense','transfer')),
  amount             INTEGER NOT NULL CHECK(amount > 0),
  account_id         INTEGER NOT NULL REFERENCES accounts(id),
  counter_account_id INTEGER REFERENCES accounts(id),
  category_id        INTEGER REFERENCES categories(id),
  description        TEXT    NOT NULL DEFAULT '',
  frequency          TEXT    NOT NULL CHECK(frequency IN ('monthly','weekly','yearly')),
  day_of_month       INTEGER,
  day_of_week        INTEGER,
  starts_on          TEXT    NOT NULL,
  ends_on            TEXT,
  last_generated_on  TEXT,
  active             INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1))
);

-- ---------------------------------------------------------
-- transactions: income / expense / transfer
-- ---------------------------------------------------------
CREATE TABLE transactions (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  occurred_on        TEXT    NOT NULL,
  type               TEXT    NOT NULL CHECK(type IN ('income','expense','transfer')),
  amount             INTEGER NOT NULL CHECK(amount > 0),
  account_id         INTEGER NOT NULL REFERENCES accounts(id),
  counter_account_id INTEGER REFERENCES accounts(id),
  category_id        INTEGER REFERENCES categories(id),
  description        TEXT    NOT NULL DEFAULT '',
  recurring_id       INTEGER REFERENCES recurring_rules(id) ON DELETE SET NULL,
  created_at         TEXT    NOT NULL,
  updated_at         TEXT    NOT NULL,
  -- transfer の場合は counter_account_id 必須・category_id NULL
  CHECK(
    (type = 'transfer' AND counter_account_id IS NOT NULL AND category_id IS NULL)
    OR (type IN ('income','expense') AND counter_account_id IS NULL AND category_id IS NOT NULL)
  )
);
CREATE INDEX idx_tx_occurred_on ON transactions(occurred_on);
CREATE INDEX idx_tx_account     ON transactions(account_id);
CREATE INDEX idx_tx_category    ON transactions(category_id);

-- ---------------------------------------------------------
-- budgets: per-category monthly/yearly budgets
-- ---------------------------------------------------------
CREATE TABLE budgets (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  category_id     INTEGER NOT NULL REFERENCES categories(id),
  period          TEXT    NOT NULL CHECK(period IN ('monthly','yearly')),
  amount          INTEGER NOT NULL CHECK(amount >= 0),
  starts_on       TEXT    NOT NULL,
  ends_on         TEXT,
  alert_threshold INTEGER NOT NULL DEFAULT 80 CHECK(alert_threshold BETWEEN 0 AND 200),
  UNIQUE(category_id, starts_on)
);

-- ---------------------------------------------------------
-- app_meta: key/value store for schema_version, last_backup_at, theme, ...
-- ---------------------------------------------------------
CREATE TABLE app_meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
