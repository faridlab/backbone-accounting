-- Hand-authored (user-owned). Not regenerated.
--
-- Best-effort restore sketch for the tenancy strip (ADR-0029). This is a breaking module
-- release against dev-stage databases: the down re-adds the company_id column as nullable
-- with the company-leading indexes in their final pre-strip shapes, but restores NO data —
-- rows written after the strip (or after the decorator re-keyed them) carry org_unit_id
-- only. The composing service's tenancy decorator remains the live fence; the
-- <table>_company_isolation policies are NOT recreated here. Treat this down as a
-- schema-shape sketch for archaeology, not a usable rollback.

ALTER TABLE accounting.accounts               ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.fiscal_periods         ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.journals               ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.journal_lines          ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.accounting_posts       ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.cost_centers           ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.financial_statements   ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.reconciliations        ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.reconciliation_items   ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.ledgers                ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.full_reconciles        ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.partial_reconciles     ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.emv_qr_configs         ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.bank_check_sequences   ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.printed_checks         ADD COLUMN IF NOT EXISTS company_id uuid;
ALTER TABLE accounting.tax_tag_repair_runs    ADD COLUMN IF NOT EXISTS company_id uuid;

-- ── accounts ───────────────────────────────────────────────────────────────────
CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_company_id_account_number
    ON accounting.accounts (company_id, account_number) WHERE (metadata->>'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_company_id_account_code
    ON accounting.accounts (company_id, account_code) WHERE (metadata->>'deleted_at') IS NULL;
CREATE INDEX IF NOT EXISTS idx_accounts_company_id_chart_code
    ON accounting.accounts (company_id, chart_code)
    WHERE chart_code IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_accounts_company_id_account_type_status
    ON accounting.accounts (company_id, account_type, status);
CREATE INDEX IF NOT EXISTS idx_accounts_company_id_parent_id_sort_order
    ON accounting.accounts (company_id, parent_id, sort_order);
CREATE INDEX IF NOT EXISTS idx_accounts_company_id_account_subtype
    ON accounting.accounts (company_id, account_subtype);
CREATE INDEX IF NOT EXISTS idx_accounts_company_id_is_reconcilable
    ON accounting.accounts (company_id, is_reconcilable);

-- ── fiscal_periods ─────────────────────────────────────────────────────────────
CREATE UNIQUE INDEX IF NOT EXISTS idx_fiscal_periods_company_id_period_code
    ON accounting.fiscal_periods (company_id, period_code) WHERE (metadata->>'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_fiscal_periods_company_id_fiscal_year_fiscal_month
    ON accounting.fiscal_periods (company_id, fiscal_year, fiscal_month) WHERE (metadata->>'deleted_at') IS NULL AND fiscal_month IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_fiscal_periods_company_id_fiscal_year
    ON accounting.fiscal_periods (company_id, fiscal_year);
CREATE INDEX IF NOT EXISTS idx_fiscal_periods_company_id_status
    ON accounting.fiscal_periods (company_id, status);
CREATE INDEX IF NOT EXISTS idx_fiscal_periods_company_id_start_date_end_date
    ON accounting.fiscal_periods (company_id, start_date, end_date);
CREATE INDEX IF NOT EXISTS idx_fiscal_periods_company_id_is_current
    ON accounting.fiscal_periods (company_id, is_current);

-- ── journals ───────────────────────────────────────────────────────────────────
CREATE UNIQUE INDEX IF NOT EXISTS idx_journals_company_id_journal_number
    ON accounting.journals (company_id, journal_number) WHERE (metadata->>'deleted_at') IS NULL;
CREATE INDEX IF NOT EXISTS idx_journals_company_id_transaction_date
    ON accounting.journals (company_id, transaction_date);
CREATE INDEX IF NOT EXISTS idx_journals_company_id_journal_type_status
    ON accounting.journals (company_id, journal_type, status);
CREATE INDEX IF NOT EXISTS idx_journals_company_id_fiscal_year_fiscal_month
    ON accounting.journals (company_id, fiscal_year, fiscal_month);
CREATE INDEX IF NOT EXISTS idx_journals_company_id_status_transaction_date
    ON accounting.journals (company_id, status, transaction_date);
CREATE INDEX IF NOT EXISTS idx_journals_company_id_is_reversed
    ON accounting.journals (company_id, is_reversed);
CREATE INDEX IF NOT EXISTS idx_journals_company_id_branch_id_transaction_date
    ON accounting.journals (company_id, branch_id, transaction_date);

-- ── journal_lines ──────────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_journal_lines_company_id_account_id
    ON accounting.journal_lines (company_id, account_id);
CREATE INDEX IF NOT EXISTS idx_journal_lines_company_id_is_reconciled
    ON accounting.journal_lines (company_id, is_reconciled);
CREATE INDEX IF NOT EXISTS idx_journal_lines_company_id_party_type_party_id_account_id
    ON accounting.journal_lines (company_id, party_type, party_id, account_id) WHERE party_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_journal_lines_company_id_full_reconcile_id
    ON accounting.journal_lines (company_id, full_reconcile_id)
    WHERE full_reconcile_id IS NOT NULL;

-- ── accounting_posts ───────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.uq_accounting_posts_idempotency_key;
DROP INDEX IF EXISTS accounting.uq_accounting_posts_source_tuple_keyless;
CREATE UNIQUE INDEX IF NOT EXISTS idx_accounting_posts_company_id_source_type_source_id_posting_type
    ON accounting.accounting_posts (company_id, source_type, source_id, posting_type)
    WHERE idempotency_key IS NULL AND posting_status = 'posted' AND (metadata->>'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_accounting_posts_company_idempotency_key
    ON accounting.accounting_posts (company_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL AND posting_status = 'posted' AND (metadata->>'deleted_at') IS NULL;
CREATE INDEX IF NOT EXISTS idx_accounting_posts_company_id_source_type_posting_status
    ON accounting.accounting_posts (company_id, source_type, posting_status);
CREATE INDEX IF NOT EXISTS idx_accounting_posts_company_id_branch_id_posting_status
    ON accounting.accounting_posts (company_id, branch_id, posting_status);
CREATE INDEX IF NOT EXISTS idx_accounting_posts_company_id_posted_at
    ON accounting.accounting_posts (company_id, posted_at) WHERE posted_at IS NOT NULL;

-- ── cost_centers ───────────────────────────────────────────────────────────────
CREATE UNIQUE INDEX IF NOT EXISTS idx_cost_centers_company_id_code
    ON accounting.cost_centers (company_id, code) WHERE (metadata->>'deleted_at') IS NULL;
CREATE INDEX IF NOT EXISTS idx_cost_centers_company_id_parent_id_sort_order
    ON accounting.cost_centers (company_id, parent_id, sort_order);
CREATE INDEX IF NOT EXISTS idx_cost_centers_company_id_status
    ON accounting.cost_centers (company_id, status);

-- ── financial_statements ───────────────────────────────────────────────────────
CREATE UNIQUE INDEX IF NOT EXISTS idx_financial_statements_company_id_statement_number
    ON accounting.financial_statements (company_id, statement_number) WHERE (metadata->>'deleted_at') IS NULL;
CREATE INDEX IF NOT EXISTS idx_financial_statements_company_id_statement_type_fiscal_year
    ON accounting.financial_statements (company_id, statement_type, fiscal_year);
CREATE INDEX IF NOT EXISTS idx_financial_statements_company_id_fiscal_period_id
    ON accounting.financial_statements (company_id, fiscal_period_id);
CREATE INDEX IF NOT EXISTS idx_financial_statements_company_id_status
    ON accounting.financial_statements (company_id, status);
CREATE INDEX IF NOT EXISTS idx_financial_statements_company_id_as_of_date
    ON accounting.financial_statements (company_id, as_of_date);

-- ── reconciliations ────────────────────────────────────────────────────────────
CREATE UNIQUE INDEX IF NOT EXISTS idx_reconciliations_company_id_reconciliation_number
    ON accounting.reconciliations (company_id, reconciliation_number) WHERE (metadata->>'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_reconciliations_company_id_account_id_statement_date
    ON accounting.reconciliations (company_id, account_id, statement_date) WHERE (metadata->>'deleted_at') IS NULL;
CREATE INDEX IF NOT EXISTS idx_reconciliations_company_id_account_id_period_start
    ON accounting.reconciliations (company_id, account_id, period_start);
CREATE INDEX IF NOT EXISTS idx_reconciliations_company_id_status
    ON accounting.reconciliations (company_id, status);
CREATE INDEX IF NOT EXISTS idx_reconciliations_company_id_is_balanced
    ON accounting.reconciliations (company_id, is_balanced);

-- ── reconciliation_items ───────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_reconciliation_items_company_id_is_outstanding
    ON accounting.reconciliation_items (company_id, is_outstanding);

-- ── ledgers ────────────────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_ledgers_company_id_account_id_transaction_date
    ON accounting.ledgers (company_id, account_id, transaction_date);
CREATE INDEX IF NOT EXISTS idx_ledgers_company_id_account_id_sequence_number
    ON accounting.ledgers (company_id, account_id, sequence_number);
CREATE INDEX IF NOT EXISTS idx_ledgers_company_id_fiscal_year_fiscal_month
    ON accounting.ledgers (company_id, fiscal_year, fiscal_month);
CREATE INDEX IF NOT EXISTS idx_ledgers_company_id_transaction_date
    ON accounting.ledgers (company_id, transaction_date);
CREATE INDEX IF NOT EXISTS idx_ledgers_company_id_account_id_is_reconciled
    ON accounting.ledgers (company_id, account_id, is_reconciled);
CREATE INDEX IF NOT EXISTS idx_ledgers_company_id_is_opening_balance
    ON accounting.ledgers (company_id, is_opening_balance);
CREATE INDEX IF NOT EXISTS idx_ledgers_company_id_is_closing_entry
    ON accounting.ledgers (company_id, is_closing_entry);
CREATE INDEX IF NOT EXISTS idx_ledgers_company_id_branch_id_account_id_transaction_date
    ON accounting.ledgers (company_id, branch_id, account_id, transaction_date);
CREATE INDEX IF NOT EXISTS idx_ledgers_company_id_party_type_party_id_account_id_transaction_date
    ON accounting.ledgers (company_id, party_type, party_id, account_id, transaction_date) WHERE party_id IS NOT NULL;

-- ── full_reconciles ────────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_full_reconciles_company_id_reconciled_at
    ON accounting.full_reconciles (company_id, reconciled_at);

-- ── partial_reconciles ─────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_partial_reconciles_company_id_debit_move_id
    ON accounting.partial_reconciles (company_id, debit_move_id);
CREATE INDEX IF NOT EXISTS idx_partial_reconciles_company_id_credit_move_id
    ON accounting.partial_reconciles (company_id, credit_move_id);
CREATE INDEX IF NOT EXISTS idx_partial_reconciles_company_id_source_type_source_id
    ON accounting.partial_reconciles (company_id, source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_partial_reconciles_company_id_max_date
    ON accounting.partial_reconciles (company_id, max_date);

-- ── emv_qr_configs ─────────────────────────────────────────────────────────────
CREATE UNIQUE INDEX IF NOT EXISTS uq_emv_qr_configs_company_bank_slot
    ON accounting.emv_qr_configs (
        company_id,
        COALESCE(bank_account_id, '00000000-0000-0000-0000-000000000000'::uuid)
    );
CREATE INDEX IF NOT EXISTS idx_emv_qr_configs_company_id
    ON accounting.emv_qr_configs (company_id);

-- ── bank_check_sequences ───────────────────────────────────────────────────────
CREATE UNIQUE INDEX IF NOT EXISTS uq_bank_check_sequences_company_bank
    ON accounting.bank_check_sequences (company_id, bank_account_id);

-- ── printed_checks ─────────────────────────────────────────────────────────────
CREATE UNIQUE INDEX IF NOT EXISTS uq_printed_checks_company_bank_number
    ON accounting.printed_checks (company_id, bank_account_id, check_number);
CREATE INDEX IF NOT EXISTS idx_printed_checks_company_payment
    ON accounting.printed_checks (company_id, payment_id);

-- ── tax_tag_repair_runs ────────────────────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_tax_tag_repair_runs_company_ran_at
    ON accounting.tax_tag_repair_runs (company_id, ran_at DESC);

-- ── drop the strip's tenant-free re-based indexes (superseded by the shapes above) ──
DROP INDEX IF EXISTS accounting.uq_emv_qr_configs_bank_slot;
DROP INDEX IF EXISTS accounting.uq_bank_check_sequences_bank;
DROP INDEX IF EXISTS accounting.uq_printed_checks_bank_number;
DROP INDEX IF EXISTS accounting.idx_tax_tag_repair_runs_ran_at;
