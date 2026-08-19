# Reconciliation Graph — Business Flow + Golden Cases

> Owning module: `backbone-accounting`. Implemented in `reconcile_write_service.rs` /
> `reconcile_rules.rs` / `reconcile_graph_repository.rs`; proven by
> `tests/reconciliation_graph_golden_cases.rs`. Design record: [ADR-0025](../adr/ADR-0025-reconciliation-graph.md).

Replaces invoice-level settlement bookkeeping: a partial settlement, a payment reversal unwind, and
a banking clearance all become **edges in one graph** over journal lines, and "what does this
document still owe?" becomes a computed read instead of a number each module maintains.

## The model

- A **partial reconcile** is an edge applying `amount` (company currency) between one debit journal
  line and one credit journal line on the *same* reconcilable account — invoice A/R meets payment
  A/R, payable meets payment, clearing meets clearing. Origin: `settlement` (billing seam),
  `clearing` (banking seam), or `manual` (accountant verb).
- A **line's residual** is `±(line amount) − Σ(edge amounts)` — always computed, never stored
  (`residuals_for_party` is the A/R-A/P aging read and lists only lines with residual > 0).
- The **matching group** is the connected component of lines under the edges; its label `P<n>` is
  derived from `min(partial_id)`. When every line's residual hits zero the group gains a
  `full_reconciles` row and every line is stamped `full_reconcile_id` + `is_reconciled` +
  `reconciled_at`.
- **On-account**: an over-payment is never forced through — the clamp leaves the surplus as the
  payment line's own residual (credit sitting on the account), unreconciled by design.

## The verbs

- `POST /accounting/reconcile` — pair two lines (by `source_type`/`source_id`/`account_id`
  locators), optionally capped at an `amount`.
- `POST /accounting/unreconcile/:partial_id` — remove one edge, **side-effecting**: every move the
  edge generated (exchange difference today, cash-basis tax when it lands) is reversed first, then
  the edge goes, then the group is repaired. Never a plain delete (AF12/TF13).
- `GET /accounting/reconciliation-groups/:line_id` — the matching group + residuals.

Guards (distinct error codes, pure rules): lines must exist; same account; account reconcilable;
opposite directions; both posted under posted journals; party accounts ⇒ same party; period open
for the exchange-move date. Applied amount is always `min(requested, residual_debit,
residual_credit)`.

**Exchange difference**: when the paired lines carry different exchange rates, applying them
generates an FX adjustment journal (`source_type='reconciliation'`, FX account from config —
fail-closed if unconfigured), and `exchange_move_id` on the edge points at it so unlink can reverse
exactly it.

## Producer seams

Producers do not depend on this crate; the host implements the `ReconcileSink` port
(`backbone-gl-posting`) over these verbs, conn-taking so the edge commits inside the producer's own
transaction:

- **billing** — settlement apply/reverse creates/unwinds `origin=settlement` edges between the
  invoice's receivable/payable line and the payment's; `outstanding_amount` remains a cache probed
  equal to `grand_total − Σ edge amounts`.
- **banking** — a payment-matched clearance creates an `origin=clearing` edge across the clearing
  account; other matched-source kinds keep bounded clearances for now.
- **payment reversal** — the reversing lines are auto-paired with their originals once the
  original's residual returns to full (reverse-then-reconcile).

## Golden cases

- **RGG-1 (clamp):** applying 100 against residuals 60/100 writes a 60 edge; the chain stays
  partial; residuals become 0/40.
- **RGG-2 (guards):** each refusal — same account required, account not reconcilable, direction
  mismatch, party mismatch, currency mismatch, unposted line, unknown locator — returns its own
  error code.
- **RGG-3 (full group):** the edge that zeroes both sides stamps both lines `is_reconciled` with
  the group's id/timestamp and links its partials to the group.
- **RGG-4 (matching group read):** a 100 invoice met by 4×25 payments yields 5 lines, 4 edges, one
  label readable from any member line.
- **RGG-5 (aging):** `residuals_for_party` lists only open lines.
- **RGG-6 (tenancy):** a locator from another company does not resolve.
- **RGG-7 (unreconcile):** removing an edge restores both residuals and repairs the group (emptied
  groups dissolve; groups still holding ≥2 edges keep their identity).
- **RGG-8 (exchange):** doc-equal / base-differing lines generate the FX journal with the exact
  difference; unlinking nets FX and the account to zero across the generated + reversal journals
  and restores both residuals.
- **RGG-9 (reverse-then-reconcile):** posting a payment reversal and unlinking the original
  settlement pairs the reversed line with its original automatically (metadata
  `rule=reverse_then_reconcile`).
- **RGG-10 (concurrency):** two concurrent applies for 60 each against a 100 residual produce
  60 + 40, never 120.

Companion probes: `tests/integrity_probes.rs` (no CRUD routes on the graph tables; verbs mounted),
`tests/rls_probe.rs` (the fence rejects a cross-tenant edge write).
