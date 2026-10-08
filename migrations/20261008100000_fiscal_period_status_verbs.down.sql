DROP TRIGGER IF EXISTS fiscal_periods_status_transition ON accounting.fiscal_periods;
DROP FUNCTION IF EXISTS accounting.fiscal_periods_status_transition();

ALTER TABLE accounting.fiscal_periods DROP COLUMN IF EXISTS reopen_reason;
ALTER TABLE accounting.fiscal_periods DROP COLUMN IF EXISTS reopened_by;
ALTER TABLE accounting.fiscal_periods DROP COLUMN IF EXISTS reopened_at;
