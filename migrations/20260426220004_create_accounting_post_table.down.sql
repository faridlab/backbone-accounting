-- Down: drop accounting.accounting_posts table
DROP TABLE IF EXISTS accounting.accounting_posts CASCADE;
DROP FUNCTION IF EXISTS accounting.accounting_posts_audit_timestamp() CASCADE;
