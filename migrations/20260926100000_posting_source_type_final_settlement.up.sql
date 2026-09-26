-- Extend posting_source_type with 'final_settlement': the offboarding final
-- settlement posts the leaver's last pay journal (Dr severance + leave
-- encashment expense / Cr employee payable) when the settlement is confirmed,
-- and carries this source kind so exit payouts stay distinguishable from
-- payroll runs and manual posts in the ledger.
ALTER TYPE public.posting_source_type ADD VALUE IF NOT EXISTS 'final_settlement' BEFORE 'manual';
