# Database Migrations for accounting

This directory contains SQLx database migrations for the accounting module. These migrations create a comprehensive database schema with support for:

- Main entity storage
- Event sourcing
- Audit trails
- Performance optimization
- Analytics and reporting

## Migration Files

### 001_create_accounting_table.sql
Creates the main `accounting` table with:
- UUID primary key with automatic generation
- Core fields: name, description, status, metadata
- Timestamps: created_at, updated_at, deleted_at (soft delete)
- Constraints and validation checks
- Performance indexes for common queries
- Full-text search capabilities
- Automatic updated_at trigger

### 002_create_accounting_events_table.sql
Creates the event store table `accounting_events` with:
- Event sourcing support for domain events
- Foreign key relationship to main table
- Event type constraints
- Versioning support for event schemas
- Optimized indexes for event replay
- Metadata for correlation and tracing

### 003_create_accounting_audit_table.sql
Creates the audit trail table `accounting_audit` with:
- Comprehensive change tracking (INSERT, UPDATE, DELETE, SOFT_DELETE, RESTORE)
- Before/after state capture
- User context tracking (ID, email, IP, user agent)
- Request correlation IDs
- Changed field tracking
- Optimized indexes for audit queries

### 004_create_accounting_indexes_and_triggers.sql
Creates additional performance optimizations:
- Audit trigger functions
- Automatic audit logging triggers
- Performance indexes for common query patterns
- Full-text search function
- Statistics aggregation function
- Partial indexes for active records
- JSON-specific indexes for metadata queries

### 005_create_accounting_views.sql
Creates optimized database views:
- `accounting_active` - Only active entities
- `accounting_stats` - Materialized statistics view
- `accounting_with_activity` - Entities with recent activity
- `accounting_audit_summary` - Audit summary per entity
- `accounting_event_timeline` - Chronological event view
- `accounting_audit_timeline` - Chronological audit view
- `accounting_by_metadata` - Entities with extracted metadata
- `accounting_performance_metrics` - Daily performance metrics

## Running Migrations

### Using SQLx CLI

```bash
# Install sqlx-cli if not already installed
cargo install sqlx-cli --no-default-features --features native-tls,postgres

# Run all pending migrations
sqlx migrate run --database-url "postgresql://user:password@localhost:5432/database"

# Run specific migration (for development)
sqlx migrate run --database-url "postgresql://user:password@localhost:5432/database" -m 001

# Check migration status
sqlx migrate info --database-url "postgresql://user:password@localhost:5432/database"
```

### Using Application Code

```rust
use sqlx::migrate;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Run migrations
    migrate!("./migrations").run(&pool).await?;

    println!("Migrations completed successfully!");
    Ok(())
}
```

## Database Schema Overview

```
accounting
├── Main Table: accounting
├── Event Store: accounting_events
├── Audit Trail: accounting_audit
├── Views (Optimized):
│   ├── accounting_active
│   ├── accounting_stats (materialized)
│   ├── accounting_with_activity
│   ├── accounting_audit_summary
│   ├── accounting_event_timeline
│   ├── accounting_audit_timeline
│   ├── accounting_by_metadata
│   └── accounting_performance_metrics (materialized)
└── Functions:
    ├── search_accounting()
    ├── get_accounting_statistics()
    ├── refresh_accounting_materialized_views()
    └── schedule_accounting_refresh()
```

## Performance Optimizations

### Indexes
- **Primary indexes** on foreign keys and frequently queried fields
- **Composite indexes** for common query patterns (status + created_at)
- **Partial indexes** for active records (WHERE deleted_at IS NULL)
- **GIN indexes** for JSONB and full-text search
- **Functional indexes** for time-based queries

### Materialized Views
- **Statistics view** with pre-aggregated counts
- **Performance metrics** with daily aggregations
- **Refresh functions** for updating materialized views

### Triggers
- **Automatic audit logging** for all DML operations
- **Updated_at timestamp** maintenance
- **Event publishing** for domain events

## Migration Best Practices

### Development
1. **Always test migrations** on a copy of production data
2. **Use `CONCURRENTLY`** for creating indexes on production
3. **Add rollback scripts** for each migration
4. **Document breaking changes** in migration comments

### Production
1. **Run migrations during maintenance windows**
2. **Monitor performance** after migration
3. **Backup database** before major migrations
4. **Use read replicas** to minimize downtime

### Performance Considerations
1. **Batch large operations** to avoid long-running transactions
2. **Use `CONCURRENTLY`** for index creation on large tables
3. **Monitor temp space** during complex migrations
4. **Test with production data volumes**

## Security Considerations

### Row-Level Security (Optional)
```sql
-- Example RLS policies can be added as separate migrations
CREATE POLICY accounting_read_policy ON accounting
    FOR SELECT USING (deleted_at IS NULL);

ALTER TABLE accounting ENABLE ROW LEVEL SECURITY;
```

### Audit Context
The audit system relies on application context being set:
```sql
-- Set context variables in your application
SET LOCAL app.current_user_id = '550e8400-e29b-41d4-a716-446655440000';
SET LOCAL app.current_user_email = 'user@example.com';
SET LOCAL app.current_ip = '192.168.1.1';
SET LOCAL app.current_request_id = 'req_123456';
```

## Monitoring and Maintenance

### Regular Tasks
1. **Refresh materialized views** regularly
2. **Monitor index usage** and remove unused indexes
3. **Archive old audit records** based on retention policy
4. **Update statistics** for query optimizer

### Health Checks
```sql
-- Check materialized view freshness
SELECT schemaname, matviewname, last_refresh
FROM pg_matviews
WHERE matviewname LIKE 'accounting_%';

-- Check index usage
SELECT schemaname, tablename, indexname, idx_scan, idx_tup_read
FROM pg_stat_user_indexes
WHERE tablename LIKE 'accounting_%';
```

## Troubleshooting

### Common Issues
1. **Migration timeouts** - Increase statement timeout
2. **Lock contention** - Use `CONCURRENTLY` for indexes
3. **Disk space** - Monitor temp space during migrations
4. **Rollback failures** - Ensure no dependent objects

### Recovery
```sql
-- Rollback specific migration
sqlx migrate revert --database-url "postgresql://user:password@localhost:5432/database"

-- Reset and re-run (development only)
sqlx migrate revert --database-url "postgresql://user:password@localhost:5432/database" --all
sqlx migrate run --database-url "postgresql://user:password@localhost:5432/database"
```

## Next Steps

After running migrations:

1. **Update application configuration** with database connection details
2. **Test all application features** with the new schema
3. **Monitor query performance** and add indexes as needed
4. **Set up scheduled jobs** for materialized view refreshes
5. **Configure monitoring** for database health and performance