-- Migration: drop chart provenance columns from accounts

DROP INDEX IF EXISTS accounting.idx_accounts_company_id_chart_code;
ALTER TABLE accounting.accounts DROP COLUMN IF EXISTS chart_version;
ALTER TABLE accounting.accounts DROP COLUMN IF EXISTS chart_code;
