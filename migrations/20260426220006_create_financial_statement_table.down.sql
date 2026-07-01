-- Down: drop financial_statements table
DROP TABLE IF EXISTS financial_statements CASCADE;
DROP FUNCTION IF EXISTS financial_statements_audit_timestamp() CASCADE;
