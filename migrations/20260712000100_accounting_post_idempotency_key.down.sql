-- Revert: restore the tuple index to cover all posted rows and drop the key index + column.
DROP INDEX IF EXISTS accounting.idx_accounting_posts_company_id_source_type_source_id_posting_type;
CREATE UNIQUE INDEX IF NOT EXISTS idx_accounting_posts_company_id_source_type_source_id_posting_type
  ON accounting.accounting_posts (company_id, source_type, source_id, posting_type)
  WHERE posting_status = 'posted' AND (metadata->>'deleted_at') IS NULL;
DROP INDEX IF EXISTS accounting.idx_accounting_posts_company_idempotency_key;
ALTER TABLE accounting.accounting_posts DROP COLUMN IF EXISTS idempotency_key;
