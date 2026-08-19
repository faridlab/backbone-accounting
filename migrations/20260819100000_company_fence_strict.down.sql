-- Revert the ADR-0014 strict fence re-statement for accounting module.
-- The fence predates this migration (ADR-0008-era), so the honest reverse is to
-- re-state the same live policy, not to disarm the tables: a down that disabled RLS
-- would leave company data unfenced — a posture this module never had.

-- Re-state the pre-existing fence for accounting.accounting_posts (identical policy; see header).
DROP POLICY IF EXISTS accounting_posts_company_isolation ON accounting.accounting_posts;
CREATE POLICY accounting_posts_company_isolation ON accounting.accounting_posts
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for accounting.accounts (identical policy; see header).
DROP POLICY IF EXISTS accounts_company_isolation ON accounting.accounts;
CREATE POLICY accounts_company_isolation ON accounting.accounts
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for accounting.cost_centers (identical policy; see header).
DROP POLICY IF EXISTS cost_centers_company_isolation ON accounting.cost_centers;
CREATE POLICY cost_centers_company_isolation ON accounting.cost_centers
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for accounting.financial_statements (identical policy; see header).
DROP POLICY IF EXISTS financial_statements_company_isolation ON accounting.financial_statements;
CREATE POLICY financial_statements_company_isolation ON accounting.financial_statements
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for accounting.fiscal_periods (identical policy; see header).
DROP POLICY IF EXISTS fiscal_periods_company_isolation ON accounting.fiscal_periods;
CREATE POLICY fiscal_periods_company_isolation ON accounting.fiscal_periods
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for accounting.journal_lines (identical policy; see header).
DROP POLICY IF EXISTS journal_lines_company_isolation ON accounting.journal_lines;
CREATE POLICY journal_lines_company_isolation ON accounting.journal_lines
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for accounting.journals (identical policy; see header).
DROP POLICY IF EXISTS journals_company_isolation ON accounting.journals;
CREATE POLICY journals_company_isolation ON accounting.journals
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for accounting.ledgers (identical policy; see header).
DROP POLICY IF EXISTS ledgers_company_isolation ON accounting.ledgers;
CREATE POLICY ledgers_company_isolation ON accounting.ledgers
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for accounting.reconciliation_items (identical policy; see header).
DROP POLICY IF EXISTS reconciliation_items_company_isolation ON accounting.reconciliation_items;
CREATE POLICY reconciliation_items_company_isolation ON accounting.reconciliation_items
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

-- Re-state the pre-existing fence for accounting.reconciliations (identical policy; see header).
DROP POLICY IF EXISTS reconciliations_company_isolation ON accounting.reconciliations;
CREATE POLICY reconciliations_company_isolation ON accounting.reconciliations
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);

