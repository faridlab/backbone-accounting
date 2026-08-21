-- PostgreSQL cannot remove an enum value. Rolling back a database that never saw a
-- gateway fee post means leaving the unused 'gateway_fee' value in place; otherwise
-- rebuild the type and re-cast the column.
SELECT 1;
