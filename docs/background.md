<!-- Reader: evaluator (secondary: maintainer). Mode: explanation. -->

# Background & Prior Art

**The point:** `backbone-accounting` is not a greenfield general-ledger design. It was extracted from a working product, and it deliberately reacts to one specific, well-known prior system — ERPNext. It keeps what ERPNext got right (a proven double-entry GL) and rejects one structural choice: GL posting as an implicit side-effect scattered across dozens of controllers. Understanding that inheritance is the fastest way to understand why this module looks the way it does.

If you want the belief system this produced, read [Philosophy](./philosophy.md). For the concrete stack decisions, read [Technology & the Why](./technology.md). For terms, see the [Glossary](./glossary.md).

## Where it came from

The module was extracted from **BERSIHir** (the `bersihir-service` product), then hardened into a standalone bounded-context library. The extraction was not a rewrite from scratch: the schema that came out of BERSIHir — **9 core models** — was already cleaner than the system BERSIHir itself had grown up alongside.

The most consequential difference was already present at extraction time: the schema had an explicit **AccountingPost** port — a first-class boundary for "post this business event to the ledger." ERPNext has no such thing; posting is implicit. That single structural head-start is what [ADR-001](./adr/ADR-001-gl-core-boundary.md) formalizes as the GL/core boundary.

## The prior art it reacts to: ERPNext

Credit where it is due. **ERPNext is a capable, widely used ERP**, and its GL model is proven in production at scale. The core tables — `gl_entry` (the double-entry ledger line) and `journal_entry` — are sound, battle-tested, and are the direct ancestors of this module's ledger.

This module does **not** reject ERPNext's ledger. It rejects two specific things about *how ERPNext posts to that ledger*:

1. **Implicit posting.** In ERPNext, GL entries are a side-effect: each document controller calls `make_gl_entries()` from inside its own `on_submit`, replicated across **30+ controllers**. There is no single seam where "an event became ledger entries" — the posting logic is diffused into every transaction type that touches money.
2. **A collapsed bounded context.** ERPNext's "Accounts" folder is roughly **192 DocTypes**, collapsing what are really about **8 distinct bounded contexts** into one undifferentiated module.

The reaction to (1) is the explicit AccountingPost port. The reaction to (2) is treating accounting as one bounded-context module with a hard edge, rather than a catch-all.

## What it borrows

- **Double-entry `gl_entry`** — the fundamental ledger-line model is inherited, not reinvented.
- **Party on the ledger line.** ERPNext's `gl_entry.party_type` / `party` — carrying the counterparty directly on the ledger line so AR/AP aging can be computed from the ledger itself — is borrowed deliberately. This is recorded as point #1 of [ADR-001](./adr/ADR-001-gl-core-boundary.md).

## What it rejects, and why

Each rejection trades a flexible-but-diffuse ERPNext mechanism for a stricter, more explicit one that is compatible with a schema-YAML-as-source-of-truth, statically-generated codebase.

| ERPNext choice | This module instead | Why | Recorded in |
|----------------|---------------------|-----|-------------|
| Implicit posting via `make_gl_entries()` in each controller's `on_submit` | Explicit **AccountingPost** port | One seam owns "event → ledger"; posting is observable, durable, retryable | [ADR-001](./adr/ADR-001-gl-core-boundary.md) |
| Cost center as a free-text string | **CostCenter** entity tree | A dimension used in reporting deserves referential integrity, not a string | [ADR-001](./adr/ADR-001-gl-core-boundary.md) #2 |
| Dynamic runtime accounting-dimension engine that mutates table columns | **Hard-coded dimensions + a JSON escape hatch** | A runtime engine that alters schema at runtime is fundamentally incompatible with static codegen where the schema YAML is the source of truth | [ADR-001](./adr/ADR-001-gl-core-boundary.md) #3 |
| Parallel ledgers / IFRS `finance_book` shadow ledger | **Single ledger, PSAK/SAK only** | Scope is Indonesian standards; a shadow ledger is complexity without a consumer | — |
| Row-level `tenant_id` in a shared schema | **Schema-per-tenant** | Isolation at the database boundary rather than in every `WHERE` clause | [ADR-001](./adr/ADR-001-gl-core-boundary.md) #4 |

## The Metaphor framework lineage

This is a Metaphor **`module`** project. That framework choice is not incidental — it is what makes the rejections above enforceable rather than aspirational:

- **DDD 4-layer** structure (domain / application / infrastructure / presentation) gives the bounded context a real internal shape.
- **Schema-YAML as the single source of truth, with codegen** means the entity, DTOs, migration, repository, service, and HTTP surface are generated from `schema/models/*.model.yaml`. You cannot quietly grow a runtime-column-mutating dimension engine when the schema is a static, generated artifact — the framework structurally forbids it.
- **Plugins dispatched as subprocesses** keep the toolchain (codegen, schema, dev) decoupled.

The belief that "the schema is the source of truth" is only credible because the framework enforces it. That belief, and the invariants it protects, are the subject of [Philosophy](./philosophy.md).
