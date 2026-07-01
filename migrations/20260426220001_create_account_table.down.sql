-- Down: drop accounting.accounts table
DROP TABLE IF EXISTS accounting.accounts CASCADE;
DROP FUNCTION IF EXISTS accounting.accounts_audit_timestamp() CASCADE;
