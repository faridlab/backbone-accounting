<!--
date: 2026-07-23
repo_type: module
unit: backbone-accounting
focus: maturity
roster: chair, skeptic, steelman, yagni-business, ddd-bounded-context, contract-seat, domain-expert(invited)
-->

# Council — module:backbone-accounting — focus: maturity

## Best call

**Land the per-account concurrency lock (`SELECT ... FOR UPDATE` on `accounts` inside the posting tx, or `pg_advisory_xact_lock((company_id, account_id))`) AND ship a default Indonesian CoA seeder in the same push. Answer the three questions as: (1) yes — it is a complete, proper GL *module* once the lock lands; (2) no — do not adapt more from bersihir, it is an 80-model laundry monolith and the GL boundary is already cleanly extracted; (3) yes — the seeder becomes the default onboard-CoA so a tenant can post on day one.**

The lock is the spine. The Skeptic's finding (confirmed broken by DOMAIN-EXPERT) is not a scaling concern — it is a correctness fault: two distinct producers posting to one account (payment gateway + POS hitting "Bank – BCA" the same millisecond) both read the same `current_balance` and `MAX(sequence_number)` with no row lock and no advisory lock, both insert duplicate `sequence_number`, and the second `UPDATE accounts SET current_balance` last-writer-wins — permanently corrupting the running-balance chain and the cached balance. The existing unique indexes key on *source identity* (dedup of the same post); the sequence index `idx_ledgers_company_id_account_id_sequence_number` is non-unique, and ADR-0010's only concurrency test covers same-source double-post, not distinct-source/same-account. Multi-producer-to-one-account is the normal production case, so the books-of-record claim collapses until this is fixed.

- Residual negative value: small and bounded — one additional row lock per post (sub-millisecond on a contended account, serializes only same-account writers, not the whole ledger; cross-account posts remain fully parallel); ~40-line integration test (N=8 distinct-source/same-account asserting no duplicate seq, monotonic balance chain, `current_balance == final balance_after`). The seeder adds one SQL file and a tenant-onboard call; zero runtime cost.
- Reversibility: high. The lock is a one-line change inside the existing tx; revertible with no schema migration. The seeder is pure data. Neither touches the generated surface or the schema YAML.
- What would flip this: evidence that posts to a single account are *never* concurrent in the actual deployment (single-threaded producer per account). That is implausible for any real ERP integration, so the flip condition is effectively "this module has no producers" — which would itself be the bigger problem.

## Disagreement map

1. **Is the core "already correct and audited"?** Steelman/CONTRACT-SEAT say the core is exemplary and audited; Skeptic + DOMAIN-EXPERT say the audit proves only same-source safety and the distinct-source/same-account case is untested and broken. **Crux:** whether `accounts.current_balance` and `MAX(sequence_number)` are read under a lock. They are not (grepped: zero `FOR UPDATE`, zero `pg_advisory_xact_lock` across migrations). Skeptic wins.
2. **More extraction from bersihir?** DDD-BOUNDED-CONTEXT and YAGNI-BUSINESS say the GL boundary is already clean (9 entities, no laundry tail, generic inbound seams); Steelman leans toward fidelity. **Crux:** a module is a bounded context, not a slice of a monolith. Pulling more couples this crate to a vertical and removes zero current pain. No-adapt wins.
3. **Seeder as default account values — data or contract?** Steelman flags it as the only genuine first-use blocker; YAGNI-BUSINESS says ship a default Indonesian CoA. **Crux:** you cannot post without accounts, so an empty seeder blocks first use. Ship-the-default wins, framed as "tenant-onboards-CoA with a sensible default."
4. **Are FX / fixed assets / budgeting / aging maturity gaps?** CONTRACT-SEAT calls single-currency IDR an undocumented contract constraint; DOMAIN-EXPERT and YAGNI-BUSINESS say these are correctly out of scope for v0.3. **Crux:** the right bar is "trustworthy GL library," not "ERP." Document as deferred (one ADR), do not build.

## Recommendations (ranked by leverage)

