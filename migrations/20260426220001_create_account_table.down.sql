-- Down: drop accounts table
DROP TABLE IF EXISTS accounts CASCADE;
DROP FUNCTION IF EXISTS accounts_audit_timestamp() CASCADE;
