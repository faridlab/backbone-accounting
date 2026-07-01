# Philosophy & Motivation

> Reader: **evaluator** — deciding whether to adopt or trust this module. Mode: **explanation**.
> This is the north star that explains every later trade-off. If a design decision elsewhere in the
> handbook surprises you, the reason is almost always here.

## The one-sentence version

`backbone-accounting` is the **General Ledger core**: the ledger of record for an Indonesia-first
ERP, with exactly one inbound door for posting and an append-only ledger that can never be edited in
place.

## The problem it exists to kill

In ERPNext — the system this product is decomposed from — General-Ledger posting is an **implicit
side-effect**. Every controller reaches directly into the `gl_entry` table on submit:
`sales_invoice.on_submit → make_gl_entries()`, purchase invoice, stock entry, payroll, and 30-plus
others. That direct coupling *is* the monolith. You cannot change one document's accounting without
risking every other document's accounting, and you cannot reason about — or test, or replace — the
ledger in isolation. An Indonesian SMB inheriting ERPNext inherits a 192-DocType "Accounts" folder
that is really about eight bounded contexts collapsed into one.

This module replaces that implicit `make_gl_entries()` side-effect with **one explicit, decoupled
seam**: the [`AccountingPost`](./glossary.md#concepts) inbound port.

## The worldview

Four beliefs drive every decision in this codebase.

1. **The ledger of record has exactly one owner.** One ubiquitous language (accounts, journals,
   debits/credits, periods), one invariant (Σdebit = Σcredit), one team's mental model. Folding the
   GL into billing or payments re-creates the ERPNext coupling. So it is its own bounded context
   with a single documented port. See [Architecture](./architecture.md).

2. **Posting is a contract, not a function call.** Producers (billing, payments, banking, inventory,
   assets, later payroll) do not import accounting and do not call its services. They **emit an
   `AccountingPost`**; accounting validates, writes the journal and ledger, and reports back via
   events. Accounting imports *no* producer — zero inbound code edges. This is the single most
   important structural rule in the module. See the [Extension Guide](./extension-guide.md).

3. **The past is immutable.** A posted [`Ledger`](./glossary.md#domain-entities) row is never edited.
   To undo, you emit a **reversal** — a mirror journal with debit and credit swapped — that lands in
   the *current* open period. History is a fact, not a mutable cell. This is enforced not just in the
   service layer but through the shipped HTTP surface via [guarded routes](./glossary.md#concepts)
   ([ADR-002](./adr/ADR-002-ledger-write-path-integrity.md)).

4. **The schema is the source of truth.** Entities, DTOs, migrations, repositories, and the 12 CRUD
   endpoints are **generated** from `schema/models/*.model.yaml`. Humans write only the imperative
   business logic that the schema cannot express — inside regeneration-safe markers. See the
   [Maintainer Guide](./maintainer-guide.md).

## What it deliberately does **not** do

Honest non-goals are what make the boundary trustworthy. This module refuses to:

- **Compute tax.** PPN/PPh are computed upstream by `backbone-tax-id` and arrive as ordinary lines
  inside an `AccountingPost`. Accounting *stores* tax accounts; it runs no tax logic.
- **Originate business documents.** It has no invoices, no orders, no payments. Those live in
  `backbone-billing`, `backbone-payments`, etc. Accounting holds party-level balances, not documents.
- **Emit postings.** It is the *receiving* authority for the whole system. It never emits an
  `AccountingPost`; it only receives them.
- **Keep parallel ledgers.** One ledger, PSAK/SAK only — no IFRS `finance_book` shadow ledger.
- **Run a dynamic accounting-dimension engine.** ERPNext mutates table columns at runtime; that is
  fundamentally incompatible with a static schema-codegen SSoT. Dimensions here are a hard-coded set
  (`cost_center_id`, `project_id`, `department_id`) plus a `dimensions` JSON escape hatch
  ([ADR-001](./adr/ADR-001-gl-core-boundary.md) #3).
- **Bake in Indonesia.** The core is region-neutral. Indonesian behavior (the SAK-EMKM/PSAK chart of
  accounts, IDR default, tax accounts) arrives as a **data seed** selected by
  `Company.locale_profile`, never as base enums. Remove the overlay seed and a clean region-neutral
  GL still runs.

## Who this is for

| You are… | Start at |
|----------|----------|
| Deciding whether to adopt or trust this | [Background & prior art](./background.md), then [Technology](./technology.md) |
| Building a module that posts to the GL | [Developer Guide](./developer-guide.md) → [Extension Guide](./extension-guide.md) |
| Extending or maintaining this module | [Architecture](./architecture.md) → [Maintainer Guide](./maintainer-guide.md) |
| Opening a PR | [Contributing](./contributing.md) |

## The test of success

The module is "done" when the [golden cases](./glossary.md#concepts) G1–G8 (the BDD oracle in the
[BRD](./brd.md)) pass, **and** the first cross-module post (`backbone-billing` → accounting) plus a
consumer's custom rule both survive regeneration of both modules with the consumer's logic intact.
That last clause — surviving regeneration — is the whole point of belief #4.
