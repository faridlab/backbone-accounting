-- Down: drop the full-reconcile anchor column from journal_lines
DROP INDEX IF EXISTS accounting.idx_journal_lines_company_id_full_reconcile_id;
ALTER TABLE accounting.journal_lines DROP CONSTRAINT IF EXISTS fk_journal_lines_full_reconcile_id;
ALTER TABLE accounting.journal_lines DROP COLUMN IF EXISTS full_reconcile_id;
