# Maintainer Guide — backbone-accounting

> Reader: **maintainer** (extending this module without breaking conventions) · Mode: **how-to** (with brief **explanation**).

This is a Metaphor **module** (a bounded-context library crate, no `main.rs`). Most of the code you
see is generated from schema YAML. Your job is to change the schema, regenerate, and add custom
logic in the narrow places the pipeline leaves for you.

See also: [architecture](./architecture.md) · [glossary](./glossary.md) ·
[developer guide](./developer-guide.md) · [contributing](./contributing.md) · [FSD](./fsd.md) ·
[extension guide](./extension-guide.md) · [ADR-001 GL core boundary](./adr/ADR-001-gl-core-boundary.md) ·
[ADR-002 ledger write-path integrity](./adr/ADR-002-ledger-write-path-integrity.md) ·
[workspace CLAUDE.md](../CLAUDE.md).

---

## 1. The golden rule

**`schema/models/<entity>.model.yaml` is the single source of truth.**

Regeneration **overwrites everything** in a generated file **except** code between these markers:

```rust
// <<< CUSTOM: <name>
...your code survives...
// END CUSTOM
```

Never hand-edit a generated file outside those markers — your change is silently lost on the next
`generate`. Two rules that follow from this:

- To change an entity's shape/behavior, edit the schema YAML first, then regenerate.
- To add behavior the generator does not produce, use a CUSTOM marker or a sibling `*_custom.rs`
  file (see §5).

---

## 2. The regeneration loop

The canonical CLI is `metaphor` (v0.2.0). Prefer it over raw `cargo`/`sqlx`
(per [workspace CLAUDE.md](../CLAUDE.md)). Older docs use `backbone` — that is a local dev alias for
the same framework CLI; prefer `metaphor`.

Schema codegen is a **nested passthrough**, so the real invocations double the word `schema`:

```bash
# 1. Validate the schema
metaphor schema schema validate

# 2. Generate all targets for the accounting domain
metaphor schema schema generate accounting --target all

#    ...or a specific subset
metaphor schema schema generate accounting --target rust,sql,handler,repository

# 3. Apply migrations (see §6)
metaphor migration create <name>

# 4. Test
metaphor dev test        # or: metaphor test
metaphor lint check
```

The `CLAUDE.md` golden path abbreviates these as `metaphor schema validate`,
`metaphor make entity <Name>`, `metaphor migration create <name>`, `metaphor dev test`,
`metaphor lint check` — those are shorthands. The nested `schema schema` form above is the real one.
([ADR-001](./adr/ADR-001-gl-core-boundary.md) records `metaphor schema schema validate`.)

### What each codegen target produces

Available targets: `proto, rust, sql, repository, cqrs, handler, grpc, events, value-object,
validator, state-machine, permission, trigger, openapi, all`. The load-bearing ones here:

| Target | Produces |
|--------|----------|
| `rust` | domain entity structs, DTOs (`Create`/`Update`/`Response`) |
| `sql` | migration files in `migrations/` |
| `repository` | repository newtypes over `GenericCrudRepository` |
| `handler` | HTTP handlers + route registration via `BackboneCrudHandler` |
| `events` | Tier-A domain events |
| `validator` / `state-machine` / `permission` / `trigger` | runtime rule scaffolding |
| `openapi` | `schema/openapi/index.openapi.yaml` |
| `all` | everything enabled |

> `index.model.yaml` **disables** `graphql`, `grpc`, and `proto`, so those targets currently produce
> nothing. Don't rely on them. Global config there: `database: postgresql`, `soft_delete: true`,
> `audit: true`, `default_timestamps: true`; external import `sapiens.User`; shared types
> `Timestamps` / `Actors`.

Inspect the resulting route surface with:

```bash
metaphor routes
```

---

## 3. Where new code goes, per layer

Defer full folder structure to [architecture](./architecture.md). Quick map:

| I want to… | Where it goes |
|------------|---------------|
| Add a new entity | new `schema/models/<entity>.model.yaml` → regenerate |
| Add / change a field | edit that entity's `.model.yaml` → regenerate → new migration |
| Add custom business logic | `application/service/<entity>_service_custom.rs`, or a `// <<< CUSTOM` block |
| Add a non-CRUD endpoint | a handler fn under `presentation/http/`, wired in the route composer (not `BackboneCrudHandler`) |
| Wire a new service into the module | the `AccountingModule` builder in `lib.rs`, inside its CUSTOM markers |

---

## 4. Add a feature end-to-end: a `Vendor` entity

**Step 1 — Author the schema (SSoT).** Create `schema/models/vendor.model.yaml` following the shape
of an existing master/config entity (e.g. `account.model.yaml`).

**Step 2 — Generate.**

```bash
metaphor schema schema validate
metaphor schema schema generate accounting --target all
```

This emits the `Vendor` entity struct, `CreateVendorDto` / `UpdateVendorDto` / `VendorResponse`, the
repository newtype, the service type alias, the handler, and a migration.

**Step 3 — Migration.**

```bash
metaphor migration create create_vendor_table
```

**Step 4 — Wire it into the builder** (`lib.rs`, inside the `// <<< CUSTOM` markers, following the
existing 10-service pattern). Each service is a type alias; each repository is a thin newtype:

```rust
// type alias — do NOT hand-roll an impl
pub type VendorService =
    GenericCrudService<Vendor, CreateVendorDto, UpdateVendorDto, VendorRepository>;

// newtype over the generic repo
pub struct VendorRepository(GenericCrudRepository<Vendor, PgPool>);
```

In the builder, mirror the wiring used for the other services:

