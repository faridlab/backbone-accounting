-- Extend posting_source_type with 'reconciliation': the reconciliation graph posts
-- its generated moves (exchange difference now, CABA later) under this source kind,
-- keyed by the partial id so the unlink path can find and reverse them.
ALTER TYPE public.posting_source_type ADD VALUE IF NOT EXISTS 'reconciliation' BEFORE 'manual';
