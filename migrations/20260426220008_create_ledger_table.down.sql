-- Down: drop ledgers table
DROP TABLE IF EXISTS ledgers CASCADE;
DROP FUNCTION IF EXISTS ledgers_audit_timestamp() CASCADE;
