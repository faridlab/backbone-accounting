# BRD — Accounting (`backbone-accounting`)

> Business Requirements Document. Captures the **business flows and rules** the GL core must obey —
> the source of the BDD oracle. Each flow below becomes a `.feature` file + golden numeric cases.
> Grounded in the real schema (`schema/models/*.model.yaml`) and `docs/erp/gl-posting-contract.md`.

## 1. Business context

**Ubiquitous language:**
- **Account** — a Chart-of-Accounts entry. Classified by `account_type` (asset | liability | equity | revenue | expense | cogs | other_income | other_expense), `account_subtype` (e.g. `accounts_receivable`, `accounts_payable`, `bank`, `cash`, `tax`…), and `normal_balance` (debit | credit). A **detail** account (`is_detail = true`, `is_header = false`) can be posted to; a **header** account cannot.
- **Journal** — a balanced double-entry transaction (header + lines). `total_debit` must equal `total_credit`.
- **JournalLine** — one debit *or* one credit against one account (`debit_amount` xor `credit_amount`).
- **Ledger** — the immutable general-ledger entry with a **running balance** per account (`balance_before` → `balance_after`, `sequence_number` ordering).
- **AccountingPost** — the inbound posting request from a producing module; the record of *why* a journal exists (`source_type`, `source_id`), its status, and its reversal links.
- **FiscalPeriod** — an accounting period (`monthly | quarterly | yearly | custom`) with a lifecycle (`open → closing → closed → locked`, plus `adjusting`). Posting is only allowed into an open period.
- **Party** — the customer/supplier/employee a receivable/payable balance belongs to (`party_type`, `party_id`).
- **CostCenter / dimensions** — controlling dimensions on a line: `cost_center_id`, `project_id`, `department_id`, plus a `dimensions` JSON bag.

**Tenancy:** schema-per-tenant. Every entity carries `company_id` (books owner, logical FK to `corporate.Company`); there is **no `provider_id`**. Isolation is a DB `search_path` concern.

## 2. Actors & roles

| Actor | Acts on | Permissions |
|-------|---------|-------------|
| Bookkeeper | Journal (draft/submit), Account (read), Reconciliation (draft) | Create manual journals; cannot approve above threshold; cannot close periods |
| Accountant | JournalLine (AR/AP), Reconciliation, FiscalPeriod (prepare) | Reconcile accounts; read AR/AP aging; prepare close |
| Controller | Journal (approve/reject/void), FiscalPeriod (close/lock), FinancialStatement (approve) | Approve above-threshold journals; close & lock periods; sign statements |
| Producer module (system) | AccountingPost (emit) | Emit posting requests via the inbound contract; subscribe to status events. No direct write to Journal/Ledger |

## 3. Business flows

### Flow: Receive & post an `AccountingPost`
- **Actor / trigger:** a producer module (billing/payments/…) emits an `AccountingPost` with `lines[]`.
- **Preconditions:** referenced accounts exist, are `is_detail`, `status = active`, in the same `company_id`; the target fiscal period is open.
- **Main path:**
  1. Accounting receives the post (`posting_status = pending`).
  2. Validate the contract rules (§5 R1–R6).
  3. Create a `Journal` (`status = posted`, `source = order|payment|…`) + one `JournalLine` per posting line.
  4. For each line, create an immutable `Ledger` entry: compute `balance_before` from the account's last entry (by `sequence_number`), set `balance_after` and `balance_change`, increment `sequence_number`.
  5. Update `Account.current_balance`; set `AccountingPost.journal_id`, `posting_status = posted`, `posted_at`.
  6. Emit `AccountingPostPosted { post_id, source_type, source_id, journal_id, status }`.
- **Alternate / exception paths:**
  - Any validation fails → `posting_status = failed`, set `error_code`/`error_message`/`failed_at`, write **no** ledger rows, emit `AccountingPostFailed`. Retryable failures (transient) schedule `next_retry_at` while `retry_count < max_retries` (default 3).
  - Duplicate `(source_type, source_id, posting_type, journal_id)` → idempotent no-op returning the original result (see §6 open item on `idempotency_key`).
