# ADR-0025: Reconciliation graph — partial edges + full groups replace invoice-level settlement bookkeeping

**Status**: Accepted — **Applied 2026-08-19**
**Deciders**: Farid (owner)
**Cites**: AF12, TF13 (Odoo extraction, `docs/odoo/accounting/accounting-business-logic.md`,
`_extraction_tax_business_logic.md`); extends ADR-0008 (company RLS fence), ADR-0014 (fence stanza
in create migrations), ADR-0011 (host auth/role contract)

## Context

Before this ADR, "how much of this invoice is settled" was answered four different ways, none of
them a graph:

- **billing** kept a numeric `outstanding_amount` drawdown/restore on the invoice row
  (`InvoiceSettlementRepository`), maintained by the settle/reverse verbs.
- **payment** carried a `reverse_settlement` event spine that knew how to unwind a settlement but
  not how to represent partial state.
- **banking** kept bounded clearances (a clearance knows how much of a payment it matched).
- **accounting** shipped an ERPNext-shaped `reconciliations` worksheet
  (`BankReconciliationService`) that greedily matched statement lines against `ledgers` rows by
  exact signed amount and flagged rows — a report, not a ledger structure.

The consequences of that shape: no partial state existed anywhere as a first-class edge, residual
("what does this A/R line still owe?") was re-derived per consumer and drifted, exchange-rate
differences had no home at all, and un-reconciling meant restoring numbers on an invoice row with
no way to reverse the moves a settlement had generated.

## Decision

### The graph (Odoo's shape)

Two tables, both company-fenced (ADR-0014 stanza inlined in their create migrations — verified by
`tests/rls_probe.rs`):

- **`partial_reconciles`** — the edges. One row = a partial application of `amount` (company
  currency, `> 0`) between a debit journal line and a credit journal line (`debit_move_id` /
  `credit_move_id → journal_lines`, `CHECK` distinct), carrying `origin`
  (settlement | clearing | manual), `max_date` (max of the two journals' `transaction_date` — the
  aging driver), forward-compat `*_amount_currency` columns, and `exchange_move_id` — the AF12
  pointer to any move the edge generated.
- **`full_reconciles`** — the groups. One row per fully-reconciled matching group, holding
  `exchange_total` and `reconciled_at`.

Two things are deliberately **not** columns:

- **Residual is computed, never stored.** A line's residual is
  `±(line base amount) − Σ(partial amounts touching it)`, read through the
  `(company, debit_move_id)` / `(company, credit_move_id)` indexes (`residuals_of`,
  `residuals_for_party` — the A/R-A/P aging read, which lists only lines with residual > 0).
  A stored residual would be a second owner of one quantity — exactly the drift class this module
  already guards against elsewhere (the billing `outstanding_amount` cache is tolerated *only* as
  a cache, probed equal to `grand_total − Σ partials`). Billing keeps its cache; the graph is the
  authoritative read.
- **The matching number is a read, not a column.** Odoo persists `matching_number` on every line of
  a group; we derive it: union-find over the edges, keyed by the group's `min(partial_id)` and
  rendered `P<n>`. Fully-reconciled groups store `full_reconcile_id` on their lines (the FK index
  serves that read directly); partial-only chains resolve through a bounded recursive CTE. A line's
  group label therefore cannot disagree with the edges, because it *is* the edges.

### The write path (`reconcile_write_service.rs`)

`reconcile_pair_on(conn, …)` — one transaction on the caller's connection:

1. Resolve both locators (`source_type` + `source_id` + `account_id`, optionally `reversing`) to
   journal lines; unresolvable ⇒ `line_not_found`.
2. Guards (pure, `reconcile_rules.rs`, mirroring `posting_rules.rs` — service-level; only the two
   CHECKs ride SQL, no triggers): same company; same account both lines; account
   `is_reconcilable`; opposite directions; both lines posted under posted journals;
   settlement-dimension-bound (party accounts ⇒ same party both lines; non-party accounts ⇒ no
   party constraint); period-open for the exchange-move date.
