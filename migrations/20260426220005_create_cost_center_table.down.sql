-- Down: drop cost_centers table
DROP TABLE IF EXISTS cost_centers CASCADE;
DROP FUNCTION IF EXISTS cost_centers_audit_timestamp() CASCADE;