- **Business rules:** R1–R6 (§5).
- **Postconditions / side effects:** one `Journal`, N `JournalLine`, N `Ledger` rows; account balances updated; `AccountingPostPosted` emitted.
- **Golden cases:**
  - **G1 (sales-invoice post, IDR):** lines = `[Dr A/R 1_110_000; Cr Revenue 1_000_000; Cr PPN Output Payable 110_000]` (PPN 11% computed upstream). Σdebit = 1_110_000 = Σcredit → **posted**. A/R line carries `party_type = customer`, `party_id`. Ledger A/R `balance_change = +1_110_000`.

### Flow: Manual journal (draft → approved → posted → voided)
- **Actor / trigger:** bookkeeper creates a `Journal` with `source = manual`, `status = draft`.
- **Preconditions:** at least two lines; balanced; period open.
- **Main path:**
  1. `draft` → submit → `pending_approval` (if `requires_approval` / amount ≥ `approval_threshold`) else straight toward posting.
  2. Controller `approve` → `approved` (stamp `approved_by`/`approved_at`); or `reject` → `rejected` (with `rejection_reason`).
  3. `approved` → post → `posted`: writes `JournalLine.is_posted = true` + `Ledger` rows exactly as the posting flow above; stamp `posted_by`/`posted_at`.
- **Alternate / exception paths:** `voided` — a posted journal is voided by emitting a **reversing** entry (never edited); stamp `is_voided`, `voided_by`, `void_reason`.
- **Business rules:** balance (R1), ≥2 lines (R2), period open (R4), account postable (R6); approval threshold on `total_debit`.
- **Postconditions / side effects:** ledger written on `posted`; no ledger effect in `draft`/`pending_approval`/`approved`.
- **Golden cases:**
  - **G2 (out-of-balance):** lines `[Dr Cash 500_000; Cr Revenue 400_000]` → Σdebit 500_000 ≠ Σcredit 400_000 → **rejected at validation**, no ledger rows.
  - **G3 (AR missing party):** line `[Dr A/R 250_000]` (account_subtype = accounts_receivable) with no `party_id` → **rejected** (R3).

### Flow: Reversal (undo without editing)
- **Actor / trigger:** producer emits `AccountingPost` with `posting_type = reversal`, same `source_type`/`source_id`, swapped debit/credit; or controller voids a manual journal.
- **Preconditions:** original journal is `posted` and not already reversed.
- **Main path:**
  1. Create a reversing `Journal` (`journal_type = reversing`, `is_reversing = true`, `reverses_id = <original>`); set original `is_reversed = true`, `reversed_by_id`, `reversed_at`.
  2. Write mirror `Ledger` rows (debit↔credit swapped) with fresh `sequence_number`; original ledger rows are untouched (`is_reversed = true`, `reversed_by_id`).
  3. Link posts: `AccountingPost.reverses_post_id` / `reversed_by_post_id`.
- **Business rules:** the reversal lands in the **current open period**, not the original's period (R7). The original ledger row is **never** updated in place except its reversal-link flags (R8, immutability).
- **Golden cases:**
  - **G4 (reversal of G1):** reversing lines `[Cr A/R 1_110_000; Dr Revenue 1_000_000; Dr PPN Output 110_000]`; A/R ledger nets to 0; original G1 rows retained.

