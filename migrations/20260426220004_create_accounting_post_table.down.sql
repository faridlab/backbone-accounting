-- Down: drop accounting_posts table
DROP TABLE IF EXISTS accounting_posts CASCADE;
DROP FUNCTION IF EXISTS accounting_posts_audit_timestamp() CASCADE;
