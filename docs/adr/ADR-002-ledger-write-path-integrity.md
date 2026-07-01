# ADR-002: Ledger write-path integrity (idempotency constraint + guarded CRUD)

**Status**: Accepted — **Applied 2026-07-01**
**Deciders**: Farid (owner), council (module:backbone-accounting, focus=maturity, 2026-07-01)
**Supersedes/extends**: ADR-001 (GL-core boundary)

## Context

A maturity-focus council review of `backbone-accounting` found the ledger's central invariant
(append-only, always-balanced, no double-count) was violable through the module's **own generated
surface**, in two ways the single-threaded golden-case oracle structurally could not observe:

1. **Idempotency not enforced under concurrency.** `PostingService::post` guarded duplicates with a
   pre-transaction `SELECT`, but the only unique index was
   `(source_type, source_id, posting_type, journal_id)` — and `journal_id` is freshly minted per
   post, so the tuple was unique on every attempt and the index could never dedupe. Two concurrent
   posts of the same source both passed the SELECT and both committed → ledger double-count. The
   SSoT declared the racy index, so regeneration reproduced it forever, while docs/hooks/workflow
   all *claimed* the key was `(company_id, source_type, source_id, posting_type)`.
2. **Mutable CRUD bypass.** `BackboneCrudHandler` auto-wires all 12 endpoints (incl POST/PATCH/
   upsert/bulk) on every entity, including the posted GL records. A caller could `POST` an
   unbalanced `accounting_posts` row with `posting_status='posted'`, or `PATCH` a posted ledger
   row, bypassing double-entry validation entirely.

## Decision

1. **DB-enforced idempotency.** The unique index on `accounting_posts` is now a **partial** index:
   `UNIQUE (company_id, source_type, source_id, posting_type) WHERE posting_status='posted'` (and
   not soft-deleted). Partial on `posted` so failed attempts don't block a retry. `PostingService`
   keeps the pre-transaction SELECT as a fast path, but the DB constraint is the real arbiter: on a
   unique-violation during the posting insert, the transaction rolls back (no partial write) and the
   concurrent winner is returned (`idempotent_reuse: true`). Fixed in the SSoT
   (`accounting_post.model.yaml`) so regen reproduces the correct constraint.
2. **Guarded route composition.** A hand-owned `create_guarded_accounting_routes` mounts the posted
   GL entities (`Journal`, `JournalLine`, `Ledger`, `AccountingPost`) **read-only**; the only
   sanctioned writer is `PostingService` (`POST /accounting/posts`). Master/config entities keep
   full CRUD. The composing app uses the guarded routes.

## Consequences

- ✅ Concurrent double-posting is impossible (DB-enforced); the ledger cannot double-count.
- ✅ The immutable-ledger / clean-boundary claim now holds through the shipped HTTP surface, not
  just the service path.
- ✅ Both fixes are regen-safe and reversible (schema field-list + route composition; no data
  migration). Two permanent regression probes added (`tests/integrity_probes.rs`).
- ⚠️ Consumers that used the generated write CRUD on GL entities must switch to `POST /accounting/posts`.
- ⚠️ A pre-existing `accounting_posts` table with duplicate posted rows would fail the new unique
  index; must dedupe first (pre-revenue: empty prod).

## Parking lot (council, out of the maturity lens — deferred)

FX base-currency correctness (`base=amount` is wrong once a foreign-currency invoice arrives — gate:
reject foreign currency until FX is modeled); period-close sequencing (`post_date ≥ last-closed`);
Income Summary / re-open path; thin `AccountingPostPosted` event forcing subscriber callbacks;
at-most-once fire-and-forget event delivery; cash-flow statement; statement snapshotting;
partial/timing reconciliation; authorization on the posting endpoint.
