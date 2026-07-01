# GL Posting — Golden Cases (executable oracle)

> Owning module: `backbone-accounting` · Companion to `gl-posting.md` and `gl_posting.feature`
> Status: **oracle authored** — the parent implements the posting service and asserts these exact
> numbers in Rust tests.

These are the **exact numeric cases the implementation MUST reproduce**. Every amount is IDR in
`decimal(18,2)` (2 decimal places). Money is exact: assert decimal equality, never float
tolerance. Each case has a stable ID (`GC-n`) referenced by scenarios in `gl_posting.feature`.

**Convention for balances (running-balance / normal-balance sign):**
- For a **debit-normal** account (asset, expense, COGS): `balance_change = debit − credit`.
- For a **credit-normal** account (liability, equity, revenue): `balance_change = credit − debit`.
- `balance_after = balance_before + balance_change`. `Ledger.debit_amount` / `credit_amount` are
  stored as the raw posted sides (always non-negative); the sign lives in `balance_change`.

**Tax note:** every PPN / PPh amount below is a **given input line** (pre-computed by the producer /
`backbone-tax-id`). Accounting does not derive it — it records and balance-checks it.

**Chart of accounts used by the golden cases** (one `company_id = ACME`, all `is_detail`, `active`):

| Code | Account | account_type | account_subtype | normal_balance |
|------|---------|--------------|-----------------|----------------|
| 1100 | Bank BCA | asset | bank | debit |
| 1200 | Piutang Usaha (A/R) | asset | accounts_receivable | debit |
| 1210 | PPN Masukan (PPN Input) | asset | tax | debit |
| 2100 | Utang Usaha (A/P) | liability | accounts_payable | credit |
| 2200 | PPN Keluaran (PPN Output) | liability | tax | credit |
| 2300 | Utang PPh 23 (PPh Payable) | liability | tax | credit |
| 4000 | Pendapatan Penjualan (Revenue) | revenue | operating_revenue | credit |
| 5000 | Beban Operasional (Expense) | expense | operating_expense | debit |

Parties: `CUST-1` (customer), `SUPP-1` (supplier).

---

## GC-1 — Sales invoice IDR 1,000,000 + PPN Output 11%

**Input** `AccountingPost` (`source_type = order`, `posting_type = original`, party on A/R line):

| # | Account | Debit | Credit | Party |
|---|---------|------:|-------:|-------|
| 1 | 1200 A/R | 1,110,000.00 | 0.00 | customer CUST-1 |
| 2 | 4000 Revenue | 0.00 | 1,000,000.00 | — |
| 3 | 2200 PPN Output | 0.00 | 110,000.00 | — |

**Expected Ledger rows (exact):**

| Account | debit_amount | credit_amount | balance_change |
|---------|------------:|--------------:|---------------:|
| 1200 A/R | 1,110,000.00 | 0.00 | +1,110,000.00 |
| 4000 Revenue | 0.00 | 1,000,000.00 | +1,000,000.00 |
| 2200 PPN Output | 0.00 | 110,000.00 | +110,000.00 |

**Assertions**
- `Σ debit = 1,110,000.00 == Σ credit = 1,110,000.00` → **balanced** (R1 holds).
- 1 Journal, 3 JournalLines, 3 Ledger rows written; `AccountingPost.posting_status = posted`.
- A/R ledger row carries `party_type = customer`, `party_id = CUST-1`.
- A/R subledger balance for CUST-1 = **+1,110,000.00**.
- `AccountingPostPosted` emitted once.

---

## GC-2 — Payment received IDR 1,110,000 (settles GC-1)

**Precondition**: GC-1 is posted; CUST-1 A/R balance = 1,110,000.00.
**Input** `AccountingPost` (`source_type = payment`, party on A/R line):

| # | Account | Debit | Credit | Party |
|---|---------|------:|-------:|-------|
| 1 | 1100 Bank BCA | 1,110,000.00 | 0.00 | — |
| 2 | 1200 A/R | 0.00 | 1,110,000.00 | customer CUST-1 |

**Expected Ledger rows:**

| Account | debit_amount | credit_amount | balance_change |
|---------|------------:|--------------:|---------------:|
| 1100 Bank BCA | 1,110,000.00 | 0.00 | +1,110,000.00 |
| 1200 A/R | 0.00 | 1,110,000.00 | −1,110,000.00 |

**Assertions**
- Balanced: `Σ debit = 1,110,000.00 == Σ credit = 1,110,000.00`.
- **CUST-1 A/R subledger balance returns to 0.00** (GC-1 +1,110,000 then GC-2 −1,110,000).
- Bank BCA balance = +1,110,000.00.

---

## GC-3 — Purchase invoice IDR 500,000 + PPN Input 55,000 + PPh 23 withholding 10,000

PPh 23 at 2% of the 500,000 service base = 10,000, withheld (a payable to the tax office), so the
net owed to the supplier is `500,000 + 55,000 − 10,000 = 545,000`.
**Input** `AccountingPost` (`source_type = expense`, party on A/P line):

| # | Account | Debit | Credit | Party |
|---|---------|------:|-------:|-------|
| 1 | 5000 Expense | 500,000.00 | 0.00 | — |
| 2 | 1210 PPN Input | 55,000.00 | 0.00 | — |
| 3 | 2100 A/P | 0.00 | 545,000.00 | supplier SUPP-1 |
| 4 | 2300 PPh 23 Payable | 0.00 | 10,000.00 | — |

