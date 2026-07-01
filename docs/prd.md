# PRD — Accounting (`backbone-accounting`)

> Product Requirements Document. Answers **why** this module exists and **what** it must do.
> Tier 1 · Pillar: Financials · Status: schema active (10 models, ADR-001 applied 2026-06-30; code not yet generated).

## 1. Summary

`backbone-accounting` is the **General Ledger core** — the ledger of record and the hub of the
Financials pillar for an Indonesia-first ERP. It holds double-entry truth (Account, Journal,
JournalLine, Ledger) and owns the **single inbound posting port** (`AccountingPost`) through which
every transactional module (billing, payments, banking, inventory, assets, later payroll) records
its financial effects. It does not originate business documents and does not emit postings — it
*receives* them, validates double-entry, writes the journal and immutable ledger, manages the
period and reversal lifecycle, and reports status back via events. It replaces ERPNext's implicit
`make_gl_entries()` side-effect (baked into 30+ controllers) with one explicit, decoupled seam.

## 2. Problem & motivation

- **The pain today.** In ERPNext, GL posting is an implicit side-effect: every controller
  (`sales_invoice.on_submit` → `make_gl_entries()`, purchase invoice, stock entry, payroll…)
  reaches *directly* into `gl_entry`. That direct coupling across 30+ controllers *is* the monolith
  — you cannot change one document's accounting without risking every other, and you cannot swap or
  reason about the ledger in isolation. An Indonesia SMB inherits all of it: a 192-DocType
  "Accounts" folder that is really ~8 bounded contexts collapsed into one.
- **Why its own bounded context.** The ledger of record has one owner, one ubiquitous language
  (accounts, journals, debits/credits, periods), and one invariant (Σdebit = Σcredit). Folding it
  into billing or payments would re-create the ERPNext coupling. Keeping it separate — with a single
  documented port — lets every other Financials module stay independent of the GL's internal schema.

## 3. Personas & jobs-to-be-done

| Persona | Job | Success looks like |
|---------|-----|--------------------|
| **Bookkeeper** (Pembukuan) | Record day-to-day manual journals, capture receipts, keep the books tidy | Posts a balanced manual journal in seconds; rejected instantly if it doesn't balance; can't touch a closed period |
| **Accountant** (Akuntan) | Own AR/AP subledgers, reconcile bank accounts, prepare period close | Reads party-level AR/AP aging straight off the ledger; reconciles a bank account against a statement; runs a clean period close |
| **Controller / Finance Manager** | Approve high-value journals, close fiscal periods, sign off statements | Approves above-threshold journals; closes a period once and it locks; trial balance and balance sheet tie out (A = L + E) |
| **Developer-consumer** (billing/selling/payments engineer) | Drive the GL from another module without importing it | Emits one `AccountingPost`, gets a `posted`/`failed` event back, never learns the GL's schema, survives regeneration of both modules |

## 4. Scope

