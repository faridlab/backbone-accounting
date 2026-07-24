<!--
date: 2026-07-24
repo_type: module
unit: backbone-accounting
focus: maturity (2nd run — re-grade after the concurrency fix, CoA seed, journal workflow, repository-compliance refactor, and hierarchy endpoints landed)
roster: chair, skeptic, steelman, yagni-business, ddd-bounded-context, contract-seat, domain-expert(invited)
-->

# Council — module:backbone-accounting (2nd run) — focus: maturity

## Best call

**Ship v0.3 as a general-ledger module with two must-fix correctness holes closed first; defer the auth-fence contract to an ADR. The module is functionally complete for a reusable GL library at the right bar (posting, ledger, reversal, period close, three statements, reconciliation, hierarchy all present and correctly atomic); missing domains (FX, fixed assets, AR/AP aging, budgeting) are correctly fenced by ADR-003 and not blocking any producer this month.**

Two holes prevent the "mature reusable library" claim from being honest, and both are cheap:

1. **Negative-amount guard in `posting_rules::validate`** — confirmed: `src/domain/services/posting_rules.rs:30-34` sums `debit` and `credit` and checks equality only. A balanced negative pair `{debit:-100, credit:-100}` posts, because `total_debit == total_credit` holds at `-100 == -100`. This is an accounting-correctness defect (corrupts normal-balance semantics, breaks sign-sensitive reports), not a security issue. Fix is two lines: reject any line where `debit < 0 || credit < 0` (and `debit > 0 && credit > 0`).
2. **Identity boundary leak on the custom routes** — confirmed: `posting_handler.rs:40,52`, `journal_workflow_handler.rs:22-40`, `accounting_ops_handler.rs:47`, `reporting_handler.rs:25,31`, `hierarchy_handler.rs:22` all read `company_id` and every `*_by` field from request body/query. Generated CRUD handlers DO use `backbone_auth::middleware::AuthContext`, so the leak is scoped to the 5 custom handlers — but there the audit trail is fully forgeable by the caller.

- Residual negative value: the negative-amount hole is "one bad producer call corrupts the GL and the bad row is immutable (append-only ledger) — recovery is a human-posted compensating reversal." The identity leak is "audit trails on 5 custom endpoints are caller-asserted, not principal-asserted — unacceptable if a host binds the module directly to a public route."
- Reversibility: both local and reversible. The sign guard is a 2-line predicate; the identity fix threads `AuthContext` (the CRUD surface's existing pattern) into 5 handlers.
- What would flip this: (a) a producer intentionally posting negative debit/credit as a sign convention; (b) a host already gating the custom routes and re-deriving identity upstream (then downgrade the identity fix to ADR-only). Neither claimed.

## Disagreement map

**T1 — Is the RLS/role fence the module's problem?** Skeptic (module-must-enforce) vs Steelman + DDD-seat (library-must-document-host-contract). *Crux:* this is a `module` crate consumed by hosts; CLAUDE.md puts auth middleware in the host's `presentation/middleware`. **Call: contract-seat wins — the gap is the missing ADR documenting the host contract (C2), not missing module code.** The RLS migration exists; the contract does not.

**T2 — Is the negative-amount gap security or correctness?** Skeptic (producer-trust) vs Domain-expert (correctness hole). *Crux:* `posting_rules::validate` already enforces ≥2 lines, party, postable — sign constraint is the same family. **Call: domain-expert wins — correctness, must-fix before 0.3.**

**T3 — Was "no AuthContext" accurate?** Verified — 10 generated handlers use `AuthContext`; the 5 custom handlers do not. **Call: skeptic directionally right (the leak is real) but overbroad; the fix targets the 5 custom handlers.**

## Recommendations (ranked by leverage)

| # | Move | Leverage | Residual negative | Reversibility | Evidence to flip |
|---|------|----------|-------------------|---------------|------------------|
| 1 | Add sign constraint to `posting_rules::validate` — reject `debit < 0 \|\| credit < 0` (+ `debit > 0 && credit > 0`); unit test that `{debit:-100, credit:-100}` is rejected | Highest — closes a real correctness hole for ~2 lines + 1 test | ~0 unless a producer relies on negative-posting convention | Trivially reversible | A producer intentionally sending negative debit/credit |
| 2 | Thread `AuthContext` into the 5 custom handlers (posting, journal_workflow, accounting_ops, reporting, hierarchy) — derive `company_id` + `*_by` from the principal, mirror CRUD's pattern | High — makes the custom-surface audit trail principal-asserted | ~1–2 days mechanical; small risk to hosts passing body `company_id` | Reversible | A host already gates these routes upstream (downgrade to ADR-only) |
| 3 | Write ADR-0011: "Host auth/role contract" — host connects as non-superuser w/o BYPASSRLS, sets `app.company_id` per request, gates custom routes behind an AuthContext middleware | Medium — closes the "host doesn't know what it must satisfy" gap; zero runtime risk | Hosts that read it slowly | Trivial (docs) | Discovery the module already enforces any of these (it does not) |
| 4 | Integration test: superuser/owner-role POST with mismatched body `company_id` is rejected | Probe validating C2 is host-dependent | ~nil | Reversible | Test passing under superuser confirms the worst case |
| 5 | Park FX, fixed assets, budgeting, AR/AP aging | Keeps scope honest | Opportunity cost only | N/A | A concrete producer requirement |

## Maturity scorecard (focus = maturity)

| Seat | Axis | Score (1–5) | One sentence why |
|---|---|---|---|
| Steelman | GL completeness | 4 | All required GL surfaces present and atomic; one correctness invariant (sign) missing. |
| Skeptic | Security boundary | 2 | RLS migration exists but role/middleware contract is undocumented; custom-route identity is fully forgeable. |
| Domain-expert | Accounting correctness | 3 | Sign-constraint gap is a real double-entry hole, not just security theater. |
| DDD-bounded-context | Layering hygiene | 4 | Clean except identity/auth crossing the presentation boundary on custom routes. |
| Contract-seat | Contract completeness | 3 | ADR-003 fences deferred scope; the host auth/role contract (C2) ADR is missing. |
| YAGNI-business | Real-this-month pain | 4 | Only the two must-fixes are real; nothing else blocks a producer this month. |

**Composite maturity for a reusable GL library crate: 3.5 / 5.** Closing #1 and #2 lifts it to ~4.5; closing #3 lifts the contract surface to match.

## Parking lot

- **Currency as free-form `String` defaulting to IDR** — Skeptic, scope: future multi-currency producer (C1). Revisit when FX is unfenced.
- **`branch_id` on `FailedPost` not consistently threaded** — noted in `posting_service.rs`; file as a follow-up.
- **Immutable ledger means corruption from the negative-amount hole needs a human-posted compensating reversal** — operational runbook concern, not a code change.
- **FX / fixed assets / budgeting / AR-AP aging** — fenced by ADR-003; revisit on first concrete producer requirement.
