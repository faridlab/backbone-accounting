-- Hand-authored (user-owned). Not regenerated.
--
-- Strip every company-fence artifact from the accounting tables (ADR-0029): the module is
-- tenant-agnostic; org scoping is installed by the COMPOSING service's tenancy decorator,
-- never by the module. Dropped here, per table: the company-leading indexes, the
-- <table>_company_isolation RLS policy, and the company_id column itself.
--
-- Ordering guard (the decorator must run FIRST on any database with data): the module
-- never moves tenancy data. A table is safe to strip when EITHER
--   a) it carries org_unit_id with no NULLs — the decorator backfilled it from company_id —
--      or b) it is empty (a fresh database: the earlier chain files created it empty).
-- Otherwise the strip RAISEs, naming the decorator step, rather than dropping a column
-- that still holds the only tenancy key. The file is re-runnable (every drop is IF EXISTS
-- and the tracker has no checksums), so a failed run retries cleanly after the decorator
-- lands.
--
-- RLS enable/force flags are deliberately NOT touched: the decorator owns those now.
-- The idempotency_key column on accounting_posts is a domain dedup key, not tenancy —
-- it stays.
--
-- A few company-leading indexes are RE-Based tenant-free rather than merely dropped:
-- where every remaining key element is a globally unique UUID, the org-free form is
-- exactly equivalent to the per-unit form and is a domain constraint the module's own
-- single-statement upserts conflict against (the posting idempotency uniques, the
-- QR-config slot unique, the check sequence unique, the check-number unique) or a
-- plain read path (ran_at).

DO $$
DECLARE
    t text;
    has_org boolean;
    org_nulls bigint;
    total bigint;
    offenders text := '';
BEGIN
    FOREACH t IN ARRAY ARRAY[
        'accounts', 'fiscal_periods', 'journals', 'journal_lines', 'accounting_posts',
        'cost_centers', 'financial_statements', 'reconciliations', 'reconciliation_items',
        'ledgers', 'full_reconciles', 'partial_reconciles',
        'emv_qr_configs', 'bank_check_sequences', 'printed_checks', 'tax_tag_repair_runs'
    ]
    LOOP
        IF to_regclass(format('accounting.%I', t)) IS NULL THEN
            CONTINUE; -- chain not fully applied on this database; nothing to strip
        END IF;

        SELECT EXISTS (
                   SELECT 1 FROM information_schema.columns
                   WHERE table_schema = 'accounting' AND table_name = t AND column_name = 'org_unit_id'
               )
        INTO has_org;

        EXECUTE format('SELECT count(*) FROM accounting.%I', t) INTO total;

        IF has_org THEN
            EXECUTE format(
                'SELECT count(*) FROM accounting.%I WHERE org_unit_id IS NULL', t)
            INTO org_nulls;
        ELSE
            org_nulls := total; -- no org column: every row's only tenancy key is company_id
        END IF;

        IF has_org AND org_nulls = 0 THEN
            CONTINUE; -- decorator backfilled: safe
        END IF;
        IF total = 0 THEN
            CONTINUE; -- empty table (fresh database): safe
        END IF;
        offenders := offenders || format(' accounting.%s (%s rows, %s rows not covered by org_unit_id);', t, total, org_nulls);
    END LOOP;

    IF offenders <> '' THEN
        RAISE EXCEPTION 'refusing to strip company_id — these tables are not yet covered by the tenancy decorator:%. Apply the composing service''s tenancy decorator (it backfills org_unit_id from company_id) and re-run; it is the only step that moves tenancy data.', offenders;
    END IF;
END $$;

-- ── accounts ───────────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.idx_accounts_company_id_account_number;
DROP INDEX IF EXISTS accounting.idx_accounts_company_id_account_code;
DROP INDEX IF EXISTS accounting.idx_accounts_company_id_chart_code;
DROP INDEX IF EXISTS accounting.idx_accounts_company_id_account_type_status;
DROP INDEX IF EXISTS accounting.idx_accounts_company_id_parent_id_sort_order;
DROP INDEX IF EXISTS accounting.idx_accounts_company_id_account_subtype;
DROP INDEX IF EXISTS accounting.idx_accounts_company_id_is_reconcilable;
DROP POLICY IF EXISTS accounts_company_isolation ON accounting.accounts;
ALTER TABLE accounting.accounts DROP COLUMN IF EXISTS company_id;

-- ── fiscal_periods ─────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.idx_fiscal_periods_company_id_period_code;
DROP INDEX IF EXISTS accounting.idx_fiscal_periods_company_id_fiscal_year_fiscal_month;
DROP INDEX IF EXISTS accounting.idx_fiscal_periods_company_id_fiscal_year;
DROP INDEX IF EXISTS accounting.idx_fiscal_periods_company_id_status;
DROP INDEX IF EXISTS accounting.idx_fiscal_periods_company_id_start_date_end_date;
DROP INDEX IF EXISTS accounting.idx_fiscal_periods_company_id_is_current;
DROP POLICY IF EXISTS fiscal_periods_company_isolation ON accounting.fiscal_periods;
ALTER TABLE accounting.fiscal_periods DROP COLUMN IF EXISTS company_id;

