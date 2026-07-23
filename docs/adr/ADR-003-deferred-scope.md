# ADR-003: Deferred scope — single-currency GL, and what the module will NOT own at v0.3

**Status**: Accepted — **Applied 2026-07-23**
**Deciders**: Farid (owner), council (module:backbone-accounting, focus=maturity, 2026-07-23)
**Supersedes/extends**: ADR-001 (GL-core boundary)

## Context

The 2026-07-23 maturity council ([docs/council/2026-07-23-module-backbone-accounting-maturity.md](../council/2026-07-23-module-backbone-accounting-maturity.md))
found that several pieces of "missing" accounting scope were implicit gaps with no stated posture —
producers integrating against the GL had no way to know what the module does and does not own. Two
concerns in particular were load-bearing:

1. **The GL is single-currency.** `PostingService::post` copies `debit → base_debit_amount`
   verbatim; there is no FX conversion. A producer posting in a non-base currency would silently
   store an unconverted amount, which is a correctness hazard, not a feature gap. The
   `base_*_amount` columns implied a conversion that never happens.
2. **Several classic accounting surfaces are absent** — fixed-asset register + depreciation,
   budgeting, AR/AP aging — with no record of whether they are deferred, out of scope, or someone
   else's job.

## Decision

At v0.3, `backbone-accounting` owns **one bounded context: the general ledger** (chart of accounts,
double-entry posting, immutable ledger, fiscal periods, statements, reconciliation). Everything
outside that is explicitly **not** this module:

1. **Single-currency (base = IDR) by design.** Producers MUST convert to the company's base currency
   before emitting an `AccountingPost`. The GL stores `base_debit_amount`/`base_credit_amount` equal
   to `debit`/`credit` today; the columns are retained so a future FX layer can populate them
   without a schema change, but until then they are **not** a conversion — they are a copy (the
   posting code is annotated to say so).
2. **FX / multi-currency revaluation** — producer-owned (convert before posting) or a future
   sibling module. Not built here.
3. **Fixed-asset register + depreciation** — a future sibling asset module, not this crate.
4. **Budgeting** — a future sibling budget module.
5. **AR/AP aging** — a read-model or a sibling AR/AP module; party balances are queryable off the
   ledger today, but the aging *report* is not provided by this crate.

## Consequences

- **Positive:** the module's contract is now explicit. Producers know the GL is single-currency and
  where each deferred concern lives. The `base_*_amount` columns stop being a silent lie.
- **Negative:** a downstream service needing multi-currency or aging must build it (or wait for the
  sibling module). A future FX layer that wants to populate `base_*_amount` differently must touch
  the posting write path — but the columns are already there, so no migration is required.
- **Revisit trigger:** a concrete producer blocked by any of the deferred items in a given month.
