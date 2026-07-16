-- Down: remove the company RLS fence for accounting module

-- Reverse the company RLS fence for accounting.accounts
DROP POLICY IF EXISTS accounts_company_isolation ON accounting.accounts;
ALTER TABLE accounting.accounts NO FORCE ROW LEVEL SECURITY;
ALTER TABLE accounting.accounts DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for accounting.accounting_posts
DROP POLICY IF EXISTS accounting_posts_company_isolation ON accounting.accounting_posts;
ALTER TABLE accounting.accounting_posts NO FORCE ROW LEVEL SECURITY;
ALTER TABLE accounting.accounting_posts DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for accounting.cost_centers
DROP POLICY IF EXISTS cost_centers_company_isolation ON accounting.cost_centers;
ALTER TABLE accounting.cost_centers NO FORCE ROW LEVEL SECURITY;
ALTER TABLE accounting.cost_centers DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for accounting.financial_statements
DROP POLICY IF EXISTS financial_statements_company_isolation ON accounting.financial_statements;
ALTER TABLE accounting.financial_statements NO FORCE ROW LEVEL SECURITY;
ALTER TABLE accounting.financial_statements DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for accounting.fiscal_periods
DROP POLICY IF EXISTS fiscal_periods_company_isolation ON accounting.fiscal_periods;
ALTER TABLE accounting.fiscal_periods NO FORCE ROW LEVEL SECURITY;
ALTER TABLE accounting.fiscal_periods DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for accounting.journals
DROP POLICY IF EXISTS journals_company_isolation ON accounting.journals;
ALTER TABLE accounting.journals NO FORCE ROW LEVEL SECURITY;
ALTER TABLE accounting.journals DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for accounting.journal_lines
DROP POLICY IF EXISTS journal_lines_company_isolation ON accounting.journal_lines;
ALTER TABLE accounting.journal_lines NO FORCE ROW LEVEL SECURITY;
ALTER TABLE accounting.journal_lines DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for accounting.ledgers
DROP POLICY IF EXISTS ledgers_company_isolation ON accounting.ledgers;
ALTER TABLE accounting.ledgers NO FORCE ROW LEVEL SECURITY;
ALTER TABLE accounting.ledgers DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for accounting.reconciliations
DROP POLICY IF EXISTS reconciliations_company_isolation ON accounting.reconciliations;
ALTER TABLE accounting.reconciliations NO FORCE ROW LEVEL SECURITY;
ALTER TABLE accounting.reconciliations DISABLE ROW LEVEL SECURITY;

-- Reverse the company RLS fence for accounting.reconciliation_items
DROP POLICY IF EXISTS reconciliation_items_company_isolation ON accounting.reconciliation_items;
ALTER TABLE accounting.reconciliation_items NO FORCE ROW LEVEL SECURITY;
ALTER TABLE accounting.reconciliation_items DISABLE ROW LEVEL SECURITY;

