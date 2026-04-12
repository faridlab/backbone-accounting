# Accounting Module Documentation

This directory contains documentation for the accounting bounded context module.

## Documentation Structure

```
docs/
├── README.md           # This file
├── domain.md           # Domain model documentation (generated)
├── brd.md              # Business requirements document
└── openapi/            # API documentation
    ├── README.md       # API overview
    └── openapi.yaml    # OpenAPI specification (generated)
```

## Quick Links

- [Domain Model](./domain.md) - Entity relationships and business rules
- [API Documentation](./openapi/README.md) - REST and gRPC API reference
- [Business Requirements](./brd.md) - Business context and requirements

## Generating Documentation

Documentation is auto-generated from the schema:

```bash
# Generate all documentation
backbone schema generate accounting --target docs

# Generate OpenAPI spec only
backbone schema generate accounting --target openapi
```

## Module Overview

**Bounded Context:** accounting

**Description:** GL, journals, ledger, reconciliation (extracted from bersihir)

### Entities

Entity documentation will be generated after running:
```bash
backbone schema generate accounting --target all
```

### API Endpoints

Each entity provides 11 standard Backbone CRUD endpoints:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/{collection}` | GET | List with pagination, filtering, sorting |
| `/api/v1/{collection}` | POST | Create new entity |
| `/api/v1/{collection}/:id` | GET | Get by ID |
| `/api/v1/{collection}/:id` | PUT | Full update |
| `/api/v1/{collection}/:id` | PATCH | Partial update |
| `/api/v1/{collection}/:id` | DELETE | Soft delete |
| `/api/v1/{collection}/bulk` | POST | Bulk create |
| `/api/v1/{collection}/upsert` | POST | Upsert (create or update) |
| `/api/v1/{collection}/trash` | GET | List deleted items |
| `/api/v1/{collection}/:id/restore` | POST | Restore deleted item |
| `/api/v1/{collection}/empty` | DELETE | Empty trash |
