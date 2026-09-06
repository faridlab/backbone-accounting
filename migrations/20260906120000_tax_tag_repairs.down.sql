-- Down: drop the tax-tag repair audit ledger.

DROP POLICY IF EXISTS tax_tag_repair_runs_company_isolation ON accounting.tax_tag_repair_runs;
DROP TABLE IF EXISTS accounting.tax_tag_repair_runs;
