-- Register backbone-manufacturing as a GL producer: it emits WIP/FG valuation posts
-- (Work Order consume/operate/receive) via the AccountingPost contract with source_type='manufacturing'.
ALTER TYPE posting_source_type ADD VALUE IF NOT EXISTS 'manufacturing';