3. **CLAMP**: `applied = min(requested, residual_debit, residual_credit)`. A clamp to zero is a
   no-op — an over-payment stays as the payment line's own residual (on-account credit), never an
   edge that over-applies.
4. Insert the partial edge.
5. **Exchange difference** — when the two lines' exchange rates differ and the application leaves
   one side's residual zero, post the difference to the configured FX account
   (`source_type='reconciliation'`, `source_id=partial_id`,
   `idempotency_key='exch:<partial_id>'`; fail-closed `ExchangeAccountUnconfigured`) and stamp
   `exchange_move_id`.
6. Recompute residuals; **both zero ⇒ create the full group**, bulk-set `full_reconcile_id` /
   `is_reconciled` / `reconciled_at` on every connected zero-residual line, link the group's
   partials.
7. **Reverse-then-reconcile pairing**: when a line's residual returns to full and a same-source
   `is_reversing` counterpart exists on the same account, pair them automatically — Odoo's rule
   that structurally closes the N-asynchronous-unlinks ordering hazard.

### Unreconciliation is side-effecting (AF12 / TF13)

`unreconcile_*` **never plain-DELETEs an edge.** It reverses every move the edge generated — the
`exchange_move_id` fast path, plus any journal with `source_type='reconciliation' AND
source_id=<partial_id>` (this sweep is the hook cash-basis tax moves will ride, TF13's port) — by
posting reversals on the same connection, then deletes the partials, repairs the groups (dissolve
emptied ones, keep ones still holding ≥ 2 partials), and clears the line flags. The golden case
proves it end-to-end: unlinking a partial that generated an exchange move leaves FX and the
affected account at exactly zero across the generated + reversal journals, and restores both
original residuals.

### How producers reach the graph

No finance module carries a normal Cargo edge into accounting (dev-dependency only, by design).
The graph is therefore exposed through the **`ReconcileSink` port in `backbone-gl-posting`**
(the `GlPostSink` pattern): conn-taking verbs (`reconcile_pair_on` / `unreconcile_pair_on`), so
the edge commits atomically inside the caller's unit of work — a settlement, a payment reversal, a
banking clearance. The host implements the sink over this module's `ReconcileWriteService`.

### HTTP surface

Three verb routes only — `POST /accounting/reconcile`, `POST /accounting/unreconcile/:partial_id`,
`GET /accounting/reconciliation-groups/:line_id`. The graph tables carry **zero CRUD routes**
(`tests/integrity_probes.rs` asserts 404 on GET+POST for both collections), so the only writes are
the guarded verbs.

### Deprecation

The ERPNext-shaped `reconciliations` worksheet (`BankReconciliationService`,
`POST /accounting/reconcile`'s older namesake) is **deprecated in documentation**. It writes
`is_reconciled`/`reconciliation_id` onto `ledgers` rows — disjoint from the graph — so it keeps
working, but new integrations must use the graph. Its route path now collides with the graph verb:
the guarded composition mounts only the graph verb, and a host mounting both
(`create_bank_reconciliation_routes` + `create_guarded_accounting_routes`) panics at boot. The
drop decision (and statement-side/bank-rec convergence onto the graph) is deferred to the pass
that follows this one.

## Consequences

- **Positive:** partial settlement is a first-class ledger structure; residual has one owner (the
  computed read); exchange differences have a home and an exactly-offsetting unlink; un-reconcile
  reverses rather than orphans; guards are pure and testable; the fence holds (probe); producers
  integrate without a dependency edge inversion.
- **Negative:** the matching-group read is a recursive CTE on partial-only chains (bounded by edge
  count; fully-reconciled groups — the common end state — are index reads); unlink posts reversal
  journals, so it is not cheap (that is the point of AF12); hosts must configure an FX account
  before any multi-currency reconciliation or fail closed.
- **Revisit trigger:** if cash-basis tax (CABA) lands, its moves must stamp
  `source_type='reconciliation' + source_id=<partial_id>` so the unlink sweep reverses them —
  anything else orphans CABA moves, the exact port bug TF13 warns about.
