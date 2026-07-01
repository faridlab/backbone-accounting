# Developer Guide — `backbone-accounting`

> **Reader:** app developer (building a backend-service or module that consumes/drives this GL module).
> **Mode:** tutorial (quickstart) + how-to (recipes).

This guide takes you from *add the dependency* to *post your first balanced transaction*. It stops
at the edge of the posting contract — the deep "how to drive/extend the GL from another module"
material lives in [extension-guide.md](./extension-guide.md).

See also: [philosophy.md](./philosophy.md) · [glossary.md](./glossary.md) ·
[architecture.md](./architecture.md) · [fsd.md](./fsd.md) · [brd.md](./brd.md) ·
[maintainer-guide.md](./maintainer-guide.md).

---

## 1. Install

`backbone-accounting` is a **library crate** (`[lib]` only, no `main.rs`). You cannot run it on its
own — you embed it in a `backend-service`. Add it as a Cargo dependency:

```toml
# your-service/Cargo.toml
[dependencies]
# From git (as the module itself is consumed):
backbone-accounting = { git = "https://github.com/faridlab/backbone-accounting", branch = "main" }

# ...or a local path in a workspace:
# backbone-accounting = { path = "../backbone-accounting" }
```

The framework companion crates come **transitively** — you do not add them yourself:
`backbone-core` (postgres), `backbone-orm`, `backbone-auth`, `backbone-messaging`, plus
`sqlx 0.8` (postgres), `axum 0.7`, `tokio`, `rust_decimal`, `uuid`, `chrono`.

---

## 2. Quickstart

The smallest thing that runs: build the module, mount its routes on an Axum app.

