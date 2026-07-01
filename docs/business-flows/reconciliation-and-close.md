# Bank Reconciliation & Period Close — Business Flow + Golden Cases

> Owning module: `backbone-accounting`. Implemented in
> `bank_reconciliation_service.rs` / `period_close_service.rs`; proven by
> `tests/reconciliation_golden_cases.rs` and `tests/period_close_golden_cases.rs`.

## Bank reconciliation matching

Match imported bank-statement lines against unreconciled `ledgers` rows on a bank account.

- Endpoint: `POST /accounting/reconcile` (body = `ReconcileRequest`).
- Algorithm: greedy match by **exact signed amount** (ledger net = debit − credit). Matched
  ledger rows are flagged `is_reconciled = true` and linked to a new `Reconciliation`;
  `ReconciliationItems` record matched / unmatched-book / unmatched-statement.
- Result: `matched_count`, `unmatched_book`, `unmatched_statement`, `closing_book_balance`,
  `closing_statement_balance`, `difference`, `is_balanced`. All in one transaction.

**Golden cases**
- **RCG-1 (partial):** 3 receipts (100/200/300); statement (100/200/999) → matched 2,
  unmatched_book 1, unmatched_statement 1, book 600 vs statement 1299, not balanced; exactly
  2 ledger rows flagged reconciled.
- **RCG-2 (full):** statement (100/200/300) → matched 3, none outstanding, difference 0,
  balanced; 3 ledger rows reconciled.

*Later:* timing/partial matches, many-to-one, auto-import from `bank_transaction`.

## Fiscal-period close

Zero the period's P&L accounts into Retained Earnings and lock the period. **Composes the
module's own GL-posting contract** — it builds a balanced closing entry and posts it through
`PostingService`, then flips the `FiscalPeriod` to `closed`.

- Endpoint: `POST /accounting/periods/{period_id}/close`
  (body = `{ company_id, retained_earnings_account_id }`).
- Closing entry: debit each revenue account by its balance, credit each expense account by its
  balance, and post the **net income** to Retained Earnings (profit → credit, loss → debit).
- Guards: period must be **open** (closed/locked → rejected); no P&L activity → close with no
  entry.

**Golden cases**
- **PCG-1:** Revenue 1,000,000 − Expense 400,000 → close rolls **net income 600,000** to
  Retained Earnings; Revenue and Expense balances become 0; period status = `closed`; Bank
  untouched (600,000).
- **PCG-2:** closing an already-closed period → rejected (`AlreadyClosed`).

*Later:* period-closing voucher, opening-balance carry-forward, year-end vs month-end close.

## Event bus

GL-posting emits `AccountingPostPosted` (success) / `AccountingPostFailed` (rejection) through a
`PostingEventSink` (the extension seam). Default `LoggingSink` traces them; a `backbone-messaging`
adapter can implement the trait for real bus delivery. Verified by the `events_emitted` test.
