# Accounting Module

A complete Domain-Driven Design (DDD) bounded context module built on the **Backbone Framework**. This module follows Clean Architecture principles with a **schema-first** approach where YAML schema files are the single source of truth.

> 📖 **Full handbook:** [`docs/`](./docs/README.md) — philosophy, architecture (C4), maintainer &
> developer guides, contribution guide, glossary, and ADRs. This README is the quickstart; the
> handbook is the depth.

## Architecture Overview

```
accounting/
├── schema/                          # SCHEMA DEFINITIONS (Single Source of Truth)
│   ├── models/                     # Entity schema definitions
│   │   ├── index.model.yaml       # Module configuration and shared types
│   │   └── {entity}.model.yaml    # Entity definitions
│   ├── hooks/                      # Event hooks and triggers
│   │   └── index.hook.yaml        # Events and scheduled jobs
│   ├── workflows/                  # Business workflows
│   │   └── README.md              # Workflow documentation
│   └── openapi/                    # OpenAPI specifications
│       └── index.openapi.yaml     # API documentation
│
├── src/                            # SOURCE CODE (Generated + Custom)
│   ├── domain/                    # Domain Layer (generated)
│   │   ├── entity/               # Entity implementations
│   │   ├── value_object/         # Value objects
│   │   ├── event/                # Domain events
│   │   └── mod.rs
│   │
│   ├── application/               # Application Layer (generated)
│   │   ├── {entity}_services.rs  # Application services
│   │   └── mod.rs
│   │
│   ├── infrastructure/            # Infrastructure Layer (generated)
│   │   ├── persistence/          # Repository implementations
│   │   │   └── postgres/
│   │   └── mod.rs
│   │
│   ├── presentation/              # Presentation Layer (generated)
│   │   ├── http/                 # REST handlers
│   │   ├── grpc/                 # gRPC services
│   │   └── mod.rs
│   │
│   └── lib.rs                    # Module entry point
│
├── migrations/                    # DATABASE MIGRATIONS
│   └── postgres/                 # PostgreSQL migrations (generated)
│
├── Cargo.toml                    # Dependencies
└── README.md                     # This file
```

## Quick Start

### 1. Define Your Schema

Create entity schema files in `schema/models/`:

```yaml
# schema/models/example.model.yaml
name: Example
table_name: examples
collection: examples

fields:
  id:
    type: uuid
    attributes: ["@id", "@default(uuid)"]
  name:
    type: string
    attributes: ["@required"]
    validation:
      min_length: 1
      max_length: 255
  description:
    type: text
    attributes: ["@nullable"]
  status:
    type: enum
    enum_values: [active, inactive, pending]
    attributes: ["@default(active)"]
  created_at:
    type: datetime
    attributes: ["@default(now)"]
  updated_at:
    type: datetime
    attributes: ["@default(now)", "@updated_at"]
  deleted_at:
    type: datetime
    attributes: ["@nullable", "@soft_delete"]

indexes:
  - name: idx_examples_status
    fields: [status]
  - name: idx_examples_created_at
    fields: [created_at]

permissions:
  create: ["admin", "editor"]
  read: ["admin", "editor", "viewer"]
  update: ["admin", "editor"]
  delete: ["admin"]
```

### 2. Generate Code

Run the schema generator to create all code from your schema:

```bash
# Validate the schema first
metaphor schema schema validate

# Generate everything
metaphor schema schema generate accounting --target all

# Or generate specific targets
metaphor schema schema generate accounting --target rust,sql
metaphor schema schema generate accounting --target handler,repository,events
```

> The canonical CLI is **`metaphor`** (v0.2.0); schema codegen is the nested passthrough
> `metaphor schema schema <op>`. The `backbone` name used elsewhere is a local dev alias for the
> framework CLI. Note: the `grpc`, `proto`, and `graphql` generators are **disabled** in
> `schema/models/index.model.yaml`, so those targets currently produce nothing.

### 3. Run Migrations

```bash
# Run PostgreSQL migrations
sqlx migrate run --source migrations/postgres
```

### 4. Use the Module

```rust
use backbone_accounting::AccountingModule;

// Create module instance with builder pattern
let module = AccountingModule::builder()
    .with_database(pool)
    .build()?;

// Get routes
let routes = module.routes();
```

## Schema Generation Targets

