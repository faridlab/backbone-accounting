-- Make the contract's `idempotency_key` a REAL dedup key (program hardening audit 2026-07-11).
--
-- Until now accounting deduped ONLY on the tuple (company_id, source_type, source_id, posting_type), and the
-- envelope's `idempotency_key` was dropped by the ACL adapters — so a producer emitting MORE THAN ONE
-- `original` post per source document had to hand-namespace `source_id` (Uuid::new_v5) or silently lose the
-- 2nd+ post. This adds an OPTIONAL `idempotency_key`:
--   • when a producer SETS it → dedup is on (company_id, idempotency_key), and the producer may reuse
--     source_id across its several posts (each disambiguated by its key);
--   • when it's ABSENT (NULL) → the legacy tuple dedup still applies, unchanged — so every existing producer
--     (whose adapter drops the key) behaves exactly as before. Backward compatible by construction.

ALTER TABLE accounting.accounting_posts ADD COLUMN IF NOT EXISTS idempotency_key text;

-- Key-based dedup — applies only to posts that carry a key.
CREATE UNIQUE INDEX IF NOT EXISTS idx_accounting_posts_company_idempotency_key
  ON accounting.accounting_posts (company_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL AND posting_status = 'posted' AND (metadata->>'deleted_at') IS NULL;

-- The legacy tuple dedup now applies ONLY to keyless posts, so a key-bearing producer can emit several
-- originals for one source_id without colliding.
DROP INDEX IF EXISTS accounting.idx_accounting_posts_company_id_source_type_source_id_posting_type;
CREATE UNIQUE INDEX IF NOT EXISTS idx_accounting_posts_company_id_source_type_source_id_posting_type
  ON accounting.accounting_posts (company_id, source_type, source_id, posting_type)
  WHERE idempotency_key IS NULL AND posting_status = 'posted' AND (metadata->>'deleted_at') IS NULL;
