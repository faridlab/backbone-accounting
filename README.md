# Accounting Module

A complete Domain-Driven Design (DDD) bounded context module built on the **Backbone Framework**. This module follows Clean Architecture principles with a **schema-first** approach where YAML schema files are the single source of truth.

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
# Generate everything
backbone schema generate accounting --target all

# Or generate specific targets
backbone schema generate accounting --target proto,rust,sql
backbone schema generate accounting --target handler,grpc,cqrs
backbone schema generate accounting --target repository,events
```

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

Each entity automatically gets 12 standard endpoints:

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/{collection}` | List with pagination |
| `POST` | `/api/v1/{collection}` | Create |
| `GET` | `/api/v1/{collection}/:id` | Get by ID |
| `PUT` | `/api/v1/{collection}/:id` | Full update |
| `PATCH` | `/api/v1/{collection}/:id` | Partial update |
| `DELETE` | `/api/v1/{collection}/:id` | Soft delete |
| `POST` | `/api/v1/{collection}/bulk` | Bulk create |
| `POST` | `/api/v1/{collection}/upsert` | Upsert |
| `GET` | `/api/v1/{collection}/trash` | List deleted |
| `POST` | `/api/v1/{collection}/:id/restore` | Restore |
| `DELETE` | `/api/v1/{collection}/empty` | Empty trash |
| `GET` | `/api/v1/{collection}/count` | Count records |

## Development Workflow

### Adding a New Entity

1. Create schema file: `schema/models/{entity}.model.yaml`
2. Run generator: `backbone schema generate accounting --target all`
3. Run migrations: `sqlx migrate run`
4. Test endpoints

### Modifying an Entity

1. Update schema file
2. Generate migration: `backbone migration alter {Entity} accounting -d "description"`
3. Regenerate code: `backbone schema generate accounting --target all`
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

- [Framework Documentation](../../docs/technical/FRAMEWORK.md)
- [API Guidelines](../../docs/technical/API_GUIDELINES.md)
- [Quick Start Guide](../../docs/technical/QUICKSTART.md)

## License

Part of the Backbone Framework.