| Target | Description | Generated Files |
|--------|-------------|-----------------|
| `proto` | Protocol Buffer definitions | `*.proto` files |
| `rust` | Rust entity structs | `domain/entity/*.rs` |
| `sql` | PostgreSQL migrations | `migrations/postgres/*.sql` |
| `repository` | Repository implementations | `infrastructure/persistence/*.rs` |
| `cqrs` | Commands and queries | `application/commands/*.rs`, `application/queries/*.rs` |
| `handler` | HTTP handlers | `presentation/http/*.rs` |
| `grpc` | gRPC services | `presentation/grpc/*.rs` |
| `events` | Domain events | `domain/event/*.rs` |
| `value-object` | Value objects | `domain/value_object/*.rs` |
| `validator` | Validation rules | `domain/validator/*.rs` |
| `state-machine` | State machines | `domain/state_machine/*.rs` |
| `permission` | Permission definitions | `domain/permission/*.rs` |
| `trigger` | Database triggers | `infrastructure/trigger/*.rs` |
| `openapi` | OpenAPI specifications | `schema/openapi/*.yaml` |
| `all` | All of the above | Everything |

## Standard CRUD Endpoints

Each entity automatically gets the 12 standard `BackboneCrudHandler` endpoints:

| # | Endpoint | Description |
|---|----------|-------------|
| 1 | `list` | List with pagination, filtering, sorting |
| 2 | `create` | Create |
| 3 | `get` / `find_by_id` | Get by ID |
| 4 | `update` | Full update (PUT) |
| 5 | `patch` | Partial update (PATCH) |
| 6 | `soft_delete` | Soft delete |
| 7 | `restore` | Restore a soft-deleted record |
| 8 | `empty_trash` | Permanently purge soft-deleted records |
| 9 | `bulk_create` | Bulk create |
| 10 | `upsert` | Upsert |
| 11 | `list_deleted` | List soft-deleted records |
| 12 | `find_by_id` | Fetch a single record by id |

> **Guarded in production.** Posted GL entities (`Journal`, `JournalLine`, `Ledger`,
> `AccountingPost`) are mounted **read-only** via `create_guarded_accounting_routes` — the only
> sanctioned GL writer is `POST /accounting/posts` (see
> [ADR-002](./docs/adr/ADR-002-ledger-write-path-integrity.md)). Master/config entities keep full
> CRUD. `JournalLine` and `ReconciliationItem` are cascade children with no `@audit_metadata`, so
> their soft-delete family (`soft_delete`/`restore`/`empty_trash`/`list_deleted`) is not generated.

## Development Workflow

### Adding a New Entity

1. Create schema file: `schema/models/{entity}.model.yaml`
2. Run generator: `metaphor schema schema generate accounting --target all`
3. Create/run migrations: `metaphor migration create {entity}`
4. Register the new service in the `AccountingModule` builder (`src/lib.rs`)
5. Test endpoints

See the [Maintainer Guide](./docs/maintainer-guide.md) for the full add-a-feature walkthrough.

### Modifying an Entity

1. Update the schema file
2. Generate a migration: `metaphor migration create {change}`
3. Regenerate code: `metaphor schema schema generate accounting --target all`
4. Run migrations

### Custom Business Logic

Add custom logic in the generated service files. The generator preserves custom code in marked sections.

## Testing

```bash
# Run all tests
cargo test --package backbone-accounting

# Run with database
DATABASE_URL=postgresql://... cargo test --package backbone-accounting
```

## Configuration

Environment variables:
- `DATABASE_URL` - PostgreSQL connection string
- `RUST_LOG` - Log level (trace, debug, info, warn, error)

## Dependencies

This module depends on:
- `backbone-core` - Core framework utilities
- `backbone-orm` - ORM and database traits
- `backbone-auth` - Authentication and authorization
- `backbone-messaging` - Event messaging

## Documentation

The full handbook lives in [`docs/`](./docs/README.md):

- [Philosophy & motivation](./docs/philosophy.md) · [Background & prior art](./docs/background.md) · [Technology & the why](./docs/technology.md)
- [Architecture (C4)](./docs/architecture.md) · [Maintainer Guide](./docs/maintainer-guide.md) · [Developer Guide](./docs/developer-guide.md)
- [Extension Guide](./docs/extension-guide.md) · [Contributing](./docs/contributing.md) · [Glossary](./docs/glossary.md) · [ADRs](./docs/adr/README.md)
- Specs: [BRD](./docs/brd.md) · [PRD](./docs/prd.md) · [FSD](./docs/fsd.md)

## License

Part of the Backbone Framework.
