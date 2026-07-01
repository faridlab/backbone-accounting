# FSD — Accounting (`backbone-accounting`)

> Functional Specification. The bridge from BRD → schema/code. Defines entities, contracts, state
> machines, and integration points. Grounded in the real `schema/models/*.model.yaml` (10 models,
> ADR-001 applied 2026-06-30). Where prose contracts and the schema disagree, the **schema is
> authoritative** and the drift is flagged in §10.

## 1. Bounded context boundary

**Owned (canonical, in this module's schema):** `Account`, `CostCenter`, `Journal`, `JournalLine`,
`Ledger`, `FiscalPeriod`, `FinancialStatement`, `Reconciliation`, `ReconciliationItem`,
`AccountingPost`.

**Referenced (logical FK only — `@exclude_from_foreign_key_check`, no DB constraint), per
`shared-masters-ownership.md`:** `corporate.Company` (`company_id`), `corporate.Branch`
(`branch_id`), `corporate.Department` (`department_id`), `party.Party` (`party_id`),
`projects.Project` (`project_id`). `sapiens.User` is an **external import** (`index.model.yaml`)
used for audit actors (`created_by`, `posted_by`, `approved_by`, …).

Accounting imports **no** producer module — 0 inbound code edges (`gl-posting-contract.md §8`).

**Tenancy:** `company_id` (books owner, required, logical FK) on every root entity; `branch_id`
(dimension, nullable) where relevant; **no `provider_id`** — schema-per-tenant via DB `search_path`.

## 2. Entities & relationships

| Entity | Owns / ref | Key fields | Relations | Notes |
|--------|-----------|------------|-----------|-------|
| **Account** | owns | `company_id`, `account_number`, `account_code`, `name`/`name_en`, `account_type`, `account_subtype`, `normal_balance`, `parent_id`, `is_header`/`is_detail`, `currency`(IDR), `current_balance`, `allow_manual_entry`, `is_reconcilable`, `status` | `parent`/`children` (tree), `tax_account`, `source`/`clones`, `journal_lines`, `ledger_entries` | Chart of Accounts. `account_subtype ∈ {accounts_receivable, accounts_payable}` drives party-required (R3). Unique `(company_id, account_number)`, `(company_id, account_code)`. |
| **CostCenter** | owns | `company_id`, `code`, `name`/`name_en`, `parent_id`, `level`, `is_group`, `branch_id`(logical ref), `is_active` | `parent`/`children`, `ledgers`, `journal_lines` | Controlling dimension (ADR-001 #2). Allocation splits deferred. `is_group` node not postable. |
| **Journal** | owns | `company_id`, `journal_number`, `journal_type`, `branch_id`(ref), `transaction_date`, `posting_date`, `fiscal_period_id`, `description`, `currency`(IDR), `total_debit`/`total_credit`, `line_count`, `source`+`source_type`/`source_id`/`source_reference`, reversal fields (`is_reversed`, `reversed_by_id`, `reverses_id`, `is_reversing`), approval fields, `status` | `fiscal_period`, `lines` (cascade), `reversed_by`/`reverses`, `*_by_user` | Balanced header. `total_debit == total_credit` (R1). Unique `(company_id, journal_number)`. |
| **JournalLine** | owns | `journal_id`, `company_id`(denorm), `branch_id`(ref), **`party_type`/`party_id`**(ref, R3), `line_number`, `account_id`, `account_number`/`account_name`(denorm), `debit_amount`/`credit_amount`, `currency`/`exchange_rate`/`base_debit_amount`/`base_credit_amount`, dimensions **`cost_center_id`**/`project_id`(ref)/`department_id`(ref)/`dimensions:json`, tax fields (`is_tax_line`,`tax_rate`,`tax_base_amount`), `is_posted`/`ledger_id` | `journal`, `account`, `cost_center`, `related_line`, `reconciliation`, `ledger` | One debit **xor** one credit. **No `@audit_metadata`** → no soft-delete (cascade child of Journal). Unique `(journal_id, line_number)`. |
| **Ledger** | owns | `company_id`, `account_id`+denorm (`account_number`/`name`/`type`/`normal_balance`), `journal_id`/`journal_number`/`journal_line_id`, `transaction_date`/`posting_date`, `fiscal_period_id`/`fiscal_year`/`fiscal_month`, `debit_amount`/`credit_amount`, **`balance_before`/`balance_after`/`balance_change`**, **`sequence_number`**, `branch_id`(ref), **`party_type`/`party_id`**(ref), dimensions, reversal (`is_reversed`,`reversed_by_id`,`reverses_id`), `is_reconciled` | `account`, `journal`, `journal_line`, `fiscal_period`, `reconciliation`, `cost_center`, `reversed_by`/`reverses` | **Immutable running-balance** entry (R8/R9). AR/AP aging index on `(company_id, party_type, party_id, account_id, transaction_date)`. |
| **FiscalPeriod** | owns | `company_id`, `period_code`, `name`, `period_type`, `start_date`/`end_date`, `fiscal_year`/`fiscal_quarter`/`fiscal_month`, `parent_id`(tree), `status`, `is_current`, close fields (`closing_started_at/by`, `closed_at/by`, `locked_at/by`), `allow_adjustments`/`adjustment_deadline`, cached summaries (`total_debits`,`net_income`,`total_assets`…) | `parent`/`children`, `journals`, `statements`, `*_user` | Lifecycle R10. Unique `(company_id, period_code)`, `(company_id, fiscal_year, fiscal_month)`. |
| **FinancialStatement** | owns (read model) | `company_id`, `statement_number`, `statement_type`, `fiscal_period_id`, `as_of_date`, balance-sheet totals + `balance_check`/`balance_difference`, income-statement totals + margins, cash-flow totals, trial-balance `total_debits`/`total_credits`/`trial_balance_check`, `line_items:json`, `status`, `currency`(IDR) | `fiscal_period`, `*_by_user` | Generated statement (R11). Unique `(company_id, statement_number)`. |
| **Reconciliation** | owns | `company_id`, `reconciliation_number`, `account_id`, `reconciliation_type`, `period_start`/`end`/`statement_date`, opening/closing book & statement balances, matched/unmatched totals, outstanding items, `difference`/`is_balanced`, `status`, `adjusting_journal_ids:json` | `account`, `previous_reconciliation`, `items` (cascade), `*_user` | Unique `(company_id, reconciliation_number)`, `(company_id, account_id, statement_date)`. |
| **ReconciliationItem** | owns | `reconciliation_id`, `company_id`(denorm), `item_number`, `source`(book/statement/both), book side (`ledger_id`,`journal_id`,`book_debit`/`credit`), statement side (`statement_debit`/`credit`), `status`, `matched_with_id`/`match_method`/`match_confidence`, adjustment (`adjustment_type`,`adjustment_journal_id`), outstanding (`outstanding_type`,`expected_clear_date`) | `reconciliation`, `ledger`, `journal`, `matched_with`, `adjustment_journal` | **No `@audit_metadata`** → no soft-delete (cascade child). Unique `(reconciliation_id, item_number)`. |
| **AccountingPost** | owns (inbound port) | `company_id`, `branch_id`(ref), `source_type`, `source_id`(ref), `source_reference`, `journal_id`, `posting_type`, `posting_status`, `currency`(IDR), `total_debit`/`total_credit`, timing (`scheduled_at`,`posted_at`,`failed_at`), retry (`retry_count`,`max_retries`,`next_retry_at`), error (`error_code`,`error_message`), reversal (`reverses_post_id`,`reversed_by_post_id`), `posted_by` | `journal`, `reverses_post`, `reversed_by_post` | The seam. Unique `(source_type, source_id, posting_type, journal_id)` = idempotency key today (R5). **No `idempotency_key` / `lines[]` column** — see §10. |

**Enums (real):** `AccountType`(8), `AccountSubtype`(17, incl. `accounts_receivable`/`accounts_payable`/`bank`/`cash`/`tax`), `NormalBalance`(debit/credit), `AccountStatus`(active/inactive/archived), `PartyType`(customer/supplier/employee), `JournalType`(10), `JournalSource`(8), `JournalStatus`(draft/pending_approval/approved/rejected/posted/voided), `PeriodType`(4), `PeriodStatus`(open/closing/closed/locked/adjusting), `StatementType`(5), `StatementStatus`(6), `ReconciliationType`(5), `ReconciliationStatus`(5), `ReconciliationItemStatus`(6), `PostingSourceType`(order/payment/settlement/refund/expense/inventory/manual), `PostingType`(original/reversal/adjustment/correction), `PostingStatus`(pending/processing/posted/failed/cancelled).

## 3. State machines / workflows

### Journal lifecycle (`JournalStatus` — real enum)
```
draft ──submit──▶ pending_approval ──approve──▶ approved ──post──▶ posted ──void──▶ voided
  │                     │                                            (reversing entry)
  └──post (no approval)─┴──reject──▶ rejected
```
- **Guards:** submit requires ≥2 lines + balance (R1/R2); `pending_approval` only when `requires_approval` or `total_debit ≥ approval_threshold`; `post` requires `approved` (or `draft` when no approval) + open period (R4) + postable accounts (R6).
- **Side effects on `post`:** write `Ledger` rows + set `JournalLine.is_posted`, `ledger_id`, `posted_at`; stamp `posted_by`/`posted_at`. **`voided`** never edits a posted entry — it emits a reversing journal (R7/R8).
> Note: the PRD's shorthand "draft → approved → posted → voided" is the happy path of this 6-state machine; `pending_approval` and `rejected` are the real intermediate/branch states.

### Posting lifecycle (`PostingStatus` — real enum)
```
pending ──▶ processing ──▶ posted
   │            │
   └────────────┴──▶ failed ──(retry while retry_count < max_retries)──▶ processing
                        └──▶ cancelled
```
- Async/durable: `next_retry_at`/`retry_count`/`max_retries` (default 3) drive retry of transient failures. On terminal success `journal_id` is set. (`gl-posting-contract.md` calls the initial state "scheduled"; schema uses `pending` — §10.)

### Fiscal period lifecycle (`PeriodStatus`)
```
open ──▶ closing ──▶ closed ──▶ locked
  └──────▶ adjusting (allow_adjustments)
```
- `closed`/`locked` block new posts (R4); `adjusting` permits adjusting entries until `adjustment_deadline`.

### Reconciliation lifecycle (`ReconciliationStatus`)
```
in_progress ──▶ pending_review ──▶ reviewed ──▶ completed   (└──▶ cancelled)
```
- `completed` requires `difference = 0` (`is_balanced = true`).

## 4. Hooks (→ `*.hook.yaml`, hand-authored orchestration)

| Hook | Trigger | Condition | Action |
|------|---------|-----------|--------|
| `validate_posting` | AccountingPost received / journal submit | always | enforce R1–R6; on fail set `error_code`/`error_message`, `posting_status = failed` |
| `assemble_journal` | post validated | `posting_status = pending→processing` | create Journal + JournalLine(s) from post lines |
| `write_ledger` | journal `post` | `status → posted` | for each line, compute running balance (R9), write immutable Ledger row, set `is_posted`/`ledger_id` |
| `update_account_balance` | ledger written | always | `Account.current_balance += balance_change` |
| `emit_post_event` | posting terminal | posted/failed | publish `AccountingPostPosted` / `AccountingPostFailed` |
| `link_reversal` | reversal post | `posting_type = reversal` | set `reverses_post_id`/`reversed_by_post_id`, journal `reverses_id`/`reversed_by_id`, original `is_reversed` |
| `block_closed_period` | any post | period `closed`/`locked` | reject (R4) |

## 5. Contracts

### Inbound — `AccountingPost` (what consumers emit to drive the module)
Logical shape a producer sends (per `gl-posting-contract.md §2`); the persisted `AccountingPost`
entity records identity + status, the `lines[]` become `JournalLine`s:
```
AccountingPost {
  company_id, branch_id?,                       # books owner + dimension
  source_type: PostingSourceType,               # order|payment|settlement|refund|expense|inventory|manual
  source_id, source_reference?,                 # logical ref — NO FK to producer
  posting_type: PostingType,                    # original|reversal|adjustment|correction
  currency = 'IDR', posting_date,
  lines: [ PostingLine ],                        # >= 2, must balance
  # idempotency: today the composite (source_type, source_id, posting_type, journal_id) unique index (§10)
}
PostingLine {                                    # → becomes a JournalLine
  account_id, debit, credit,                     # one > 0, the other 0
  party_type?, party_id?,                        # REQUIRED iff account_subtype ∈ {accounts_receivable, accounts_payable}
  cost_center_id?, project_id?, department_id?, dimensions?,
  description?, exchange_rate?,
  is_tax_line?, tax_rate?, tax_base_amount?      # tax computed upstream by backbone-tax-id
}
```
**Validation on receipt:** R1 (balanced), R2 (≥2 detail lines, same company), R3 (party for AR/AP),
R4 (period open), R5 (idempotent), R6 (account postable). See `brd.md §5`.

### Outbound events (`src/exports/events.rs`, Tier A — public)
- `AccountingPostPosted { post_id, source_type, source_id, journal_id, status }`
- `AccountingPostFailed { post_id, source_type, source_id, error_code, error_message }`

No synchronous call-back into producers — status feedback is event-only (`gl-posting-contract.md §6`).

### Exported DTOs (`src/exports/types.rs`, Tier A)
- Posting **result/status** DTO (post_id, journal_id, posting_status).
- **Account lookup** DTO (resolve account by `account_code`/`account_number` → `account_id`, `account_subtype`).
- **Trial-balance / statement projection** DTOs (from `FinancialStatement`/`Ledger` aggregates).
Internal domain entities never leak.

### GL postings emitted
**None.** Accounting *receives* every `AccountingPost`; it never emits one. It is the receiving authority for the whole system.

## 6. Behavior that is NOT in schema (hand-authored)

The schema generates entities/DTOs/migrations/repos/CRUD handlers. The following imperative logic
lives in `application/service/*_custom.rs` (Tier B), each tied to its BRD golden case (the oracle):
- **Posting validation** (R1–R6) — `accounting_post_service_custom.rs`. Oracle: G1–G3, G5.
- **Journal assembly** from post lines (map `PostingLine` → `JournalLine`, denormalize account fields). Oracle: G1.
- **Ledger write with running balance** (R9): read last `sequence_number`/`balance_after` per account, compute `balance_before/change/after`, write immutable row. Oracle: G1, G7.
- **Reversal assembly** (R7/R8): build mirror journal, swap debit/credit, link posts, land in current open period. Oracle: G4.
- **Period-close routine**: closing journal (revenue/expense → retained earnings), cache period summaries. Oracle: G5, G7.
- **Statement generation** (trial balance, balance sheet, income statement): aggregate `Ledger`, compute `trial_balance_check`/`balance_check`. Oracle: G7.
- **Reconciliation matching** (auto/manual/rule) + adjusting-entry emission. Oracle: G8.
- **Retry driver** for `failed` posts (`next_retry_at`, `retry_count < max_retries`).
- **Idempotency guard** (R5) on the composite key.

Everything else (the 12 CRUD endpoints, DTOs, repositories) is generated from schema and must not be hand-edited outside `// <<< CUSTOM` markers.

## 7. Integration points (ACL — `src/integration/context_map.rs`)

| Referenced master | Local use | Mapping |
|-------------------|-----------|---------|
| `corporate.Company` | `company_id` on every root entity | logical FK; no join — resolved at DB `search_path`/ACL |
| `corporate.Branch` | `branch_id` dimension | logical FK; display-only join via ACL |
| `corporate.Department` | `department_id` dimension | logical FK |
| `party.Party` | `party_type`/`party_id` on AR/AP lines | logical FK; ACL resolves name for aging reports |
| `projects.Project` | `project_id` dimension | logical FK |
| `sapiens.User` | audit actors | external import (real DB FK within the shared identity module) |

## 8. Endpoints

**Standard 12 CRUD per owned entity** via `BackboneCrudHandler` (list, create, get/find_by_id,
update, patch, soft_delete, restore, empty_trash, bulk_create, upsert, list_deleted): for `Account`,
`CostCenter`, `Journal`, `JournalLine`*, `Ledger`, `FiscalPeriod`, `FinancialStatement`,
`Reconciliation`, `ReconciliationItem`*, `AccountingPost`.
> *`JournalLine` and `ReconciliationItem` have **no `@audit_metadata`** (cascade children,
> `no_soft_delete` repositories) — their `soft_delete`/`restore`/`empty_trash`/`list_deleted`
> endpoints are not generated; they are managed through their parent aggregate.

**Non-CRUD endpoints** (hand-authored, registered in `routes/mod.rs` outside the CRUD composition):
- `POST /accounting/posts` — **the posting endpoint**. Accepts the inbound `AccountingPost` contract,
  runs §5 validation, assembles Journal + Ledger, returns the posting result (`posting_status`,
  `journal_id`) and/or 202-accepts for async. This is the module's one behavioral (non-CRUD) route
  and the entire system's GL write path.

(Candidate future non-CRUD routes — period `close`, statement `generate`, reconciliation `match` —
are workflow actions, deferred until a consumer needs them; promote on demand.)

## 9. Indonesia overlay touchpoints

- **Never baked in.** Base COA is region-neutral; the Indonesian SAK-EMKM/PSAK COA is an `Account`
  seed selected by `Company.locale_profile` (default `id`) — not an enum (`localization-standard.md §2`).
- **Tax** — PPN/PPh are ordinary seeded `Account` rows; tax lines arrive pre-computed inside
  `AccountingPost.lines[]` from `backbone-tax-id`. Accounting invokes **no** tax logic.
- **Currency** — `currency` defaults `'IDR'` module-wide; multi-currency handled via
  `exchange_rate`/`base_*_amount` on `JournalLine`, balance invariant runs on base amounts.

## 10. Open technical questions

- **`idempotency_key` missing.** `gl-posting-contract.md §2/§3.5` requires `AccountingPost.idempotency_key`; the schema has none. Today R5 relies on the unique index `(source_type, source_id, posting_type, journal_id)`. **Decide:** add the field (schema change → migration) or ratify the composite key. Blocks nothing but should be settled before codegen.
- **`lines[]` persistence.** The inbound contract carries `lines[]`; the persisted `AccountingPost` stores only totals (`total_debit`/`total_credit`) + `journal_id` — lines live on the created `Journal`. Confirm this is the intended shape (post = header + link to journal, lines only on journal).
- **`PostingStatus` vs contract wording.** Schema: `pending|processing|posted|failed|cancelled`. Contract prose: `scheduled → posted|failed`. FSD documents the schema; reconcile the doc.
- **`PostingType.adjustment`/`correction` semantics** — schema has 4 variants; contract only exercises `original`/`reversal`. Define adjustment/correction posting behavior or defer.
- **Company/Branch/CostCenter ownership** with `backbone-corporate` (ADR-001 consequence) — must land before Tier-0 masters.
