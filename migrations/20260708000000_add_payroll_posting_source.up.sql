-- Register backbone-payroll as a GL producer: it emits the salary journal
-- (Dr Salary Expense · Cr Salary Payable · Cr BPJS Payable · Cr PPh 21 Payable)
-- via the AccountingPost contract with source_type='payroll'. The 8th GL producer.
ALTER TYPE posting_source_type ADD VALUE IF NOT EXISTS 'payroll';
