# ADR-0011: Host auth/role contract (the tenant + identity boundary this module relies on)

**Status**: Accepted — **Applied 2026-07-24**
**Deciders**: Farid (owner), council (module:backbone-accounting, 2nd run, focus=maturity, 2026-07-24)
**Supersedes/extends**: ADR-0008 (company RLS fence), ADR-0010 (cross-tenant reversal leak)

## Context

The 2026-07-24 maturity council found that the module's multi-tenant security posture is **asserted
but not self-enforcing**: row-level security is `ENABLED`+`FORCED` with `USING`/`WITH CHECK`
predicates on all 9 company-scoped tables (ADR-0008), and every adapter query scopes by
`company_id` at the predicate level — but RLS only protects the data **if the consuming host
satisfies three conditions the module cannot verify from inside a library crate**:

1. **The runtime DB role is not a superuser and does not have `BYPASSRLS`.** Postgres voids
   `FORCE ROW LEVEL SECURITY` for superusers and `BYPASSRLS` roles. The migration owner (which runs
   migrations/seeders) legitimately bypasses RLS; the *serving* connection pool must not.
2. **`app.company_id` is set per request** (`SET LOCAL`) to the principal's verified company before
   any accounting SQL runs, so the RLS predicates resolve against the authenticated tenant — not a
   value read from the request body.
3. **The custom (non-CRUD) routes are mounted behind an auth middleware** that extracts a verified
   principal and supplies `AuthContext`. The generated CRUD handlers already require
   `backbone_auth::middleware::AuthContext`; the hand-written handlers
   (posting, journal-workflow, accounting-ops, reporting, hierarchy) were updated in the same
   change to derive `company_id` and the `*_by` audit fields from `AuthContext` rather than from the
   request body.

If any of these is violated, the boundary degrades: a superuser connection makes every
`company_id` in a request body a free cross-tenant write; deriving `company_id` from the request
instead of the session makes RLS faithfully enforce the attacker's chosen tenant; and
ungated custom routes leave the audit trail (`posted_by`, `approved_by`, …) caller-forgeable.

## Decision

This module treats the three conditions above as a **documented host contract**, not as code it
ships. A consuming `backend-service` host MUST:

1. Run migrations/seeders as the table owner, but serve requests from a connection pool whose role
   is **non-superuser and without `BYPASSRLS`**, and whose only tenant privilege comes from the RLS
   policies.
2. Install request-scoped middleware that sets `app.company_id` (`SET LOCAL`) from the **verified
   principal**, before the accounting layer runs.
3. Mount the custom routes behind the same `AuthContext`-deriving middleware the CRUD surface uses,
   so `company_id` and every `*_by` field are principal-asserted.

The module's own contribution is: the RLS fence (ADR-0008), `company_id` predicates in every
adapter, the custom handlers' use of `AuthContext`, and this contract.

## Consequences

- **Positive:** the boundary is explicit and testable. Hosts know exactly what they must provide;
  reviewers can check the host config against this ADR. The module stays a library (no DB-role
  bootstrap or global middleware baked in).
- **Negative:** a host that ignores the contract is silently insecure — RLS gives a false sense of
  protection under a superuser pool. The mitigation is the probe test
  (`tests/rls_probe.rs`) that demonstrates a mismatched-tenant write is rejected under a
  restricted role, and fails loudly if run under a superuser.
- **Revisit trigger:** if a host cannot satisfy condition 1 (e.g. a shared infra constraint), the
  tenant fence must move into application-level enforcement inside this crate — reopen this ADR.
