-- Down: drop accounting.ledgers table
DROP TABLE IF EXISTS accounting.ledgers CASCADE;
DROP FUNCTION IF EXISTS accounting.ledgers_audit_timestamp() CASCADE;
