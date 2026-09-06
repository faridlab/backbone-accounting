-- Down: drop the check printing sequence + registry surface.

DROP POLICY IF EXISTS printed_checks_company_isolation ON accounting.printed_checks;
DROP TABLE IF EXISTS accounting.printed_checks;
DROP POLICY IF EXISTS bank_check_sequences_company_isolation ON accounting.bank_check_sequences;
DROP TABLE IF EXISTS accounting.bank_check_sequences;
