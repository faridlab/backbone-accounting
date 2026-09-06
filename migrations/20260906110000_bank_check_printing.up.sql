-- Check printing: per-bank-journal check-number sequences plus the printed-check
-- registry (the Odoo account_check_printing shape, ported onto this estate's
-- bank accounts). A "bank journal" here is a company's bank account: the caller
-- (payment side) already holds the bank_account_id, and accounting stores it as
-- a logical reference — banking.bank_accounts owns the row, no cross-schema
-- foreign key by contract.
--
-- Two tables:
--   bank_check_sequences — one row per (company, bank account); numbering mode
--     ('auto' allocates from next_number inside the verb transaction, 'manual'
--     validates officer-supplied numbers against the registry) and the
--     allocation cursor. next_number is the NEXT number to hand out.
--   printed_checks — the registry: which payment received which check number.
--     The unique (company, bank account, check_number) constraint IS the
--     cross-payment uniqueness guard, enforced in SQL.
--
-- The allocation verb caps numbers at 2147483647 (MAX_INT32): sequence columns
-- upstream are 32-bit and printed numbers must stay portable.
--
-- Hand-authored migration (no generator header): both tables are verb-backed
-- (CheckPrintingService); they deliberately mount no CRUD surface.

CREATE SCHEMA IF NOT EXISTS accounting;

CREATE TABLE IF NOT EXISTS accounting.bank_check_sequences (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL,
    bank_account_id UUID NOT NULL,
    -- 'auto': the record verb allocates from next_number. 'manual': the officer
    -- supplies the number (prenumbered check stock) and only the uniqueness and
    -- registry guards apply.
    numbering_mode TEXT NOT NULL DEFAULT 'auto',
    next_number BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    PRIMARY KEY (id),
    CONSTRAINT bank_check_sequences_mode_check
        CHECK (numbering_mode IN ('auto', 'manual')),
    CONSTRAINT bank_check_sequences_next_number_positive
        CHECK (next_number > 0)
    -- The MAX_INT32 cap is enforced by the allocation verb (typed refusal, whole
    -- transaction rolls back so no number is consumed), not by a column CHECK:
    -- a SQL-level violation would surface as an opaque 23514 instead.
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_bank_check_sequences_company_bank
    ON accounting.bank_check_sequences (company_id, bank_account_id);

CREATE TABLE IF NOT EXISTS accounting.printed_checks (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL,
    bank_account_id UUID NOT NULL,
    -- The payment the check settles. Logical reference only
    -- (payment.payment_entries owns the row); no cross-schema foreign key.
    payment_id UUID NOT NULL,
    payment_number TEXT,
    check_number TEXT NOT NULL,
    amount NUMERIC(18, 2) NOT NULL,
    payee_name TEXT,
    -- 'printed' → 'voided' (a voided number stays consumed: reversing the
    -- payment does not free the number for reuse).
    status TEXT NOT NULL DEFAULT 'printed',
    printed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    printed_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    PRIMARY KEY (id),
    CONSTRAINT printed_checks_status_check
        CHECK (status IN ('printed', 'voided')),
    CONSTRAINT printed_checks_amount_positive CHECK (amount > 0)
);

-- The cross-payment uniqueness guard: one check number per bank journal, ever.
CREATE UNIQUE INDEX IF NOT EXISTS uq_printed_checks_company_bank_number
    ON accounting.printed_checks (company_id, bank_account_id, check_number);
CREATE INDEX IF NOT EXISTS idx_printed_checks_company_payment
    ON accounting.printed_checks (company_id, payment_id);

-- Company fence (ADR-0014: strict) — same posture as every company-scoped table
-- in this module: a session sees only rows whose company_id equals the
-- request-scoped company (set_config('app.company_id', <uuid>, true)); an unset
-- var sees zero rows (fail-closed). Requires the app to connect as a
-- non-superuser role; migrations/seeders run as the owner and bypass.
ALTER TABLE accounting.bank_check_sequences ENABLE ROW LEVEL SECURITY;
ALTER TABLE accounting.bank_check_sequences FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS bank_check_sequences_company_isolation ON accounting.bank_check_sequences;
CREATE POLICY bank_check_sequences_company_isolation ON accounting.bank_check_sequences
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

ALTER TABLE accounting.printed_checks ENABLE ROW LEVEL SECURITY;
ALTER TABLE accounting.printed_checks FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS printed_checks_company_isolation ON accounting.printed_checks;
CREATE POLICY printed_checks_company_isolation ON accounting.printed_checks
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
