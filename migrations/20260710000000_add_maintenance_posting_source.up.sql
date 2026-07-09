-- Register backbone-maintenance as a GL producer: it emits the maintenance-cost journal
-- (Dr Maintenance Expense · Cr Inventory Parts · Cr Labor Payable) with source_type='maintenance'. 9th producer.
ALTER TYPE posting_source_type ADD VALUE IF NOT EXISTS 'maintenance';
