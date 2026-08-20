-- Migration: drop unused columns from accounts and cost_centers
-- Removes columns no hand-written logic reads or writes: reconciliation stamps the
-- graph computes on read, manual-entry/cost-center/project requirement flags no rule
-- enforces, pre-repartition account-level tax settings (tax routing lives in
-- backbone-tax repartition lines), bank details owned by backbone-banking, and the
-- localization columns (translation is a presentation concern, not a schema column —
-- see docs/refactoring-schema in the serpa workspace). No environment holds real
-- data yet, so the drop is safe; the down migration restores the exact previous
-- shape for rollback. accounts is fence-strict; dropping columns changes nothing
-- about the fence posture.

ALTER TABLE accounting.accounts
    DROP COLUMN IF EXISTS last_reconciled_at,
    DROP COLUMN IF EXISTS last_reconciled_balance,
    DROP COLUMN IF EXISTS allow_manual_entry,
    DROP COLUMN IF EXISTS require_cost_center,
    DROP COLUMN IF EXISTS require_project,
    DROP COLUMN IF EXISTS is_taxable,
    DROP COLUMN IF EXISTS tax_rate,
    DROP COLUMN IF EXISTS tax_account_id,
    DROP COLUMN IF EXISTS bank_name,
    DROP COLUMN IF EXISTS bank_account_number,
    DROP COLUMN IF EXISTS bank_account_name,
    DROP COLUMN IF EXISTS bank_branch,
    DROP COLUMN IF EXISTS name_en;

ALTER TABLE accounting.cost_centers DROP COLUMN IF EXISTS name_en;
