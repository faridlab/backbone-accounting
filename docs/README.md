# Accounting Handbook

The documentation set for **`backbone-accounting`** — the General Ledger core of an Indonesia-first
ERP. This is the ledger of record: one inbound door for posting, an append-only ledger, and a schema
that is the single source of truth for the code.

New here? Read the [Philosophy](./philosophy.md) first — it explains every trade-off the rest of the
handbook makes.

## Find your path

| You are a… | Read, in order |
|------------|----------------|
| **Evaluator** — deciding whether to adopt or trust it | [Philosophy](./philosophy.md) → [Background & prior art](./background.md) → [Technology & the why](./technology.md) → [Architecture](./architecture.md) |
| **App developer** — consuming or driving the GL | [Developer Guide](./developer-guide.md) → [Extension Guide](./extension-guide.md) → [Glossary](./glossary.md) |
| **Maintainer** — extending this module | [Architecture](./architecture.md) → [Maintainer Guide](./maintainer-guide.md) → [FSD](./fsd.md) → [ADRs](./adr/README.md) |
| **Contributor** — opening a PR | [Contributing](./contributing.md) → [Maintainer Guide](./maintainer-guide.md) |

## The handbook

### Understand (explanation)
- [**Philosophy & motivation**](./philosophy.md) — what problem, what worldview, honest non-goals.
- [**Background & prior art**](./background.md) — extracted from BERSIHir; what it borrows from and rejects in ERPNext.
- [**Technology & the why**](./technology.md) — the stack, each choice with a rejected alternative.
- [**Architecture**](./architecture.md) — C4 context → containers → components → the posting data/control flow.

### Build (tutorial + how-to)
- [**Developer Guide**](./developer-guide.md) — install → quickstart → recipes → configuration → troubleshooting.
- [**Extension Guide**](./extension-guide.md) — how to drive/extend the GL from another module without breaking it (the stability tiers, the posting contract in depth).
- [**Maintainer Guide**](./maintainer-guide.md) — the schema-SSoT regeneration loop, `// <<< CUSTOM` markers, add-a-feature walkthrough, guarding the ledger.

### Contribute
- [**Contribution Guide**](./contributing.md) — dev setup, conventional commits (no signatures), tests, PR checklist.

### Reference
- [**Glossary**](./glossary.md) — the ubiquitous language; one term, one meaning.
- [**ADRs**](./adr/README.md) — accepted architecture decisions (immutable; superseded, not edited).
- [**API (OpenAPI)**](./openapi/) — generated REST reference.

### Product & specification
- [**BRD**](./brd.md) — business flows, the R1–R11 rule catalog, and the G1–G8 golden-case oracle.
- [**PRD**](./prd.md) — why the module exists and what it must do.
- [**FSD**](./fsd.md) — entities, contracts, state machines; where schema and prose disagree, schema wins.
- [**Business flows**](./business-flows/) — per-feature flow write-ups.

## What this module is (in one screen)

- A **library crate** (no `main.rs`) linked into a backend-service. Build it with
  `AccountingModule::builder().with_database(pool).build()?`; mount `create_guarded_accounting_routes(&module)`.
- **Ten entities:** Account, CostCenter, Journal, JournalLine, Ledger, FiscalPeriod,
  FinancialStatement, Reconciliation, ReconciliationItem, AccountingPost. See the [Glossary](./glossary.md).
- **One write path:** `POST /accounting/posts`. Posted GL entities are mounted **read-only**
  ([ADR-002](./adr/ADR-002-ledger-write-path-integrity.md)). Everything else gets the standard
  12 CRUD endpoints via `BackboneCrudHandler`.
- **Schema is the source of truth:** `schema/models/*.model.yaml`. Regeneration overwrites everything
  outside `// <<< CUSTOM` markers. See the [Maintainer Guide](./maintainer-guide.md).

## Regenerating & building

```bash
metaphor schema schema validate                          # validate the schema SSoT
metaphor schema schema generate accounting --target all  # regenerate code from schema
metaphor migration create <name>                         # new migration
metaphor dev test                                        # run tests
metaphor routes                                           # list the HTTP surface
```

> The canonical CLI is **`metaphor`** (v0.2.0). The `backbone` name in some older docs and the schema
> header is a local dev alias for the framework CLI — prefer `metaphor`.

## Doc conventions

Each page names its **reader** and **Diátaxis mode** at the top. Terms defer to the
[Glossary](./glossary.md). When code and a doc disagree, **code wins** — fix the doc and flag the
drift. Accepted [ADRs](./adr/README.md) are immutable; supersede rather than edit.