### Flow: Fiscal period close (Record-to-Report — no cross-module seam)
- **Actor / trigger:** controller closes a `FiscalPeriod`.
- **Preconditions:** period is `open` (or `adjusting`); statements prepared.
- **Main path:** `open` → `closing` (`closing_started_at/by`) → close revenue/expense to retained earnings via a `closing` journal → cache summary balances (`total_debits`, `total_credits`, `net_income`, `total_assets/liabilities/equity`) → `closed` (`closed_at/by`) → optionally `locked`.
- **Alternate / exception paths:** `adjusting` status allows adjusting entries after close when `allow_adjustments = true` (until `adjustment_deadline`).
- **Business rules:** once `closed`, **no new posts** into the period (R4); reversals/adjustments land in the current open period. `locked` allows no changes at all.
- **Postconditions / side effects:** period summary cached; `balance_sheet_generated`/`income_statement_generated` may flip; statements generated.
- **Golden cases:**
  - **G5 (closed-period post):** `AccountingPost.posting_date` in a `closed` period → **rejected** (R4), no ledger rows, `AccountingPostFailed`.

### Flow: AR/AP subledger aging
- **Actor / trigger:** accountant reads party-level balances.
- **Main path:** read `Ledger` filtered by `company_id`, `party_type`, `party_id`, `account_id` (aging index) — balances are **source-independent** (manual journals, one payment settling many invoices all roll up per party).
- **Business rules:** every AR/AP ledger line carries a party (R3); aging is computed off the ledger, not off source documents.
- **Golden cases:**
  - **G6 (multi-invoice settlement):** one payment `[Dr Bank 3_000_000; Cr A/R 3_000_000 (party=Cust-X)]` settling three invoices → Cust-X A/R balance reads correctly off the ledger without reference to invoice documents.

### Flow: Trial balance & statements
- **Actor / trigger:** controller generates a `FinancialStatement`.
- **Main path:** aggregate `Ledger` by account for the period → `trial_balance` (Σdebit vs Σcredit), `balance_sheet` (A = L + E), `income_statement` (Revenue − COGS − Expenses = Net Income).
- **Business rules:** `trial_balance_check = (total_debits == total_credits)`; `balance_check = (total_assets == total_liabilities + total_equity)`, with `balance_difference` recorded if not.
- **Golden cases:**
  - **G7 (trial balance):** after G1, Σdebit across accounts = Σcredit = 1_110_000 → `trial_balance_check = true`.

### Flow: Bank reconciliation
- **Actor / trigger:** accountant reconciles a `bank`/reconcilable account against a statement.
- **Main path:** open a `Reconciliation` (`in_progress`) with opening book/statement balances → import statement → create `ReconciliationItem`s (book side from `Ledger`, statement side from the import) → match (`auto | manual | rule`) → classify unmatched as outstanding (deposit_in_transit / outstanding_check) or as adjustments (bank_charge / interest / nsf) → create adjusting journals → `is_balanced` when `difference = 0` → `pending_review` → `reviewed` → `completed`.
- **Business rules:** `difference = adjusted_book_balance − adjusted_statement_balance` must be 0 to complete; adjusting entries post through the normal journal flow (so R1–R6 apply); reconciled ledger lines set `is_reconciled = true`.
- **Golden cases:**
  - **G8 (bank charge):** statement shows Rp 25_000 charge not in books → item `adjustment_type = bank_charge` → adjusting journal `[Dr Bank Charges 25_000; Cr Bank 25_000]` → `difference = 0` → `is_balanced = true`.

## 4. Cross-module hand-offs

| Seam | Direction | Mechanism |
|------|-----------|-----------|
| Producer → accounting | inbound | Producer emits `AccountingPost` (Tier-A inbound contract). Accounting imports no producer. |
| Accounting → producer | outbound | `AccountingPostPosted` / `AccountingPostFailed` events (no synchronous call-back). Producer reconciles its own document state. |
| Tax lines | inbound (data) | Computed by `backbone-tax-id`, arrive inside `AccountingPost.lines[]`; accounting stores them as ordinary lines (`is_tax_line`, `tax_rate`, `tax_base_amount`). |
| AR/AP party | reference | `party_id` is a logical FK to `party.Party`; resolved at the producer/ACL for display. |

## 5. Business rules catalog

