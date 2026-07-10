-- Register backbone-equity as a GL producer: it emits the share-issue journal (Dr Bank · Cr Share Capital ·
-- Cr Share Premium), the buyback journal (Dr Share Capital · Cr Bank), and the dividend journals
-- (declare: Dr Retained Earnings · Cr Dividend Payable; pay: Dr Dividend Payable · Cr Bank) with
-- source_type='equity'. The 10th producer.
ALTER TYPE posting_source_type ADD VALUE IF NOT EXISTS 'equity';
