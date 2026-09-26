-- PostgreSQL cannot remove an enum value. Rolling back a database that never
-- saw a final settlement post means leaving the unused 'final_settlement'
-- value in place; otherwise rebuild the type and re-cast the column.
SELECT 1;
