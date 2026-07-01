-- Down: drop accounting.fiscal_periods table
DROP TABLE IF EXISTS accounting.fiscal_periods CASCADE;
DROP FUNCTION IF EXISTS accounting.fiscal_periods_audit_timestamp() CASCADE;
