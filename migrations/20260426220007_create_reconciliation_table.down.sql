-- Down: drop reconciliations table
DROP TABLE IF EXISTS reconciliations CASCADE;
DROP FUNCTION IF EXISTS reconciliations_audit_timestamp() CASCADE;
