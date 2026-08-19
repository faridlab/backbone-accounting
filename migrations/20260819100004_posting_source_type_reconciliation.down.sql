-- PostgreSQL cannot remove an enum value. Rolling back a database that never saw a
-- reconciliation-generated post means leaving the unused 'reconciliation' value in
-- place; otherwise rebuild the type and re-cast the column.
SELECT 1;