**Expected Ledger rows:**

| Account | debit_amount | credit_amount | balance_change |
|---------|------------:|--------------:|---------------:|
| 5000 Expense | 500,000.00 | 0.00 | +500,000.00 |
| 1210 PPN Input | 55,000.00 | 0.00 | +55,000.00 |
| 2100 A/P | 0.00 | 545,000.00 | +545,000.00 |
| 2300 PPh 23 Payable | 0.00 | 10,000.00 | +10,000.00 |

**Assertions**
- `Σ debit = 555,000.00 == Σ credit = 555,000.00` → **balanced**.
- 4 JournalLines, 4 Ledger rows; A/P line carries `party_type = supplier`, `party_id = SUPP-1`.
- SUPP-1 A/P subledger balance = +545,000.00.

---

## GC-4 — Reversal of the sales invoice (reverses GC-1)

**Precondition**: GC-1 posted as post `P1` / journal `J1`.
**Input** `AccountingPost` (`posting_type = reversal`, `reverses_post_id = P1.id`, same `source_*`).
**Expected reversing Ledger rows (debit/credit swapped vs GC-1):**

| Account | debit_amount | credit_amount | balance_change |
|---------|------------:|--------------:|---------------:|
| 1200 A/R | 0.00 | 1,110,000.00 | −1,110,000.00 |
| 4000 Revenue | 1,000,000.00 | 0.00 | −1,000,000.00 |
| 2200 PPN Output | 110,000.00 | 0.00 | −110,000.00 |

**Assertions**
- The reversing journal is itself balanced: `Σ debit = 1,110,000.00 == Σ credit`.
- **Net GL effect of {GC-1, GC-4} is zero for every account**: A/R 0, Revenue 0, PPN Output 0.
- Links set: `P1.reversed_by_post_id = P2.id`, `P2.reverses_post_id = P1.id`;
  `J1.is_reversed = true`, `J1.reversed_by_id = J2.id`, `J2.is_reversing = true`,
  `J2.reverses_id = J1.id`.
- CUST-1 A/R subledger balance returns to 0.00.

---

## GC-5 — Unbalanced attempt (Dr 100 · Cr 90) → rejected, zero rows

**Input** `AccountingPost`:

| # | Account | Debit | Credit |
|---|---------|------:|-------:|
| 1 | 5000 Expense | 100.00 | 0.00 |
| 2 | 1100 Bank BCA | 0.00 | 90.00 |

**Assertions**
- `Σ debit = 100.00 ≠ Σ credit = 90.00` → rejected with `422 unbalanced`.
- **Zero Journal, zero JournalLine, zero Ledger rows written** (no partial write).
- No `Account.current_balance` changed for 5000 or 1100.
- `AccountingPost.posting_status = failed`; `AccountingPostFailed` emitted with
  `error_code = "unbalanced"`.

---

## GC-6 — AR line missing party → rejected (R4 exception a)

**Input**: single-purpose variant of GC-1 where line 1 (1200 A/R) has **no** `party_type` /
`party_id`.
**Assertions**: rejected with `422 party_required`; zero rows written; `posting_status = failed`.

---

## GC-7 — Party on a non-AR/AP line → rejected (R4 exception b)

**Input**: GC-1 where line 2 (4000 Revenue, an operating-revenue account) **carries** a
`party_type = customer` / `party_id = CUST-1`.
**Assertions**: rejected with `422 party_not_allowed`; zero rows written; `posting_status = failed`.

---

## GC-8 — Idempotent retry of GC-1 → original returned, no double-write

**Precondition**: GC-1 posted once with `idempotency_key = K1` (journal `J1`).
**Input**: the *same* `AccountingPost` payload emitted again with `idempotency_key = K1`.
**Assertions**
- Returns the original `post_id` / `journal_id = J1.id` (not an error, not a new journal).
- Row counts unchanged: still exactly 1 Journal, 3 JournalLines, 3 Ledger rows total.
- CUST-1 A/R balance still +1,110,000.00 (charged once, not twice).

---

## GC-9 — Single line → rejected (R2)

**Input**: an `AccountingPost` with exactly one line (1100 Bank BCA Dr 100.00).
**Assertions**: rejected with `422 too_few_lines`; zero rows written.

---

## GC-10 — Post into a closed period → rejected (R5)

**Precondition**: the `FiscalPeriod` containing `posting_date` has `status = closed`.
**Input**: an otherwise-valid balanced GC-1-shaped post with that `posting_date`.
**Assertions**: rejected with `422 period_closed`; zero rows written.

---

## GC-11 — Post against a header account → rejected (R3)

**Precondition**: account `1000` is a header (`is_header = true`, `is_detail = false`).
**Input**: a balanced post whose debit line targets header account `1000`.
**Assertions**: rejected with `422 non_postable_account`; zero rows written.

---

## Cross-case invariants (must hold across the whole suite)

1. **Global balance**: after any successful post, `Σ all Ledger.debit_amount == Σ all
   Ledger.credit_amount` for the company.
2. **No partial writes**: every rejected case leaves Journal/JournalLine/Ledger row counts exactly
   as they were before the attempt.
3. **Subledger reconciliation**: `Σ per-party A/R balances == 1200 control-account balance`; same
   for A/P vs 2100.
4. **Running balance continuity**: within one account, consecutive `Ledger` rows satisfy
   `row[n].balance_before == row[n-1].balance_after` and `sequence_number` is strictly increasing.
</content>
