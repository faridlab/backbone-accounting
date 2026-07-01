-- Down: drop deferred foreign keys for accounting module
ALTER TABLE ledgers DROP CONSTRAINT IF EXISTS fk_ledgers_journal_line_id;
