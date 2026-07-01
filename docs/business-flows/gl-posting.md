# Business Flow — GL Posting (General Ledger core)

> Owning module: `backbone-accounting` · Status: **documented** (oracle authored; Rust wiring by parent)
> Contract seam: `docs/erp/gl-posting-contract.md` · ADR: `docs/adr/ADR-001-gl-core-boundary.md`
> Scenarios: `tests/features/gl_posting.feature` · Golden cases: `docs/business-flows/golden-cases.md`

This document captures the business behavior of the **single inbound port** through which every
transactional module records its financial effect in the General Ledger. It is the load-bearing
seam of the Financials pillar: producers (billing, payments, banking, inventory, assets…) *emit a
posting request*; accounting owns all GL-writing logic and never imports a producer.

Money is exact throughout: `decimal(18,2)`, IDR (2 decimal places / minor units), never floating
point. **Tax amounts are inputs, not computations** — the producer (via `backbone-tax-id`) has
already computed every PPN / PPh line and included it in `lines[]`. Accounting only *records* what
it is given, after validating double-entry integrity.

## Ubiquitous language (terms used below)

All terms are drawn from the schema / ADR / contract. No new terms introduced.

| Term | Meaning (source) |
|------|------------------|
| Posting request / `AccountingPost` | The inbound DTO a producer emits (`accounting_post.model.yaml`) |
| `idempotency_key` | Producer-stable dedupe key (contract §3.5) |
| `Journal` / `JournalLine` | Double-entry record header + lines (`journal.model.yaml`) |
| `Ledger` | Immutable GL entry with running balance (`ledger.model.yaml`) |
| Detail account | `Account.is_detail = true`, postable (`account.model.yaml`) |
| Header account | `Account.is_header = true`, not postable |
| Party | `party_type` (customer/supplier/employee) + `party_id`, on AR/AP lines |
| AR / AP subledger | Per-party balances derived from `Ledger` party columns |
| `FiscalPeriod` | Accounting period; `status ∈ {open, closing, closed, locked, adjusting}` |
| Reversal | `posting_type = reversal`, swapped debit/credit, linked via `reverses_post_id` |
| PPN Output / PPN Input | Indonesian VAT payable / recoverable (11%) — pre-computed tax line |
| PPh (bukti potong) | Indonesian withholding tax payable — pre-computed tax line |

---

## Flow 1 — Post a balanced entry

**Actors**: a producing module (billing / payments / banking / inventory / assets — the *initiator*);
`backbone-accounting` GL core (the *processor*); event-bus subscribers (producers reconciling their
own document state — *observers*).

**Preconditions**
- The `company_id` books exist; the referenced `Account`s exist in that company, are `is_detail`,
  `status = active`, and honor `allow_manual_entry`.
- The `posting_date` falls inside a `FiscalPeriod` whose `status = open`.
- Every AR/AP line carries a party (see Rule R4).

**Trigger**: a producer emits an `AccountingPost { idempotency_key, company_id, branch_id?,
source_type, source_id, source_reference?, posting_date, currency, posting_type=original, lines[] }`.

**Main path** (maps to the application-layer *post* use case)
1. Accounting receives the request and runs all validation rules (R1–R6 below) atomically.
2. It creates one `Journal` (header) with `total_debit = total_credit = Σ`, `line_count`,
   `journal_number`, `status = posted`, `source_* ` copied from the request.
3. For each line it creates a `JournalLine` (account, debit_amount/credit_amount, party, dimensions).
4. For each line it appends an immutable `Ledger` entry with `balance_before`, `balance_after`,
   `balance_change`, and a per-account `sequence_number` (running balance).
5. It sets `AccountingPost.journal_id`, `posting_status = posted`, `posted_at`, and copies
   `total_debit` / `total_credit`.
6. It publishes `AccountingPostPosted { post_id, source_type, source_id, journal_id, status }`.

**Postconditions / side-effects (on success)**
- Exactly one `Journal` + N `JournalLine`s + N `Ledger` entries written; nothing else.
- `Σ Ledger.debit_amount == Σ Ledger.credit_amount` for the journal (books stay balanced).
- Each posted `Account.current_balance` reflects the applied `balance_change`.
- `AccountingPost.posting_status = posted`, `journal_id` set.
- `AccountingPostPosted` emitted exactly once.

**Business rules / invariants**
- **R1 — Balanced**: `Σ debit == Σ credit` in base currency (exact decimal equality).
- **R2 — Minimum lines**: `lines.length >= 2`.
- **R3 — Postable accounts**: every `account_id` is `is_detail = true`, `status = active`, in the
  same `company_id`; a header / inactive / foreign-company account is rejected.
- **R4 — Party rule**: `party_type` + `party_id` are REQUIRED **iff** the line's
  `account_subtype ∈ {accounts_receivable, accounts_payable}`; forbidden (or ignored — see
  reconciliation) otherwise. Missing party on an AR/AP line → reject. Party on a non-AR/AP line is
  a producer error → reject.
- **R5 — Open period**: `posting_date` must fall in a `FiscalPeriod` with `status = open`. A
  `closed` / `locked` period → reject. (`adjusting` is out of scope for original posts here.)
- **R6 — Idempotency**: a second request with the same `idempotency_key` returns the original
  `AccountingPost` / `journal_id`; no second journal or ledger rows are written.

**Exception paths** — each rule violation → the request is **rejected with error code 422** (or a
specific code below), `AccountingPost.posting_status = failed` (or is never created for pre-persist
rejects), **zero `Journal` / `JournalLine` / `Ledger` rows written**, `AccountingPostFailed`
emitted. The whole operation is atomic: partial writes are never observable.