| # | Rule | Source | Testable as |
|---|------|--------|-------------|
| R1 | **Double-entry invariant:** for every journal/post, Σ`debit` = Σ`credit` (base currency). Reject otherwise. | gl-posting-contract §3.1 | G1 posts, G2 rejects |
| R2 | **≥ 2 lines**, all referencing detail (non-header) accounts in the same `company_id`. | gl-posting-contract §3.2 | G2 (1-sided) rejects |
| R3 | **Party required** iff the line's account `account_subtype ∈ {accounts_receivable, accounts_payable}`. | ADR-001 #1 | G3 rejects; G1/G6 carry party |
| R4 | **No posting into a closed period:** `posting_date` must fall in a fiscal period with `status = open` (or `adjusting`). | gl-posting-contract §3.4 | G5 rejects |
| R5 | **Idempotent:** a repeat of `(source_type, source_id, posting_type, journal_id)` returns the original result, never double-posts. | schema unique index; gl-posting-contract §3.5 | duplicate-post no-op |
| R6 | **Account postable:** target account is `is_detail`, `status = active`, and `allow_manual_entry` honored for manual journals. | gl-posting-contract §3.6 | post to header/inactive rejects |
| R7 | **Reversal-not-edit:** to undo, emit a reversal post/reversing journal (swapped debit/credit); it lands in the current open period. | gl-posting-contract §5 | G4 |
| R8 | **Immutable ledger:** a posted `Ledger` row is never mutated except its reversal-link flags (`is_reversed`, `reversed_by_id`). | financials §Record-to-Report; schema | G4 retains original rows |
| R9 | **Running balance:** each `Ledger` row records `balance_before`, `balance_change`, `balance_after` and a monotonic `sequence_number` per account. | ledger.model.yaml | balance continuity assertion |
| R10 | **Period lifecycle:** `open → closing → closed → locked`; `adjusting` permits adjusting entries when `allow_adjustments`. | fiscal.model.yaml | period state machine |
| R11 | **Statement identities:** `trial_balance_check = (Σdebit == Σcredit)`; `balance_check = (assets == liabilities + equity)`. | financial_statement.model.yaml | G7 |

## 6. Indonesia compliance rules (if any)

Accounting is region-neutral; compliance is **DEFERRED** to the overlay (`localization-standard.md`,
`tax-compliance.md`) and owned by `backbone-tax-id`:
- **DEFERRED — PPN (11%) / PPh withholding:** computed by `backbone-tax-id` and delivered as lines in
  `AccountingPost`. Accounting only *stores* them (PPN Output Payable, PPN Input, PPh Payable are
  seeded accounts). Regulatory source: DJP regulations; e-Faktur / Coretax. Author later with a reviewer.
- **DEFERRED — SAK-EMKM / PSAK statement presentation:** COA seed selects account structure; formal
  PSAK statement layout is an overlay/report concern, not a base rule.
- **Open item — `idempotency_key`:** the contract requires it but the schema lacks the field; today
  idempotency is the unique index R5. Resolve before codegen (also flagged in PRD §9 / FSD §10).

## 7. Acceptance

The module is "done" when these Gherkin scenarios (from G1–G8) pass:
- **post_balanced_sales_invoice** (G1) — balanced post → one journal, N ledger rows, `AccountingPostPosted`.
- **reject_out_of_balance** (G2) — Σdebit ≠ Σcredit → `AccountingPostFailed`, no ledger rows.
- **reject_ar_line_without_party** (G3) — receivable line without party → rejected.
- **reverse_posted_journal** (G4) — reversal mirrors and links; original ledger retained.
- **reject_post_into_closed_period** (G5) — closed period → rejected.
- **ar_aging_source_independent** (G6) — party balance reads off the ledger across sources.
- **trial_balance_ties** (G7) — Σdebit = Σcredit; balance sheet balances.
- **bank_reconciliation_balances** (G8) — bank charge adjustment drives difference to 0.
- **cross_module_round_trip** (extension-contract §5) — billing → accounting post + consumer custom rule survive regeneration of both modules.
