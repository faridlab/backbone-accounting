-- Extend posting_source_type with 'gateway_fee': payment-gateway settlement posts a
-- fee companion journal (Dr fee expense / Cr bank) when a provider-notified charge
-- settles, and carries this source kind so gateway-booked fees stay distinguishable
-- from manual posts in the ledger.
ALTER TYPE public.posting_source_type ADD VALUE IF NOT EXISTS 'gateway_fee' BEFORE 'manual';