| Rule | Error code | Business reason |
|------|-----------|-----------------|
| R1 | `422 unbalanced` | Σdebit ≠ Σcredit |
| R2 | `422 too_few_lines` | fewer than 2 lines |
| R3 | `422 non_postable_account` | header / inactive / wrong-company account |
| R4 | `422 party_required` / `422 party_not_allowed` | AR/AP party missing, or party on non-AR/AP line |
| R5 | `422 period_closed` | posting date in a closed/locked period |

---

## Flow 2 — Reject an unbalanced entry (exception of Flow 1)

**Trigger**: producer emits a post where `Σ debit ≠ Σ credit`.
**Path**: R1 fails before any write. **Postcondition**: `422 unbalanced`; zero Journal / JournalLine
/ Ledger rows; no `Account.current_balance` change; `AccountingPostFailed` emitted. This is the
canonical "no partial write" invariant — asserted by golden case **GC-5**.

---

## Flow 3 — Reject structural / account / period violations (exceptions of Flow 1)

Covers R2 (< 2 lines), R3 (non-detail / inactive / wrong-company account), R5 (closed period). Same
guarantee as Flow 2: pre-write rejection, zero rows, typed error, failure event.

---

## Flow 4 — Party rule on AR/AP lines (alternate + exception of Flow 1)

**Rule R4** governs the AR/AP subledger.
- **Happy alternate**: an AR line (`account_subtype = accounts_receivable`) *with* a customer party,
  or an AP line (`accounts_payable`) *with* a supplier party → accepted; the party propagates to
  `JournalLine.party_*` and `Ledger.party_*`.
- **Exception a**: an AR/AP line *without* a party → `422 party_required`.
- **Exception b**: a non-AR/AP line (e.g. revenue, bank) *carrying* a party → `422 party_not_allowed`.

**Postcondition (happy)**: ledger entries for the AR/AP account carry `party_type` + `party_id`,
making per-party balances derivable (see Flow 7).

---

## Flow 5 — Idempotent retry (alternate of Flow 1)

**Actors**: same producer retrying after a timeout / at-least-once delivery.
**Precondition**: an `AccountingPost` with `idempotency_key = K` was already `posted`.
**Trigger**: the producer emits a second post with the same `idempotency_key = K`.
**Path**: accounting recognizes `K`, short-circuits, and returns the original `post_id` /
`journal_id`.
**Postconditions**: no new `Journal` / `JournalLine` / `Ledger` rows; the account balance is
unchanged from the first post; the caller receives the original posted result (not an error).
Asserted structurally: after two identical posts, row counts equal a single post's.

---

## Flow 6 — Reversal (alternate of Flow 1)

**Actors**: a producer undoing a previously posted effect (a producer never edits posted GL).
**Precondition**: an original `AccountingPost` P (`posting_type = original`) is `posted` with journal
`J`.
**Trigger**: the producer emits a post with `posting_type = reversal`, the same `source_*`, and
`reverses_post_id = P.id` (lines may be omitted and derived from `J`, or supplied swapped — the
result is identical).
**Main path**
1. Accounting creates a reversing `Journal` `J'` whose every line has debit/credit **swapped**
   relative to `J` (`is_reversing = true`, `reverses_id = J.id`, `journal_type = reversing`).
2. It writes the immutable `Ledger` entries for `J'` (the reversing balance_change is the negation).
3. It links the posts: `P.reversed_by_post_id = P'.id`, `P'.reverses_post_id = P.id`; and journals
   `J.reversed_by_id = J'.id`, `J.is_reversed = true`, `J'.reverses_id = J.id`.
4. The reversal lands in the **current open period** even if the original period is now closed.
**Postconditions / invariants**
- **Net GL effect of {J, J'} is zero** for every account (`Σ debit == Σ credit` across the pair per
  account). Asserted by golden case **GC-4**.
- AR/AP party balances touched by `J` return to their pre-`J` value after `J'`.
- The reversal is itself a balanced journal (R1 still holds).

---

## Flow 7 — AR/AP subledger aging (read model derived from the ledger)

**Actors**: statements / dunning / aging reports (read side); no write.
**Precondition**: AR/AP `Ledger` entries carry `party_type` + `party_id` (Flow 4).
**Trigger**: a query for a party's balance on an AR (or AP) account.
**Path / rule**: the per-party balance is `Σ debit − Σ credit` over `Ledger` rows filtered by
`company_id`, `account_id` (or `account_subtype`), `party_type`, `party_id` — independent of the
source document. A sales invoice (Dr A/R, party = customer) raises the balance; a payment (Cr A/R,
same party) lowers it. One payment can settle many invoices; the balance is still correct because it
reads the ledger, not the documents.
**Postcondition**: a fully-paid customer's A/R balance is `0` (golden case **GC-2**). The
subledger reconciles to the A/R control account: `Σ over all parties == control-account balance`.

---

## Traceability to golden cases

| Flow | Golden case(s) |
|------|----------------|
| 1 Post balanced | GC-1 (sales invoice + PPN), GC-3 (purchase invoice + PPN Input + PPh) |
| 2 Reject unbalanced | GC-5 |
| 4 Party rule | GC-1 (AR party set), GC-6 (party missing → reject), GC-7 (party on revenue → reject) |
| 6 Reversal | GC-4 |
| 7 AR/AP aging | GC-2 (payment settles A/R to 0) |
</content>
