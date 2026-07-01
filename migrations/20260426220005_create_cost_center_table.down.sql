-- Down: drop accounting.cost_centers table
DROP TABLE IF EXISTS accounting.cost_centers CASCADE;
DROP FUNCTION IF EXISTS accounting.cost_centers_audit_timestamp() CASCADE;
