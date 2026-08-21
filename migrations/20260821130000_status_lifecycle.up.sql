-- Migration: replace the cost-center active boolean with a status enum
-- accounting.cost_centers carried `is_active BOOLEAN NOT NULL DEFAULT TRUE`; the tree-wide
-- convention is one `status` enum field per lifecycle (see docs/refactoring-schema in the serpa
-- workspace). FALSE rows are written to 'inactive'; TRUE rows ride the new column's DEFAULT
-- 'active' (no UPDATE needed). The enum type is created unqualified so it lands beside the
-- module's other enum types (public), where the generated sqlx type_name resolves.

DO $$ BEGIN
    CREATE TYPE cost_center_status AS ENUM ('active', 'inactive');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

ALTER TABLE accounting.cost_centers ADD COLUMN status cost_center_status NOT NULL DEFAULT 'active';
UPDATE accounting.cost_centers SET status = 'inactive' WHERE NOT is_active;
ALTER TABLE accounting.cost_centers DROP COLUMN is_active;

DROP INDEX IF EXISTS accounting.idx_cost_centers_company_id_is_active;
CREATE INDEX IF NOT EXISTS idx_cost_centers_company_id_status ON accounting.cost_centers (company_id, status);
