# Architecture Decision Records

> Reader: **maintainer**. Mode: **reference**.
> One record per architecturally significant decision: context, decision, status, consequences.
> **Accepted ADRs are immutable** — to change a decision, write a new ADR that supersedes the old one
> rather than editing it. See the [Contribution Guide](../contributing.md#when-to-write-an-adr) for
> when a change needs an ADR.

## Index

| ADR | Title | Status | Date | Supersedes |
|-----|-------|--------|------|------------|
| [001](./ADR-001-gl-core-boundary.md) | GL-core boundary, posting contract, and tenancy | Accepted — applied to schema 2026-06-30 | 2026-06-30 | — |
| [002](./ADR-002-ledger-write-path-integrity.md) | Ledger write-path integrity (idempotency constraint + guarded CRUD) | Accepted — applied 2026-07-01 | 2026-07-01 | extends 001 |

## What each one settled

- **[ADR-001](./ADR-001-gl-core-boundary.md)** fixes the bounded-context boundary before any module
  binds to it: `AccountingPost` is the single inbound posting port; party (`party_type`/`party_id`)
  is added to the ledger line for source-independent AR/AP aging; `CostCenter` is promoted to a tree
  entity; accounting dimensions are a hard-coded set plus a JSON escape hatch (no runtime column
  mutation); tenancy is **schema-per-tenant** with `company_id` as the books owner and `provider_id`
  dropped module-wide.

- **[ADR-002](./ADR-002-ledger-write-path-integrity.md)** closes two ways the ledger's core invariant
  was violable through the module's *own* generated surface: it replaces the racy idempotency index
  with a **partial unique index** (`WHERE posting_status='posted'`) enforced at the DB layer, and it
  mounts the posted GL entities **read-only** via `create_guarded_accounting_routes` so the only
  sanctioned GL writer is `POST /accounting/posts`. Both fixes are in the schema SSoT / route
  composition, so regeneration reproduces them.

## Open items tracked by the ADRs

- `idempotency_key` field vs. the composite-key contract (see the [PRD §9](../prd.md) /
  [FSD §10](../fsd.md) open questions; ADR-002 ratified the composite key with a partial index).
- `Company`/`Branch`/`CostCenter` ownership must be settled with `backbone-corporate` before Tier-0
  masters land (ADR-001 consequence).
- ADR-002 **parking lot** (deferred, out of the maturity lens): FX base-currency correctness,
  period-close sequencing, Income Summary / re-open path, richer event payloads and delivery
  guarantees, cash-flow statement, statement snapshotting, and authorization on the posting endpoint.

## Writing a new ADR

Copy the shape of an existing record: **Status**, **Date**, **Deciders**, **Context**, **Decision**,
**Alternatives considered**, **Consequences** (✅ / ⚠️), and an **Implementation** note. Number it
sequentially (`ADR-003-…`). Keep it to one decision.