| # | Move | Leverage | Residual negative | Reversibility | Evidence to flip |
|---|------|----------|-------------------|---------------|------------------|
| 1 | Add `SELECT ... FOR UPDATE` on `accounts` (or `pg_advisory_xact_lock` per `(company_id, account_id)`) inside the posting tx in `posting_service.rs:235-414`; add the ~40-line N=8 distinct-source/same-account integration test | Restores the books-of-record guarantee; without it every other maturity axis is moot | ~sub-ms per contended same-account post; serializes only same-account writers | One-line in-tx change, no migration; fully revertible | Proof that no two producers ever post the same account concurrently |
| 2 | Ship a default Indonesian CoA seeder (`account_seed.sql`) — asset/liability/equity/revenue/expense/COGS with PPN Output Payable / PPN Input / PPh Payable / retained earnings / Bank/Cash/AR/AP detail — wired as the tenant-onboard default | Unblocks first use; you literally cannot post without accounts | Couples the crate to an Indonesian default CoA (acceptable: the module is already IDR-only) | Pure data file; swap per tenant | A tenant that genuinely needs to author its own CoA from scratch (rare) |
| 3 | Answer "should we adapt more from bersihir?" → **no**; record the bounded-context decision in one ADR (9 GL entities, generic inbound seams, producer-owned FX/tax/party) | Prevents vertical re-coupling; removes imaginary work | None — bersihir's GL is already extracted | Decision record only | A concrete current pain that only a bersihir model solves (none identified) |
| 4 | Make the single-currency (IDR) assumption a *documented* contract constraint: ADR deferring FX + fixed assets + budgeting + AR/AP aging as explicit YAGNI; fix the `base_debit_amount = debit` copy-lie (either populate base or document that base=IDR today) | Removes the undocumented-constraint tax on producers | A short ADR + one column comment | Documentation only | A real user blocked by multi-currency this month (none identified) |
| 5 | Either wire the advertised draft→approve→post journal flow to HTTP, or add an ADR deferring it and stop advertising it in the BRD | Removes the advertised-but-absent contract surface | Small scope cost if wired; zero if deferred | Endpoint or ADR | A user who actually needs approval workflow this month |

## Maturity scorecard (focus = maturity)

| Seat | Axis | Score (1–5) | One sentence why |
|---|---|---|---|
| Steelman | Core GL completeness | 4 | Double-entry posting, immutable ledger, idempotency, trial balance, period close, reconciliation are all present and tied out — only the empty seeder genuinely blocks first use. |
| Skeptic | Concurrency correctness | 2 | The running-balance + sequence_number path is not race-safe under distinct-source/same-account posts and will silently corrupt the books in the normal production case. |
| DDD-Bounded-Context | Boundary cleanliness | 5 | Extraction is faithful at the GL boundary — 9 entities, no laundry tail, generic inbound seams, party_id/tax correctly producer-owned, no entity leak. |
| Contract-Seat | Contract maturity | 3 | PostingRequest/Result/Error and events are well-typed and regen-safe, but single-currency IDR is an undocumented constraint and the draft→approve→post flow is advertised but not exposed. |
| Domain-Expert | Accounting correctness | 3 (conditional on fix #1) | R1–R11 invariants and the hard flows are modeled per standard double-entry practice, but the concurrency fault disqualifies the books-of-record claim until fixed. |
| YAGNI-Business | This-month pain addressed | 4 | Correctly reframes the three asks to two real pains (lock + seed) and correctly defers FX/assets/budgeting/aging as imaginary scale. |

## Parking lot

- FX / multi-currency revaluation — raised by CONTRACT-SEAT + DOMAIN-EXPERT, scope: this module's future (defer via ADR, not this month).
- Fixed-asset register + depreciation — raised by Steelman, scope: a sibling asset module, not this crate.
- AR/AP aging endpoint — raised by Steelman, scope: a sibling AR/AP module or a read-model; not core GL.
- Budgeting — raised by Steelman, scope: a sibling budget module.
- Manufacturing/payroll/maintenance/equity inbound producers — raised by DDD-Bounded-Context, scope: producer services, not this crate.
- `base_debit_amount` column semantics (copy vs converted) — raised by CONTRACT-SEAT, scope: this module's DTO; fold into recommendation #4.