```rust
// <<< CUSTOM: builder-services
let repo = Arc::new(VendorRepository::new(pool.clone()));
let vendor_service = Arc::new(VendorService::with_repository(repo));
// END CUSTOM
```

Add custom repository methods only when `GenericCrudRepository` cannot express the query.

**Step 5 — Decide route exposure.** Is `Vendor` a **master/config** record or a **posted GL**
record? A vendor is master data → full CRUD is appropriate; merge its routes alongside `Account`,
`CostCenter`, etc. in `create_guarded_accounting_routes` (`src/presentation/http/guarded_routes.rs`).
If it were a posted GL record, you would mount it read-only instead — see §7.

---

## 5. Custom logic that survives regeneration

Two mechanisms, both immune to `generate`:

1. **Sibling `*_custom.rs` files** — e.g. `application/service/accounting_post_service_custom.rs`.
   Never generated, never overwritten.
2. **`// <<< CUSTOM … // END CUSTOM` markers** inside otherwise-generated files (the `AccountingModule`
   builder + struct carry these).

The following behaviors are **hand-authored** (not produced by any codegen target). Each is anchored
to a golden-case oracle; see [FSD §6](./fsd.md):

| Hand-authored behavior | Oracle / suite |
|------------------------|----------------|
| Posting validation **R1–R6** | `posting_golden_cases.rs` |
| Journal assembly | `posting_golden_cases.rs` |
| Ledger write with running balance **R9** | `posting_golden_cases.rs` / `integrity_probes.rs` |
| Reversal assembly **R7/R8** | `posting_golden_cases.rs` |
| Period-close routine | `period_close_golden_cases.rs` |
| Statement generation | `reporting_golden_cases.rs` |
| Reconciliation matching | `reconciliation_golden_cases.rs` |
| Retry driver | `messaging_bus.rs` / integration tests |
| Idempotency guard | `integrity_probes.rs` |

When you extend any of these, keep the logic in the `*_custom.rs` / CUSTOM block and keep its golden
case green.

---

## 6. Migrations

Migrations are **generated artifacts** — the `sql` target emits them into `migrations/`
(`NNN_description.up.sql` / `.down.sql`). They are currently **absent from the working tree pending
regeneration**; run the loop in §2 to reproduce them. Do **not** hand-write ad-hoc migrations that
diverge from the schema.

```bash
metaphor migration create <name>
```

**Fix invariants in the schema, not the generated migration.**
[ADR-002](./adr/ADR-002-ledger-write-path-integrity.md) is the model example: a racy idempotency
constraint was corrected in the SSoT (`accounting_post.model.yaml`) to a **partial unique index** on
`(company_id, source_type, source_id, posting_type) WHERE posting_status = 'posted'`, so every future
regeneration reproduces the correct constraint automatically. If you had patched the generated SQL
instead, the next `generate` would have reintroduced the bug.

---

## 7. Guarding the ledger

**Explanation.** The generated `AccountingModule::routes()` exposes full mutable CRUD
(POST/PATCH/upsert/bulk) on **every** entity, including posted GL records. That is a bypass: a caller
could write an unbalanced posting or PATCH a posted ledger row, defeating double-entry validation.
[ADR-002](./adr/ADR-002-ledger-write-path-integrity.md) closes this.

**How-to.** Mount the module via `create_guarded_accounting_routes(&AccountingModule) -> Router`
(`src/presentation/http/guarded_routes.rs`). It:

- Mounts posted GL entities — **Journal, JournalLine, Ledger, AccountingPost** — **read-only**.
  The only sanctioned writer is `PostingService` via `POST /accounting/posts`.
- Keeps full CRUD on master/config entities — **Account, CostCenter, FiscalPeriod,
  FinancialStatement, Reconciliation** (and `ReconciliationItem`).

Prefer `create_guarded_accounting_routes` over `AccountingModule::routes()` in any real deployment.

**When you add an entity, classify it first:**

- **Posted GL record** (double-entry, immutable once posted) → mount read-only, route writes through
  the posting contract.
- **Master / config record** → full CRUD is fine.

> Note: `JournalLine` and `ReconciliationItem` lack `@audit_metadata`, so their
> `soft_delete` / `restore` / `empty_trash` / `list_deleted` endpoints are not generated — they are
> cascade children of their parents.

---

## 8. Testing your change

Run the golden-case suites and integrity probes:

```bash
metaphor dev test           # or: metaphor test
# direct cargo path (needs DATABASE_URL for DB-backed tests):
cargo test -p backbone-accounting
```

Suites in `tests/`:

- `posting_golden_cases.rs` — posting validation, journal assembly, ledger balance, reversals.
- `period_close_golden_cases.rs` — period-close routine.
- `reconciliation_golden_cases.rs` — reconciliation matching.
- `reporting_golden_cases.rs` — statement generation.
- `integrity_probes.rs` — 2 permanent regression probes from
  [ADR-002](./adr/ADR-002-ledger-write-path-integrity.md).
- `integration_tests.rs`, `messaging_bus.rs`, and BDD `.feature` files under `tests/features/`.

Also run `metaphor lint check`. DB-backed tests need `DATABASE_URL` set.

---

## 9. Versioning, MSRV, release

- **Edition:** Rust 2021. Module is a **library crate** — never add `main.rs` or a binary target.
- **Version:** module semver, currently **0.2.0**.
- **Extension contract** ([extension guide §6](./extension-guide.md)): Tier-A events and exported DTOs
  are versioned.
  - **Additive** change (new optional field, new event) → **minor** bump.
  - **Remove / rename / semantic change** → **major** bump; ship the old alongside the new for
    **≥1 migration cycle**.

Never leak one module's entity into another module's API.
