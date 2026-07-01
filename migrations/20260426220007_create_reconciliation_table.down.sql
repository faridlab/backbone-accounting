-- Down: drop accounting.reconciliations table
DROP TABLE IF EXISTS accounting.reconciliations CASCADE;
DROP FUNCTION IF EXISTS accounting.reconciliations_audit_timestamp() CASCADE;