**In scope** (the bounded context's responsibilities):
- Chart of Accounts (`Account`, tree, SAK/PSAK-shaped via seed) and cost-center dimension (`CostCenter`).
- Journals + lines (`Journal`, `JournalLine`) with an approval → posting lifecycle.
- The immutable general ledger with running balances (`Ledger`).
- The `AccountingPost` inbound posting contract, its validation, and its async/reversal lifecycle.
- Fiscal periods and period close (`FiscalPeriod`).
- Financial-statement read models (`FinancialStatement`: balance sheet, income statement, trial balance, cash flow, equity).
- Account reconciliation (`Reconciliation`, `ReconciliationItem`).
- Exported events (`AccountingPostPosted` / `AccountingPostFailed`) and exported DTOs.

**Out of scope / deferred** (an Indonesia SMB does not need these yet — cite the vs-ERP call in `financials.md §4`):
- **Tax rules** → owned by `backbone-tax-id`. Accounting never computes PPN/PPh; tax lines arrive inside `AccountingPost.lines[]` already computed.
- **Invoicing (AR/AP documents)** → owned by `backbone-billing`. Accounting stores party-level balances, not invoices.
- Parallel ledgers (IFRS + local `finance_book`) — **single ledger** + PSAK/SAK only.
- Document splitting; runtime dynamic accounting-dimension engine; cost-center percentage-split allocation; budgeting beyond the lightweight `has_budget`/`budget_amount` on `Account`.
- Equity/shareholder, bank guarantee, invoice discounting — parked, promote on demand.

**Explicitly owned by another module** (logical refs out — no DB FK):
- `corporate.Company` (`company_id`, books owner), `corporate.Branch` (`branch_id`, dimension), `corporate.Department` (`department_id`).
- `party.Party` (`party_id` on AR/AP ledger lines).
- `projects.Project` (`project_id` dimension).
- `sapiens.User` (audit actors — imported, not owned).

## 5. Key capabilities (prioritized)

| # | Capability | Priority | Notes |
|---|------------|----------|-------|
| 1 | Receive & post an `AccountingPost` (validate double-entry, write Journal + Ledger, emit event) | P0 | The load-bearing seam; every Tier-2 module depends on it |
| 2 | Chart of Accounts CRUD + tree (header vs detail, subtype-driven behavior) | P0 | Detail accounts are postable; subtype `accounts_receivable`/`accounts_payable` drives party-required |
| 3 | Manual journal with draft → approved → posted lifecycle + void | P0 | Bookkeeper/controller path; approval threshold |
| 4 | Immutable ledger with running balance per account | P0 | Reversal-not-edit; `balance_before`/`balance_after`/`balance_change`/`sequence_number` |
| 5 | Fiscal period lifecycle + no-posting-into-closed-period | P0 | `open → closing → closed → locked`; blocks posts |
| 6 | AR/AP subledger aging off the ledger (party-level) | P1 | Source-independent, per ADR-001 #1 |
| 7 | Reversal via new post (`posting_type = reversal`), never edit | P1 | Links `reverses_post_id`/`reversed_by_post_id` |
| 8 | Cost centers + 3 hard-coded dimensions (cost_center/project/department) + JSON bag | P1 | Per ADR-001 #2/#3; allocation deferred |
| 9 | Financial statements (trial balance, balance sheet, income statement) read models | P1 | `balance_check` (A = L + E), `trial_balance_check` |
| 10 | Bank / account reconciliation | P2 | `Reconciliation` + items; auto/manual match, adjusting entries |

## 6. Indonesia-first considerations

The core is **region-neutral**; Indonesian behavior arrives via an **overlay data seed**, never baked
into base enums (per `localization-standard.md §2`):
- **IDR default** — `currency` defaults to `'IDR'` on `Account`, `Journal`, `JournalLine`, `Ledger`, `AccountingPost`, `FinancialStatement`.
- **SAK/PSAK Chart of Accounts** — the Indonesian COA (SAK-EMKM / PSAK) is a seed of `Account` rows, selected by `Company.locale_profile` (default `id`). It is not an enum. Account names carry both `name` (Indonesian, e.g. *Piutang Usaha*, *Beban*) and `name_en`.
- **Tax accounts** — PPN Output Payable / PPN Input / PPh Payable are ordinary seeded `Account` rows with `is_taxable`/`tax_rate` fields; postings credit/debit them. Accounting holds them as accounts with **no PPN logic** — tax is computed upstream by `backbone-tax-id`.
- Remove the overlay seed → a clean region-neutral GL still runs. See `localization-standard.md`.

## 7. Success criteria & metrics

Tied to the BRD's BDD oracle flows (`brd.md §7`):
- A balanced `AccountingPost` produces exactly one `Journal`, N `JournalLine`s, N `Ledger` entries with correct running balances, and emits `AccountingPostPosted`. **(Golden case: sales-invoice post.)**
- An unbalanced post (Σdebit ≠ Σcredit) is rejected, writes no ledger rows, and emits `AccountingPostFailed`. **(Golden case: out-of-balance.)**
- A receivable/payable line **without** a party is rejected. **(Golden case: AR-missing-party.)**
- A post dated into a `closed` fiscal period is rejected. **(Golden case: closed-period.)**
- A reversal post produces a mirror journal (swapped debit/credit), links both posts, and lands in the current open period; the ledger is never edited in place. **(Golden case: reversal.)**
- Trial balance ties (Σdebit = Σcredit across accounts); balance sheet `balance_check` is true (A = L + E). **(Golden case: trial-balance.)**
- **Acceptance gate (`extension-contract.md §5`):** the first cross-module post (`backbone-billing` → accounting) plus a consumer custom rule survive regeneration of both modules with the consumer's logic intact.

## 8. Dependencies & consumers

- **Depends on** (Foundation, logical FK only): `corporate.Company`, `corporate.Branch`, `corporate.Department`; `party.Party`; `projects.Project`; `sapiens.User` (external import, audit actors). Ownership of `Company`/`Branch`/`CostCenter` must be settled with `backbone-corporate` before Tier-0 masters land (ADR-001 consequence).
- **Consumed / extended by** (Tier-2 emitters, none imported by accounting): `backbone-billing` (sales/purchase invoice posts), `backbone-payments` (settlement), `backbone-banking` (clearing), `backbone-inventory`, `backbone-assets`, `backbone-pos`. All bind through `AccountingPost`.

## 9. Open questions

- **Idempotency key.** The contract (`gl-posting-contract.md §3.5`) requires an `idempotency_key` on `AccountingPost`; the current schema has **no such field** — dedupe is enforced today only by the unique index on `(source_type, source_id, posting_type, journal_id)`. Decide: add `idempotency_key` or ratify the composite key as the idempotency contract.
- **Status vocabulary drift.** `gl-posting-contract.md` names `scheduled → posted | failed`; the schema `PostingStatus` enum is `pending | processing | posted | failed | cancelled`. Reconcile the doc/enum before codegen (FSD §3 documents the schema as authoritative).
- **Currency of the balance invariant.** Contract says "Σdebit = Σcredit *in base currency*"; `JournalLine` carries `base_debit_amount`/`base_credit_amount`. Confirm validation runs on base amounts.
- **Company/Branch/CostCenter ownership** with `backbone-corporate` (ADR-001 consequence).
