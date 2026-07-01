# Contribution Guide

> Reader: **contributor** — proposing and landing a change to this module. Mode: **how-to**.
> If you are *consuming* the module instead, see the [Developer Guide](./developer-guide.md). If you
> are *extending* the GL from another module, see the [Extension Guide](./extension-guide.md).

## Before you write code

This is a Metaphor **`module`** project: a bounded-context library crate whose code is largely
**generated from `schema/models/*.model.yaml`**. Read these first — a PR that hand-edits generated
code will be sent back:

- [`../CLAUDE.md`](../CLAUDE.md) — the module conventions (non-negotiable).
- [Maintainer Guide](./maintainer-guide.md) — the regeneration loop and where new code goes.
- [Glossary](./glossary.md) — use the existing term for a thing; don't coin a synonym.

## Dev setup

```bash
metaphor info      # confirm you are inside backbone-accounting (a Module project)
metaphor doctor    # tooling + plugin health
```

- **Toolchain:** Rust 2021 edition.
- **Database:** PostgreSQL. Export `DATABASE_URL` for tests that hit the DB:
  `export DATABASE_URL=postgresql://localhost/accounting_dev`.
- **CLI:** use `metaphor` (v0.2.0), not raw `cargo`/`sqlx`, wherever a subcommand exists — it applies
  workspace-wide policy. (`backbone` in older docs is a local dev alias for the framework CLI.)

## The change loop

1. **Edit the schema, not the generated code.** For any entity change, edit
   `schema/models/<entity>.model.yaml` first.
2. **Validate:** `metaphor schema schema validate`
3. **Regenerate:** `metaphor schema schema generate accounting --target all`
4. **Custom logic** goes in a sibling `*_custom.rs` or inside `// <<< CUSTOM … // END CUSTOM`
   markers — nowhere else. Everything outside those markers is overwritten on the next regen.
5. **Migrate** (generated artifacts): `metaphor migration create <name>`.
6. **Test:** `metaphor dev test` (or `cargo test -p backbone-accounting` with `DATABASE_URL` set).
7. **Lint:** `metaphor lint check`.

See the [Maintainer Guide](./maintainer-guide.md) for the full add-a-feature walkthrough.

## Commit conventions

- **Conventional Commits.** `type(scope): summary` — e.g. `feat(ledger): add running-balance probe`,
  `fix(posting): reject foreign currency until FX is modeled`, `docs(handbook): add architecture page`.
  Common types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`.
- **One-line, context-carrying messages.** State *why*, not just *what*. No filler
  (`update`, `fix stuff`, `wip`, `changes`).
- **Group by functionality.** Split unrelated changes into separate, focused commits; keep large
  files in their own commit.
- **NEVER add a signature.** No `Co-Authored-By`, no "Generated with", no Claude/AI attribution of
  any kind in commit messages or PR bodies. This is a hard rule for this workspace.

## Tests you must not break

Changes to the GL write path or invariants must keep these green (see the
[Maintainer Guide](./maintainer-guide.md#testing-your-change) for how to run them):

| Suite | Guards |
|-------|--------|
| `tests/posting_golden_cases.rs` | G1–G3 posting oracle (balanced post, out-of-balance, missing party) |
| `tests/period_close_golden_cases.rs` | Period close / closed-period rejection (G5, G7) |
| `tests/reconciliation_golden_cases.rs` | Bank reconciliation (G8) |
| `tests/reporting_golden_cases.rs` | Trial balance / statements (G7) |
| `tests/integrity_probes.rs` | The two permanent regression probes from [ADR-002](./adr/ADR-002-ledger-write-path-integrity.md) (concurrent double-post, guarded CRUD) |
| `tests/integration_tests.rs`, `tests/messaging_bus.rs` | End-to-end + event emission |
| `tests/features/` | BDD `.feature` acceptance scenarios |

New behavior that maps to a [business rule](./brd.md) or [golden case](./glossary.md#concepts)
needs a test tied to that oracle.

## PR checklist

Before you open a PR, confirm:

- [ ] Schema validates (`metaphor schema schema validate`) and the generated tree is clean
      (no hand-edits outside `// <<< CUSTOM` markers).
- [ ] Custom logic lives in `*_custom.rs` or `CUSTOM` markers — nothing generated was hand-edited.
- [ ] No `main.rs` / binary target added (this is a library crate).
- [ ] No ad-hoc Axum CRUD routes (use `BackboneCrudHandler`); posted GL entities stay behind
      [guarded routes](./glossary.md#concepts).
- [ ] No accounting entity leaked into another module's API; no producer imported.
- [ ] Golden-case suites and integrity probes pass; new behavior has a test tied to its oracle.
- [ ] `metaphor lint check` is clean.
- [ ] Commits are conventional, grouped, and **signature-free**.
- [ ] A design decision that changes a boundary, invariant, or public contract is recorded as an
      [ADR](./adr/) (see below).

## When to write an ADR

Architecturally significant decisions get a record under [`docs/adr/`](./adr/) — one decision per
file (context, decision, status, consequences). Examples already in the tree:
[ADR-001](./adr/ADR-001-gl-core-boundary.md) (GL-core boundary & tenancy) and
[ADR-002](./adr/ADR-002-ledger-write-path-integrity.md) (ledger write-path integrity). **Accepted
ADRs are immutable** — supersede with a new ADR rather than editing an accepted one. See the
[ADR index](./adr/README.md).

## Review expectations

- Reviewers check against the multi-axis lens: correctness (does the invariant still hold?),
  boundary hygiene (no new inbound coupling), regen-safety (survives `--target all`), and test
  coverage tied to an oracle.
- The load-bearing invariant — **Σdebit = Σcredit, append-only ledger, no double-count** — is
  non-negotiable. A change that can violate it through *any* surface (service or HTTP) will not land;
  see [ADR-002](./adr/ADR-002-ledger-write-path-integrity.md) for why that includes the CRUD surface.
