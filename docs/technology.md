<!-- Reader: evaluator (secondary: maintainer). Mode: explanation. -->

# Technology & the Why

**The point:** Every stack choice in `backbone-accounting` is defensive. This is a ledger of record, so the technology is chosen to make money exact, invariants type-checked, and posting idempotent and durable — and to make the wrong thing hard to write. Below, each choice is paired with the alternative it beat and the one-line reason.

For where these decisions come from, see [Background & Prior Art](./background.md). For how they fit together at runtime, see [Architecture](./architecture.md). For how to change them safely, see the [Maintainer Guide](./maintainer-guide.md). Terms are in the [Glossary](./glossary.md).

## The stack, with rejected alternatives

| Choice | Version / config | Rationale (one line) | Rejected alternative |
|--------|------------------|----------------------|----------------------|
| **Rust** | edition 2021, `[lib]` only, crate v0.2.0 | Memory safety + performance for the ledger of record; the type system encodes money and invariants | A dynamic language (Python / Frappe) — the soil the implicit-posting monolith grew in |
| **PostgreSQL** | via SQLx | ACID transactions for double-entry; DB-layer idempotency via a *partial* unique index; one schema per tenant | Row-level `tenant_id` in a shared schema; any non-transactional store |
| **SQLx** | 0.8, `runtime-tokio-rustls`, `postgres`, `uuid`, `chrono`, `json`, `migrate`, `rust_decimal` | SQL correctness checked at **build time** against a real Postgres | A heavyweight ORM that hides SQL (Diesel-style macros / active-record) |
| **rust_decimal** | 1.36 | Exact decimal money — never floats | `f64` (rounding error is unacceptable in a ledger) |
| **Axum** | 0.7 (`macros`) + `tower` 0.4 + `tower-http` 0.5 (`cors`, `trace`) | HTTP surface for the 12 CRUD endpoints + `POST /accounting/posts` | Hand-rolled `hyper`; a heavier framework |
| **Tonic / prost** | tonic 0.12, prost 0.13, prost-types, pbjson, tonic-build 0.12 (build-dep) | Present for gRPC + Protobuf, feature-gated behind `grpc` — but **dormant**: the `grpc`/`proto` generators are disabled, so no gRPC is generated today | Shipping gRPC before a consumer needs it |
| **Tokio** | 1 (`full`) | Async runtime for durable, retryable posting | — |
| **thiserror** | — | Typed domain errors | Stringly-typed errors / `anyhow` everywhere (`anyhow` is kept only at the module boundary / builder) |
| **backbone-messaging** | git, branch `main` | Event bus for `AccountingPostPosted` / `AccountingPostFailed` — fire-and-forget | A synchronous callback into producers (would re-couple the seam this module exists to protect) |
| **serde / serde_json / serde_yaml** | serde_yaml 0.9 | Schema YAML parsing + JSON DTOs | — |
| **validator** | 0.16 | Declarative field validation | — |
| **tracing** | + tracing-subscriber (`env-filter`, `json`) | Structured logs | — |
| **backbone-core / -orm / -auth** | git, branch `main`; core with `features=[postgres]` | Framework crates: generic CRUD service/repository, auth, ORM | Reimplementing generic CRUD per module |

**Cargo features:** `default = []`; opt-in `events`, `auth`, `grpc`, `openapi`, `validation`. Everything beyond the base library surface is feature-gated.

## The load-bearing choices

### PostgreSQL is not interchangeable

The idempotency guarantee for posting is enforced *in the database*, not in application code. It is a **partial unique index** on `(company_id, source_type, source_id, posting_type) WHERE posting_status = 'posted'`. Partial indexes are a Postgres-specific feature: they let the database reject a duplicate *posted* entry while still permitting failed/retrying rows for the same source. That single constraint is the reason "why Postgres" is not a swappable decision — it is documented in [ADR-002](./adr/ADR-002-ledger-write-path-integrity.md). The same reasoning drives schema-per-tenant isolation: the tenant boundary lives at the database, not in every query.

### SQLx over an ORM

Queries are checked at compile time against a live Postgres schema. In a ledger, a silently-wrong query is a correctness bug with financial consequences, so the trade — more explicit SQL, less magic — is deliberately in favor of visibility. A macro-heavy ORM that hides the SQL was rejected for exactly the property SQLx gives up: opacity.

### rust_decimal over floats

Money is `rust_decimal`, always. Floating point is never used for amounts. There is no rounding-tolerance to reason about because there is no rounding error to begin with.

### gRPC is present but off — be explicit

`tonic`, `prost`, and `tonic-build` are in `Cargo.toml`, and a `grpc` feature exists. **No gRPC is generated today.** The codegen config disables the `graphql`, `grpc`, and `proto` generators, so the transport is dormant infrastructure, not a shipped interface. Treat it as a reserved capability behind a feature flag, not a current API. Adding a consumer is the trigger to turn it on.

### Tokio + a retry driver, because posting is durable

Posting is not fire-once. An `AccountingPost` carries `retry_count`, `max_retries` (default 3), and `next_retry_at`. That means posting is an async, durable, retryable operation with a scheduling dimension — which is why Tokio is the runtime and why failure is a first-class state (`AccountingPostFailed`) rather than an exception thrown back at the producer.

### Events are fire-and-forget on purpose

The whole reason for the AccountingPost seam (see [Background](./background.md)) is to *decouple* business events from ledger writes. Emitting `AccountingPostPosted` / `AccountingPostFailed` over `backbone-messaging` preserves that decoupling. A synchronous callback into the producing controller was rejected because it would rebuild the exact coupling this module was designed to break.

## The deepest "why" lives upstream

The single most important technology decision — **schema YAML as the source of truth, driving codegen** — is a Metaphor *framework* decision, not a per-module one. It is what makes the runtime-column-mutating dimension engine impossible and the invariants enforceable. That rationale is in [Background & Prior Art](./background.md) and [Philosophy](./philosophy.md). The two hardest technology-shaped decisions that are specific to this module are recorded as accepted ADRs in [`./adr/`](./adr/): the GL/core boundary and the ledger write-path integrity guarantees.
