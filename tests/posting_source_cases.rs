//! Every posting source the database holds is one the module can read.
//!
//! A source added by a hand-written migration (`ALTER TYPE ... ADD VALUE`) and never added to
//! the schema made every read that met such a post fail whole: a final settlement posted, and
//! the accounting posts list answered 500 from then on. Each value of the database type is
//! decoded here through sqlx, the path a row read takes, so a value the schema lacks fails
//! this test instead of a screen.
//!
//! Requires DATABASE_URL with the module's migrations applied.

use backbone_accounting::domain::entity::PostingSourceType;
use sqlx::PgPool;

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.unwrap()
}

#[tokio::test]
async fn every_posting_source_in_the_database_reads_back() {
    let pool = pool().await;
    let labels: Vec<String> = sqlx::query_scalar(
        "SELECT e.enumlabel::text FROM pg_enum e JOIN pg_type t ON t.oid = e.enumtypid
          WHERE t.typname = 'posting_source_type' ORDER BY e.enumsortorder",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(labels.iter().any(|l| l == "final_settlement"), "the migrations add final_settlement");

    for label in &labels {
        let read: Result<PostingSourceType, _> =
            sqlx::query_scalar("SELECT $1::text::posting_source_type")
                .bind(label)
                .fetch_one(&pool)
                .await;
        let value = read.unwrap_or_else(|e| panic!("posting source '{label}' cannot be read: {e}"));
        assert_eq!(value.to_string(), *label, "'{label}' reads back as itself");
    }
}
