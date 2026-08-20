-- Migration: add chart provenance columns to accounts
-- Two nullable columns identifying which chart-of-accounts dataset (and version) installed
-- a given row. Null = the account was created manually (or by anything other than the
-- chart install engine). The install engine's overlap gate keys on deterministic-id
-- equality plus a matching chart_code (a chart's own rows under earlier numbering also
-- refuse); NULL here is the usual consequence of "not engine-installed", not the
-- predicate itself. accounts is fence-strict; additive nullable columns change nothing
-- about the fence posture.

ALTER TABLE accounting.accounts ADD COLUMN IF NOT EXISTS chart_code TEXT;
ALTER TABLE accounting.accounts ADD COLUMN IF NOT EXISTS chart_version TEXT;

CREATE INDEX IF NOT EXISTS idx_accounts_company_id_chart_code
    ON accounting.accounts (company_id, chart_code)
    WHERE chart_code IS NOT NULL;
