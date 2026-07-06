-- Register backbone-asset as a GL producer: it emits capitalization / depreciation / disposal posts
-- via the AccountingPost contract with source_type='asset'.
ALTER TYPE posting_source_type ADD VALUE IF NOT EXISTS 'asset';
