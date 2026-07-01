# ADR-001: GL-core boundary, posting contract, and tenancy for backbone-accounting

**Status**: Accepted — **Applied to schema 2026-06-30** (validated; not yet code-generated)
**Date**: 2026-06-30
**Deciders**: Farid (owner), council (root:leverage run, 2026-06-30)
**Context doc**: `docs/erp/financials.md` (workspace root)

## Context

`backbone-accounting` is the GL core (ledger of record) for the Indonesia-first ERP product being
decomposed from ERPNext. It is the hub of the Financials pillar: billing, payments, banking,
assets and (later) payroll all post into it. Before any of those modules bind to it, the GL-core
boundary, its inbound posting contract, and the tenancy model must be fixed — otherwise every
downstream module inherits the boundary mistakes.

Field-level comparison against ERPNext's `gl_entry` / `journal_entry` surfaced four decisions. The
existing schema (9 models: Account, Journal, JournalLine, Ledger, AccountingPost, FiscalPeriod,
FinancialStatement, Reconciliation, ReconciliationItem) was extracted from BERSIHir and is cleaner
than ERPNext — notably it already has an explicit `AccountingPost` port, which ERPNext lacks
(ERPNext bakes posting into each controller's `on_submit`, the source of its monolith coupling).

## Decision

### Posting contract (the seam)
`AccountingPost` is the **only** inbound port for GL posting. Any module emits
`AccountingPost{ source_type, source_id, source_reference, company_id, branch_id, lines[], idempotency }`;
accounting validates double-entry (Σdebit = Σcredit), writes `Journal` + `Ledger`, manages reversal
and async status, and reports back. Accounting **never imports** an emitting module; `source_*` is a
logical reference (ACL boundary). Full spec to be written as `docs/erp/gl-posting-contract.md`.

### 1. Party on the ledger line — ADD
Add `party_type` (enum: customer | supplier | employee) and `party_id` (logical FK to
`party.Party`/`crm`, no DB constraint) to `Ledger` and `JournalLine`. Required **iff** the line's
account has `account_subtype ∈ {receivable, payable}`.
*Rationale*: AR/AP subledger aging, statements, and dunning need party-level balances read directly
from the GL — independent of the source document (manual journals, one payment settling many
invoices). Mirrors ERPNext `gl_entry.party_type/party`.

### 2. Cost Center — PROMOTE to an entity
Add a `CostCenter` entity (tree: `parent_id`, `is_group`, `company_id`) owned by
`backbone-accounting`. Replace the `cost_center` *string* on `JournalLine` with `cost_center_id`.
*Rationale*: cost center is a controlling/accounting dimension (segment P&L, budget-vs-actual).
Percentage-split allocation is **deferred** (SAP-tier complexity, not needed for SMB).

### 3. Accounting Dimension — hard-coded set + JSON escape hatch
Keep explicit `cost_center_id`, `project_id`, `department_id` on the ledger line; add
`dimensions: json?` for forward-compat.
*Rationale*: ERPNext's dynamic accounting-dimension engine alters table columns at runtime —
fundamentally incompatible with Metaphor's static schema-codegen SSoT. Hard-coded dimensions are
type-safe and indexable; an Indonesia SMB does not need arbitrary user-defined dimensions.

### 4. Tenancy and org generalized
- `outlet_id` → `branch_id` (logical FK to `corporate.Branch`) — a dimension, not the books owner.
- Add `company_id` — the legal entity that owns the books (CoA, fiscal periods, statements are
  per-company; this was implicitly `provider_id`).
- **Drop `provider_id` from all domain models.** Multi-tenancy is **schema-per-tenant**: each
  tenant gets its own Postgres schema (`search_path`); isolation is a DB-layer concern.

## Alternatives considered

- *Derive AR/AP party from `source_id`* — rejected: breaks for manual journals and multi-invoice
  settlements.
- *Cost center as a string* — rejected: no tree, no budget linkage.
- *ERPNext-style dynamic dimensions* — rejected: incompatible with codegen SSoT.
- *Row-level `tenant_id`* — rejected in favor of schema-per-tenant (cleaner domain, DB-level
  isolation; matches the schema-per-module direction).

## Consequences

- ✅ AR/AP aging is a first-class GL capability, source-independent.
- ✅ Domain models are tenant-agnostic — no `provider_id` noise; isolation is infra.
- ✅ `AccountingPost` keeps all Financials modules decoupled from the GL.
- ⚠️ `party_id` is a cross-module logical FK — needs the Extension Contract's versioning rule.
- ⚠️ DB connection layer must set `search_path` per tenant; migrations run per schema.
- ⚠️ `CostCenter` and `Company`/`Branch` ownership must be settled with `backbone-corporate`
  (Foundation) before Tier-0 masters land.

## Implementation

**Applied 2026-06-30** (`metaphor schema schema validate` → all schemas valid):
- `provider_id` dropped and `company_id` (required, logical FK to `corporate.Company`) added across
  **all 8 models** — not just the four originally scoped, but also `fiscal`, `reconciliation`,
  `financial_statement` (schema-per-tenant must be consistent module-wide). `outlet_id` → `branch_id`
  on account/journal/ledger/accounting_post. All per-tenant uniqueness indexes re-keyed to `company_id`.
- `PartyType` enum added (`account.model.yaml`); `party_type` + `party_id` (logical FK) added to
  `Ledger` and `JournalLine` with AR/AP aging indexes.
- New `CostCenter` entity (`cost_center.model.yaml`, registered in `index.model.yaml`); `cost_center`
  string → `cost_center_id` (FK) on `Ledger`/`JournalLine`, with `cost_center` relations.
- `project`/`department` strings → `project_id`/`department_id` (logical FK) + `dimensions: json?`.

**Not yet done (next):** code generation (`metaphor schema schema generate`), migration, service/
handler wiring, and `module.rs` registration of the new `CostCenter` service — to follow the
CLAUDE.md golden path. No codegen until this point per spec-first/codegen-gated strategy.
