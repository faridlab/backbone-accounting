-- Down: drop accounting.journals table
DROP TABLE IF EXISTS accounting.journals CASCADE;
DROP FUNCTION IF EXISTS accounting.journals_audit_timestamp() CASCADE;