-- ── journals ───────────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.idx_journals_company_id_journal_number;
DROP INDEX IF EXISTS accounting.idx_journals_company_id_transaction_date;
DROP INDEX IF EXISTS accounting.idx_journals_company_id_journal_type_status;
DROP INDEX IF EXISTS accounting.idx_journals_company_id_fiscal_year_fiscal_month;
DROP INDEX IF EXISTS accounting.idx_journals_company_id_status_transaction_date;
DROP INDEX IF EXISTS accounting.idx_journals_company_id_is_reversed;
DROP INDEX IF EXISTS accounting.idx_journals_company_id_branch_id_transaction_date;
DROP POLICY IF EXISTS journals_company_isolation ON accounting.journals;
ALTER TABLE accounting.journals DROP COLUMN IF EXISTS company_id;

-- ── journal_lines ──────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.idx_journal_lines_company_id_account_id;
DROP INDEX IF EXISTS accounting.idx_journal_lines_company_id_is_reconciled;
DROP INDEX IF EXISTS accounting.idx_journal_lines_company_id_party_type_party_id_account_id;
DROP INDEX IF EXISTS accounting.idx_journal_lines_company_id_full_reconcile_id;
DROP POLICY IF EXISTS journal_lines_company_isolation ON accounting.journal_lines;
ALTER TABLE accounting.journal_lines DROP COLUMN IF EXISTS company_id;

-- ── accounting_posts ───────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.idx_accounting_posts_company_id_source_type_source_id_posting_type;
DROP INDEX IF EXISTS accounting.idx_accounting_posts_company_idempotency_key;
DROP INDEX IF EXISTS accounting.idx_accounting_posts_company_id_source_type_posting_status;
DROP INDEX IF EXISTS accounting.idx_accounting_posts_company_id_branch_id_posting_status;
DROP INDEX IF EXISTS accounting.idx_accounting_posts_company_id_posted_at;
DROP POLICY IF EXISTS accounting_posts_company_isolation ON accounting.accounting_posts;
ALTER TABLE accounting.accounting_posts DROP COLUMN IF EXISTS company_id;

-- Tenant-free re-base: the posting idempotency uniques — the dedup grain and the
-- race arbiter the post INSERT conflicts against (typed concurrent-posting loss).
-- Both remaining keys are globally unique in themselves — a producer-minted
-- idempotency_key, and a UUID source_id tuple — so the org-free form is exactly
-- the per-unit grain, and it must exist module-side or an undecorated deployment
-- silently double-posts under concurrency.
CREATE UNIQUE INDEX IF NOT EXISTS uq_accounting_posts_idempotency_key
    ON accounting.accounting_posts (idempotency_key)
    WHERE idempotency_key IS NOT NULL AND posting_status = 'posted'
      AND (metadata->>'deleted_at') IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uq_accounting_posts_source_tuple_keyless
    ON accounting.accounting_posts (source_type, source_id, posting_type)
    WHERE idempotency_key IS NULL AND posting_status = 'posted'
      AND (metadata->>'deleted_at') IS NULL;

-- ── cost_centers ───────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.idx_cost_centers_company_id_code;
DROP INDEX IF EXISTS accounting.idx_cost_centers_company_id_parent_id_sort_order;
DROP INDEX IF EXISTS accounting.idx_cost_centers_company_id_status;
DROP POLICY IF EXISTS cost_centers_company_isolation ON accounting.cost_centers;
ALTER TABLE accounting.cost_centers DROP COLUMN IF EXISTS company_id;

-- ── financial_statements ───────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.idx_financial_statements_company_id_statement_number;
DROP INDEX IF EXISTS accounting.idx_financial_statements_company_id_statement_type_fiscal_year;
DROP INDEX IF EXISTS accounting.idx_financial_statements_company_id_fiscal_period_id;
DROP INDEX IF EXISTS accounting.idx_financial_statements_company_id_status;
DROP INDEX IF EXISTS accounting.idx_financial_statements_company_id_as_of_date;
DROP POLICY IF EXISTS financial_statements_company_isolation ON accounting.financial_statements;
ALTER TABLE accounting.financial_statements DROP COLUMN IF EXISTS company_id;

-- ── reconciliations ────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.idx_reconciliations_company_id_reconciliation_number;
DROP INDEX IF EXISTS accounting.idx_reconciliations_company_id_account_id_statement_date;
DROP INDEX IF EXISTS accounting.idx_reconciliations_company_id_account_id_period_start;
DROP INDEX IF EXISTS accounting.idx_reconciliations_company_id_status;
DROP INDEX IF EXISTS accounting.idx_reconciliations_company_id_is_balanced;
DROP POLICY IF EXISTS reconciliations_company_isolation ON accounting.reconciliations;
ALTER TABLE accounting.reconciliations DROP COLUMN IF EXISTS company_id;

-- ── reconciliation_items ───────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.idx_reconciliation_items_company_id_is_outstanding;
DROP POLICY IF EXISTS reconciliation_items_company_isolation ON accounting.reconciliation_items;
ALTER TABLE accounting.reconciliation_items DROP COLUMN IF EXISTS company_id;

