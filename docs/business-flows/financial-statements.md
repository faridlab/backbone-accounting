# Financial Statements — Business Flow + Golden Cases

> Owning module: `backbone-accounting` · Implemented in
> `src/application/service/reporting_service.rs`, proven by `tests/reporting_golden_cases.rs`.
> Read-only reports computed from the immutable `ledgers` table (any as-of date / period works).

## Statements

| Report | Endpoint | Computation |
|--------|----------|-------------|
| **Trial Balance** (Neraca Saldo) | `GET /accounting/reports/trial-balance?company_id&as_of` | Per detail account: net = Σdebit − Σcredit (≤ as_of); shown on its normal side. **Σdebit == Σcredit** always. |
| **Balance Sheet** (Neraca) | `GET /accounting/reports/balance-sheet?company_id&as_of` | Assets (Σ asset normal), Liabilities, Equity, **Current Earnings** (Revenue − Expenses, not yet closed). **Assets == Liabilities + Equity + Current Earnings**. |
| **Income Statement** (Laba Rugi) | `GET /accounting/reports/income-statement?company_id&period_start&period_end` | Revenue + Other Income − COGS − Expenses − Other Expense = **Net Income**, over the period. |

## Sign convention (matches posting)

- Debit-normal (asset, expense, cogs, other_expense): balance = Σdebit − Σcredit.
- Credit-normal (liability, equity, revenue, other_income): balance = Σcredit − Σdebit.
- The global ledger is always balanced, so Trial Balance foots and Balance Sheet balances.
- **Period close is out of scope** — undistributed profit lives in Current Earnings, not yet
  rolled to Retained Earnings.

## Golden cases (exact — derived from posting GC-1 / GC-3)

### RGC-1 — after one sales invoice (GC-1: A/R 1,110,000 · Revenue 1,000,000 · PPN Output 110,000)
- Trial Balance: total debit = total credit = **1,110,000.00**, 3 lines, balanced.
- Income Statement: revenue **1,000,000.00**, expenses 0 → net income **1,000,000.00**.
- Balance Sheet: assets **1,110,000.00** = liabilities **110,000.00** + equity 0 + current earnings **1,000,000.00**.

### RGC-2 — after sales + purchase (GC-1 + GC-3)
- Trial Balance total debit = **1,665,000.00** (A/R 1,110,000 + Expense 500,000 + PPN Input 55,000), balanced.
- Income Statement: revenue 1,000,000 − expenses 500,000 = net income **500,000.00**.
- Balance Sheet: assets **1,165,000.00** = liabilities **665,000.00** + current earnings **500,000.00**.

### RGC-3 — period filter
- Income Statement for a July window (no activity) → revenue 0, net income 0.
- Balance Sheet as-of before the posting date → assets 0, balanced.

## Not yet implemented
Cash-Flow Statement (operating/investing/financing), consolidated statements across companies,
comparative periods, and statement snapshotting into the `FinancialStatement` entity.
