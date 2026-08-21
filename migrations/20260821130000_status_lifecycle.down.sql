-- Down: restore the cost-center active boolean from the status enum
-- Only 'inactive' rows are written back to FALSE; 'active' rows ride the boolean DEFAULT TRUE.

ALTER TABLE accounting.cost_centers ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE accounting.cost_centers SET is_active = FALSE WHERE status = 'inactive';
ALTER TABLE accounting.cost_centers DROP COLUMN status;

DROP INDEX IF EXISTS idx_cost_centers_company_id_status;
CREATE INDEX IF NOT EXISTS idx_cost_centers_company_id_is_active ON accounting.cost_centers (company_id, is_active);

DROP TYPE IF EXISTS cost_center_status;
