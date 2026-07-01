-- Down: drop accounting.financial_statements table
DROP TABLE IF EXISTS accounting.financial_statements CASCADE;
DROP FUNCTION IF EXISTS accounting.financial_statements_audit_timestamp() CASCADE;
