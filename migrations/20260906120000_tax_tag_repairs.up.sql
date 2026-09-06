-- Tax-tag legal-change repair: the audit ledger for recompute runs.
--
-- The repair verb rewrites the tax-report tag set (journal_lines.tags, the
-- jsonb array of tax-module tag identifiers) on POSTED journal lines inside a
-- date window, after a legal change moves the correct assignment. The lines
-- themselves are never touched — only their tag assignment — and every run
-- (dry run or apply) stamps one row here recording the window, the exact rule
-- set applied, the counts, the officer, and the mandatory justification. This
-- table is the audit trail: the verb is the only writer, and it refuses to run
-- without a reason string.
--
-- Guard posture: the verb refuses when the window overlaps a LOCKED fiscal
-- period (no override) and demands an explicit override flag for CLOSED ones —
-- the local rendering of the tax-lock-date posture, fail-closed.
--
-- Hand-authored migration (no generator header): the table is verb-backed
-- (TaxTagRepairService); it deliberately mounts no CRUD surface.

CREATE SCHEMA IF NOT EXISTS accounting;

CREATE TABLE IF NOT EXISTS accounting.tax_tag_repair_runs (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL,
    date_from DATE NOT NULL,
    date_to DATE NOT NULL,
    -- The assignment rules exactly as applied (selectors + tag sets). Stored so
    -- a later run is diffable against what an earlier one did.
    rules JSONB NOT NULL,
    lines_examined BIGINT NOT NULL,
    lines_retagged BIGINT NOT NULL,
    dry_run BOOLEAN NOT NULL DEFAULT false,
    -- The closed periods the run crossed with the override armed (audit copy).
    overridden_closed_periods JSONB NOT NULL DEFAULT '[]'::jsonb,
    -- The officer who ran the repair (logical reference to sapiens.users).
    actor UUID,
    -- Mandatory legal-change justification; the verb refuses empty reasons.
    reason TEXT NOT NULL,
    ran_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    PRIMARY KEY (id),
    CONSTRAINT tax_tag_repair_runs_window_order CHECK (date_from <= date_to)
);

CREATE INDEX IF NOT EXISTS idx_tax_tag_repair_runs_company_ran_at
    ON accounting.tax_tag_repair_runs (company_id, ran_at DESC);

-- Company fence (ADR-0014: strict) — same posture as every company-scoped table
-- in this module: a session sees only rows whose company_id equals the
-- request-scoped company (set_config('app.company_id', <uuid>, true)); an unset
-- var sees zero rows (fail-closed). Requires the app to connect as a
-- non-superuser role; migrations/seeders run as the owner and bypass.
ALTER TABLE accounting.tax_tag_repair_runs ENABLE ROW LEVEL SECURITY;
ALTER TABLE accounting.tax_tag_repair_runs FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tax_tag_repair_runs_company_isolation ON accounting.tax_tag_repair_runs;
CREATE POLICY tax_tag_repair_runs_company_isolation ON accounting.tax_tag_repair_runs
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
