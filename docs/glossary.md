# Glossary — Ubiquitous Language

> Reader: **all four** (evaluator, app developer, maintainer, contributor). Mode: **reference**.
> One term, one meaning. Every other handbook page defers to this file. If a term you need is
> missing, add it here rather than coining a synonym elsewhere. Terms are grounded in the schema
> SSoT (`schema/models/*.model.yaml`) and the [BRD](./brd.md) / [FSD](./fsd.md).

## Domain entities

| Term | Definition |
|------|------------|
| **Account** | A Chart-of-Accounts entry. Classified by `account_type` (asset, liability, equity, revenue, expense, cogs, other_income, other_expense), `account_subtype` (e.g. `accounts_receivable`, `accounts_payable`, `bank`, `cash`, `tax`), and `normal_balance` (debit or credit). A **detail** account (`is_detail = true`) can be posted to; a **header** account (`is_header = true`) cannot. |
| **CostCenter** | A controlling dimension organized as a tree (`parent_id`, `is_group`). A `is_group` node is not postable. Replaces the free-text `cost_center` string (see [ADR-001](./adr/ADR-001-gl-core-boundary.md) #2). Percentage-split allocation is out of scope. |
| **Journal** | A balanced double-entry transaction: one header plus ≥2 lines. `total_debit` must equal `total_credit`. Carries an approval → posting lifecycle. |
| **JournalLine** | One debit **xor** one credit against one Account. A cascade child of Journal with no soft-delete of its own. Carries party and dimension fields. |
| **Ledger** | The immutable general-ledger entry with a **running balance** per account (`balance_before` → `balance_change` → `balance_after`) ordered by a monotonic `sequence_number`. Never edited in place except its reversal-link flags. |
| **AccountingPost** | The single inbound posting request from a producing module — the record of *why* a Journal exists (`source_type`, `source_id`), its `posting_status`, retries, and reversal links. See **Posting port**. |
| **FiscalPeriod** | An accounting period (`monthly`, `quarterly`, `yearly`, `custom`) with a lifecycle (`open → closing → closed → locked`, plus `adjusting`). Posting is only allowed into an `open` (or `adjusting`) period. |
| **FinancialStatement** | A generated read model: balance sheet, income statement, trial balance, cash flow, or equity. Carries `trial_balance_check` and `balance_check`. |
| **Reconciliation** | The act and record of matching a `bank`/reconcilable Account's book balance against an external statement, driving `difference` to 0. |
| **ReconciliationItem** | A single book-side or statement-side line within a Reconciliation; matched, classified as outstanding, or turned into an adjusting entry. A cascade child with no soft-delete of its own. |

## Referenced (owned by other modules — logical FK, no DB constraint)

| Term | Owner | Local use |
|------|-------|-----------|
| **Company** (`company_id`) | `corporate` | The legal entity that owns the books; on every root entity. See **Books owner**. |
| **Branch** (`branch_id`) | `corporate` | A dimension, not the books owner. |
| **Department** (`department_id`) | `corporate` | A dimension on a line. |
| **Party** (`party_type`/`party_id`) | `party` | The customer/supplier/employee an AR/AP balance belongs to. |
| **Project** (`project_id`) | `projects` | A dimension on a line. |
| **User** | `sapiens` | Audit actors (`created_by`, `posted_by`, …). The one **external import** with a real DB FK. |

## Concepts

| Term | Definition |
|------|------------|
| **Posting port / the seam** | `AccountingPost` is the **only** inbound port for GL posting. Every producing module (billing, payments, banking, …) records financial effects by emitting an `AccountingPost`; none of them import accounting's internals, and accounting imports none of them. |
| **Books owner** | The `company_id` — the legal entity whose Chart of Accounts, fiscal periods, and statements these are. All per-tenant uniqueness is keyed on it. |
| **Double-entry invariant** | For every journal/post, Σ`debit` = Σ`credit` in base currency (rule **R1**). The load-bearing invariant of the whole module. |
| **Running balance** | Each Ledger row records `balance_before`, `balance_change`, `balance_after`, and a per-account monotonic `sequence_number` (rule **R9**). |
| **Reversal-not-edit** | To undo a posted entry you emit a **reversal** (a mirror journal with debit/credit swapped) that lands in the *current open period*; the original Ledger row is never mutated (rules **R7**/**R8**). |
| **Immutable ledger** | A posted Ledger row is never updated except its reversal-link flags (`is_reversed`, `reversed_by_id`). |
| **Idempotency** | A repeat of a post does not double-write. Enforced by a **partial unique index** `(company_id, source_type, source_id, posting_type) WHERE posting_status='posted'` (see [ADR-002](./adr/ADR-002-ledger-write-path-integrity.md)). |
| **Guarded routes** | `create_guarded_accounting_routes` mounts the posted GL entities (Journal, JournalLine, Ledger, AccountingPost) **read-only**; the only sanctioned writer is the posting path. Master/config entities keep full CRUD. |
| **Golden case (G1–G8)** | A numeric acceptance scenario in the [BRD](./brd.md) that serves as the BDD oracle; each maps to a `.feature` file and a test under `tests/`. |
| **Business rule (R1–R11)** | The catalog of GL invariants in [BRD §5](./brd.md). Referenced throughout the handbook by number. |
| **Schema SSoT** | `schema/models/*.model.yaml` is the single source of truth. Entities, DTOs, migrations, repositories, and CRUD handlers are generated from it. |
| **CUSTOM marker** | `// <<< CUSTOM … // END CUSTOM` — the only region of a generated file that survives regeneration. Custom logic otherwise lives in a sibling `*_custom.rs`. |
| **Tier A / B / C** | Stability tiers from the [Extension Guide](./extension-guide.md): **A** public & versioned (posting contract, events, exported DTOs); **B** supported-but-coupled (your own `*_custom.rs`); **C** internal, never depend on (accounting's entities, repos, CRUD-as-write-path). |
| **Schema-per-tenant** | Tenant isolation is a DB `search_path` concern — each tenant gets its own Postgres schema. Domain models carry no `provider_id`/`tenant_id` ([ADR-001](./adr/ADR-001-gl-core-boundary.md) #4). |

## CLI & tooling

| Term | Definition |
|------|------------|
| **`metaphor`** | The canonical workspace CLI (v0.2.0). Prefer it over raw `cargo`/`sqlx`. Schema codegen is the nested passthrough `metaphor schema schema <op>`. |
| **`backbone`** | A local dev **alias** for the framework's own CLI (`cargo run -p backbone-cli -- …`). Older docs and the schema header reference it; the canonical entry point is `metaphor`. |
| **`BackboneCrudHandler`** | The framework component that auto-wires the 12 standard CRUD endpoints per entity. |
