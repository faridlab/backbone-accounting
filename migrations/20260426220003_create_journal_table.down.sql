-- Down: drop journals table
DROP TABLE IF EXISTS journals CASCADE;
DROP FUNCTION IF EXISTS journals_audit_timestamp() CASCADE;
