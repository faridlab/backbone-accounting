-- Migration: add full_reconcile_id to journal_lines
-- The reconciliation-graph anchor on the line itself: set when the line's residual reaches
-- zero inside a full-reconcile group (fully-reconciled groups read this FK; partial-only
-- chains resolve through the matching-group read). journal_lines is already fenced strict
-- (20260819100000) — the new column changes nothing about the fence posture.

ALTER TABLE accounting.journal_lines ADD COLUMN IF NOT EXISTS full_reconcile_id UUID;

ALTER TABLE accounting.journal_lines DROP CONSTRAINT IF EXISTS fk_journal_lines_full_reconcile_id;
ALTER TABLE accounting.journal_lines ADD CONSTRAINT fk_journal_lines_full_reconcile_id
    FOREIGN KEY (full_reconcile_id) REFERENCES accounting.full_reconciles (id);

CREATE INDEX IF NOT EXISTS idx_journal_lines_company_id_full_reconcile_id
    ON accounting.journal_lines (company_id, full_reconcile_id)
    WHERE full_reconcile_id IS NOT NULL;
