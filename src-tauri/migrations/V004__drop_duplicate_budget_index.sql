-- =========================================================
-- V004: drop duplicate budgets index
-- =========================================================
-- V001 declares UNIQUE(category_id, starts_on) on `budgets`, which SQLite
-- already backs with an automatic index on exactly those two columns
-- (sqlite_autoindex_budgets_1). V003 then added an explicit
-- `idx_budgets_category_starts ON budgets(category_id, starts_on)`, so every
-- budget write maintained two identical B-trees with no lookup benefit.
-- Shipped migration files are immutable (CLAUDE.md rule 6), so the redundant
-- index is dropped here instead of by editing V003.
-- =========================================================

DROP INDEX IF EXISTS idx_budgets_category_starts;
