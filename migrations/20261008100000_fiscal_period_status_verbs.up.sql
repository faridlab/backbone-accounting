-- A fiscal period's status moves only through the close, lock and reopen verbs.
--
-- Two parts. The reopen trail: who reopened a closed period, when and why,
-- beside the closed_* and locked_* columns the close and lock already record.
-- And the transition fence: an UPDATE that moves `status` must follow the
-- period lifecycle, whoever issues it — the generic CRUD write, a write
-- service, or psql. The service refuses the same moves first with a readable
-- error; this trigger is the backstop for every writer that bypasses it.
--
--   open | adjusting -> closing        the close claims the period
--   closing -> closed                  the closing entry was posted
--   closing -> open | adjusting        the close failed and gave the claim back
--   closed -> locked                   the period is final
--   closed -> open                     a deliberate reopen
--
-- A locked period moves nowhere. Rows whose status does not change pass
-- untouched, so ordinary edits of a period's other fields are unaffected.

ALTER TABLE accounting.fiscal_periods ADD COLUMN IF NOT EXISTS reopened_at TIMESTAMPTZ;
ALTER TABLE accounting.fiscal_periods ADD COLUMN IF NOT EXISTS reopened_by UUID;
ALTER TABLE accounting.fiscal_periods ADD COLUMN IF NOT EXISTS reopen_reason TEXT;

CREATE OR REPLACE FUNCTION accounting.fiscal_periods_status_transition() RETURNS trigger AS $$
BEGIN
    IF NEW.status IS NOT DISTINCT FROM OLD.status THEN
        RETURN NEW;
    END IF;
    IF (OLD.status::text, NEW.status::text) IN (
        ('open', 'closing'),
        ('adjusting', 'closing'),
        ('closing', 'closed'),
        ('closing', 'open'),
        ('closing', 'adjusting'),
        ('closed', 'locked'),
        ('closed', 'open')
    ) THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'a fiscal period cannot move from % to %', OLD.status, NEW.status
        USING ERRCODE = 'check_violation',
              CONSTRAINT = 'fiscal_period_status_transition';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS fiscal_periods_status_transition ON accounting.fiscal_periods;
CREATE TRIGGER fiscal_periods_status_transition
    BEFORE UPDATE OF status ON accounting.fiscal_periods
    FOR EACH ROW EXECUTE FUNCTION accounting.fiscal_periods_status_transition();