```rust
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use backbone_accounting::AccountingModule;
use backbone_accounting::presentation::http::create_guarded_accounting_routes;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;

    // Build the module (wires every service).
    let module = AccountingModule::builder()
        .with_database(pool.clone())
        .build()?;

    // Recommended for anything exposed: posted GL entities are read-only.
    let app: Router = Router::new()
        .merge(create_guarded_accounting_routes(&module));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

### `routes()` vs `create_guarded_accounting_routes(&module)`

| Mount                                        | What you get                                                                 |
|----------------------------------------------|------------------------------------------------------------------------------|
| `module.routes()`                            | Full 12-endpoint CRUD on **every** entity — including posted GL records.      |
| `create_guarded_accounting_routes(&module)`  | Posted GL entities (Journal, JournalLine, Ledger, AccountingPost) **read-only**; master/config entities keep full CRUD. |

**Recommendation:** use `create_guarded_accounting_routes` for anything you expose. `routes()`
lets a caller `POST`/`PATCH` a posted ledger row directly, bypassing double-entry validation. The
only sanctioned GL writer is `POST /accounting/posts` (see [Key concepts](#3-key-concepts) and
[extension-guide.md](./extension-guide.md)).

### Migrate and seed

Migrations live in `migrations/` and run through the framework/sqlx. Seed the chart of accounts
from `migrations/seeds/`. Bring the schema up before starting the service; list the resulting HTTP
surface with:

```bash
metaphor routes
```

---

## 3. Key concepts

Brief here — full definitions in [glossary.md](./glossary.md), rationale in
[philosophy.md](./philosophy.md).

- **The posting port.** `AccountingPost` is the inbound contract; `POST /accounting/posts` is the
  **only GL write path**. Everything that touches the ledger goes through it.
- **Immutable ledger + reversal-not-edit.** Posted `Journal`/`JournalLine`/`Ledger` rows are never
  mutated. To undo, you post a *reversal* (rules R7, R8).
- **Fiscal period gating.** A post's `posting_date` must fall in an open `FiscalPeriod`; closed
  periods reject writes (R4).
- **Schema-per-tenant.** Every root entity carries `company_id`; isolation is enforced by the
  PostgreSQL `search_path` (one schema per tenant). `currency` defaults to **IDR** module-wide.

---

## 4. Recipes

Each recipe is a goal + steps. Posting-contract depth (field-by-field, event choreography) is in
[extension-guide.md](./extension-guide.md).

### Post a balanced transaction

**Goal:** record a sales invoice (golden case G1). Emit an `AccountingPost` to `POST /accounting/posts`.

```jsonc
POST /accounting/posts
{
  "company_id": "…",
  "source_type": "order",
  "source_id": "INV-1001",          // keep stable → retries stay idempotent
  "posting_type": "original",
  "currency": "IDR",
  "posting_date": "2026-07-01",
  "lines": [
    { "account_id": "…ar…",      "debit": 1110000, "credit": 0,
      "party_type": "customer", "party_id": "…" },   // party required on AR/AP
    { "account_id": "…revenue…", "debit": 0, "credit": 1000000 },
    { "account_id": "…ppn-out…", "debit": 0, "credit": 110000,
      "is_tax_line": true, "tax_rate": 0.11 }
  ]
}
```

Response carries `posting_status` and `journal_id` (or `202` for async). **Do not block on it** —
subscribe to the events instead:

- `AccountingPostPosted { post_id, source_type, source_id, journal_id, status }`
- `AccountingPostFailed { post_id, source_type, source_id, error_code, error_message }`

Status feedback is **event-only**; there is no synchronous callback. See
[extension-guide.md §3](./extension-guide.md).

### Reverse a posted transaction

**Goal:** undo a posting without editing the ledger (R7).

Emit a new post with `posting_type: "reversal"` and the debit/credit swapped versus the original.
The reversal lands in the **current open period**. Never edit or delete the original journal/ledger
rows. Depth: [extension-guide.md](./extension-guide.md).

### Read AR/AP aging

**Goal:** party-level receivable/payable balances.

Read them off the **`Ledger`** (`GET /api/v1/ledgers`, filter by party). Ledger balances are
source-independent — aging is the same whether the underlying post came from an order, payment, or
manual entry. See running balance (R9) in [glossary.md](./glossary.md).

### Generate a financial statement / trial balance

**Goal:** trial balance or statement snapshot.

Use the **`FinancialStatement`** read model (`GET /api/v1/financial_statements`). It exposes
`trial_balance_check` and `balance_check` so you can assert the books tie out.

### List the HTTP routes

```bash
metaphor routes
```

Every entity is served at `/api/v1/{collection}` with up to 12 endpoints (list, create, get,
update, patch, soft_delete, restore, empty_trash, bulk_create, upsert, find_by_id, list_deleted).
`JournalLine` and `ReconciliationItem` omit the soft-delete family (they cascade with their parent).
Under `create_guarded_accounting_routes`, the posted GL entities expose only the read endpoints.

---

## 5. Configuration

| Setting                | Where                     | Notes                                                        |
|------------------------|---------------------------|-------------------------------------------------------------|
| `DATABASE_URL`         | env                       | PostgreSQL connection string (required).                    |
| `RUST_LOG`             | env                       | tracing level, e.g. `RUST_LOG=info,backbone_accounting=debug`. |
| `config/application.yml` | repo (module-local)     | module-local config file shipped in the repo.               |
| tenant `search_path`   | connection / session      | schema-per-tenant isolation — one Postgres schema per tenant. |
| `currency`             | per-post (defaults IDR)   | module-wide default is **IDR**.                             |

---

## 6. Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| Post rejected, Σdebit ≠ Σcredit | Double-entry invariant **R1** violated (golden case **G2**). | Balance the lines: total debits must equal total credits in base currency. |
| Post rejected, "party required" | An AR/AP line (`account_subtype` ∈ `accounts_receivable`/`accounts_payable`) is missing `party_type`/`party_id` — rule **R3** (**G3**). | Add `party_type` and `party_id` on every AR/AP line. |
| Post rejected, closed period | `posting_date` falls in a closed/locked `FiscalPeriod` — rule **R4** (**G5**). | Post into an open period, or record it as an adjusting entry. |
| Duplicate post did nothing | Idempotency: a repeat of `(company_id, source_type, source_id, posting_type)` while a `posted` row exists is a no-op (**ADR-002**). | Expected. Keep `source_id` stable so retries are idempotent. |
| "Cannot POST/PATCH a journal/ledger row" | Those entities are mounted read-only under `create_guarded_accounting_routes`. | Write through `POST /accounting/posts` — the only sanctioned GL writer. |
| Build error: no `main.rs` / can't run it | `backbone-accounting` is a **library crate**. | Embed it in a `backend-service` (see [Install](#1-install) / [Quickstart](#2-quickstart)). |

Golden cases G1–G8 are enumerated in [brd.md](./brd.md); business rules in [fsd.md](./fsd.md).