-- ── ledgers ────────────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.idx_ledgers_company_id_account_id_transaction_date;
DROP INDEX IF EXISTS accounting.idx_ledgers_company_id_account_id_sequence_number;
DROP INDEX IF EXISTS accounting.idx_ledgers_company_id_fiscal_year_fiscal_month;
DROP INDEX IF EXISTS accounting.idx_ledgers_company_id_transaction_date;
DROP INDEX IF EXISTS accounting.idx_ledgers_company_id_account_id_is_reconciled;
DROP INDEX IF EXISTS accounting.idx_ledgers_company_id_is_opening_balance;
DROP INDEX IF EXISTS accounting.idx_ledgers_company_id_is_closing_entry;
DROP INDEX IF EXISTS accounting.idx_ledgers_company_id_branch_id_account_id_transaction_date;
DROP INDEX IF EXISTS accounting.idx_ledgers_company_id_party_type_party_id_account_id_transaction_date;
DROP POLICY IF EXISTS ledgers_company_isolation ON accounting.ledgers;
ALTER TABLE accounting.ledgers DROP COLUMN IF EXISTS company_id;

-- ── full_reconciles ────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.idx_full_reconciles_company_id_reconciled_at;
DROP POLICY IF EXISTS full_reconciles_company_isolation ON accounting.full_reconciles;
ALTER TABLE accounting.full_reconciles DROP COLUMN IF EXISTS company_id;

-- ── partial_reconciles ─────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.idx_partial_reconciles_company_id_debit_move_id;
DROP INDEX IF EXISTS accounting.idx_partial_reconciles_company_id_credit_move_id;
DROP INDEX IF EXISTS accounting.idx_partial_reconciles_company_id_source_type_source_id;
DROP INDEX IF EXISTS accounting.idx_partial_reconciles_company_id_max_date;
DROP POLICY IF EXISTS partial_reconciles_company_isolation ON accounting.partial_reconciles;
ALTER TABLE accounting.partial_reconciles DROP COLUMN IF EXISTS company_id;

-- ── emv_qr_configs ─────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.uq_emv_qr_configs_company_bank_slot;
DROP INDEX IF EXISTS accounting.idx_emv_qr_configs_company_id;
DROP POLICY IF EXISTS emv_qr_configs_company_isolation ON accounting.emv_qr_configs;
ALTER TABLE accounting.emv_qr_configs DROP COLUMN IF EXISTS company_id;
-- Tenant-free re-base of the dropped slot unique. bank_account_id is a globally
-- unique UUID (banking.BankAccount), so the org-free form is exactly equivalent
-- to the per-unit one — a DOMAIN constraint, not tenancy posture: the module's
-- own single-statement upsert conflicts against it, so it must exist here,
-- independent of any decorator.
CREATE UNIQUE INDEX IF NOT EXISTS uq_emv_qr_configs_bank_slot
    ON accounting.emv_qr_configs (
        COALESCE(bank_account_id, '00000000-0000-0000-0000-000000000000'::uuid)
    );

-- ── bank_check_sequences ───────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.uq_bank_check_sequences_company_bank;
DROP POLICY IF EXISTS bank_check_sequences_company_isolation ON accounting.bank_check_sequences;
ALTER TABLE accounting.bank_check_sequences DROP COLUMN IF EXISTS company_id;
-- Tenant-free re-base, same reasoning as the slot unique above: one cursor per
-- bank journal, keyed on the globally unique bank account.
CREATE UNIQUE INDEX IF NOT EXISTS uq_bank_check_sequences_bank
    ON accounting.bank_check_sequences (bank_account_id);

-- ── printed_checks ─────────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.uq_printed_checks_company_bank_number;
DROP INDEX IF EXISTS accounting.idx_printed_checks_company_payment;
DROP POLICY IF EXISTS printed_checks_company_isolation ON accounting.printed_checks;
ALTER TABLE accounting.printed_checks DROP COLUMN IF EXISTS company_id;
-- Tenant-free re-base: the cross-payment number-uniqueness guard a manual-number
-- collision maps to `duplicate_check_number` from. Global (bank_account_id,
-- check_number) equals the per-unit form because a bank account belongs to
-- exactly one unit.
CREATE UNIQUE INDEX IF NOT EXISTS uq_printed_checks_bank_number
    ON accounting.printed_checks (bank_account_id, check_number);

-- ── tax_tag_repair_runs ────────────────────────────────────────────────────────
DROP INDEX IF EXISTS accounting.idx_tax_tag_repair_runs_company_ran_at;
DROP POLICY IF EXISTS tax_tag_repair_runs_company_isolation ON accounting.tax_tag_repair_runs;
ALTER TABLE accounting.tax_tag_repair_runs DROP COLUMN IF EXISTS company_id;
-- Domain read path (the audit-trail listing orders by ran_at), re-based tenant-free.
CREATE INDEX IF NOT EXISTS idx_tax_tag_repair_runs_ran_at
    ON accounting.tax_tag_repair_runs (ran_at DESC);
