-- Down: drop fiscal_periods table
DROP TABLE IF EXISTS fiscal_periods CASCADE;
DROP FUNCTION IF EXISTS fiscal_periods_audit_timestamp() CASCADE;
