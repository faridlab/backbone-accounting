-- Down: restore the dropped columns with their exact previous shape (types and
-- defaults from the original create migrations). Data is NOT restored — no
-- environment holds real data; this exists so the migration pair is reversible
-- at the schema level.

ALTER TABLE accounting.accounts
    ADD COLUMN IF NOT EXISTS last_reconciled_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS last_reconciled_balance NUMERIC(18, 2),
    ADD COLUMN IF NOT EXISTS allow_manual_entry BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS require_cost_center BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS require_project BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS is_taxable BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS tax_rate NUMERIC(5, 2),
    ADD COLUMN IF NOT EXISTS tax_account_id UUID,
    ADD COLUMN IF NOT EXISTS bank_name TEXT,
    ADD COLUMN IF NOT EXISTS bank_account_number TEXT,
    ADD COLUMN IF NOT EXISTS bank_account_name TEXT,
    ADD COLUMN IF NOT EXISTS bank_branch TEXT,
    ADD COLUMN IF NOT EXISTS name_en TEXT;

ALTER TABLE accounting.accounts
    DROP CONSTRAINT IF EXISTS fk_accounts_tax_account_id;
ALTER TABLE accounting.accounts
    ADD CONSTRAINT fk_accounts_tax_account_id
    FOREIGN KEY (tax_account_id) REFERENCES accounting.accounts (id);

ALTER TABLE accounting.cost_centers ADD COLUMN IF NOT EXISTS name_en TEXT;
