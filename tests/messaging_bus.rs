//! Proves GL-posting events are published to the real backbone-messaging IntegrationEventBus.
//! The adapter unit tests (`publish_async`) are deterministic; the end-to-end test drives a real
//! post through `PostingService::with_sink` (fire-and-forget) and polls the bus history.
//! Requires DATABASE_URL (defaults to local dev Postgres on :5433) for the end-to-end test.

use std::sync::Arc;
use std::time::Duration;

use backbone_messaging::IntegrationEventBus;
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::posting_service::{
    AccountingPostFailed, AccountingPostPosted, PostingEvent, PostingLine, PostingRequest,
    PostingService,
};
use backbone_accounting::infrastructure::messaging::MessagingSink;

fn dec(s: &str) -> Decimal {
    Decimal::from_str_exact(s).unwrap()
}

// ── Adapter unit tests (deterministic) ───────────────────────────────────────
#[tokio::test]
async fn publishes_posted_event_to_bus() {
    let bus = Arc::new(IntegrationEventBus::new());
    let sink = MessagingSink::new(bus.clone());
    let post_id = Uuid::new_v4();
    let evt = PostingEvent::AccountingPostPosted(AccountingPostPosted {
        post_id,
        journal_id: Uuid::new_v4(),
        company_id: Uuid::new_v4(),
        source_type: "order".into(),
        source_id: Uuid::new_v4(),
        total_debit: dec("1110000.00"),
        total_credit: dec("1110000.00"),
        occurred_at: Utc::now(),
    });

    sink.publish_async(evt).await.unwrap();

    let hist = bus.history().await;
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].event_type, "accounting.posting.posted");
    assert_eq!(hist[0].source_context, "accounting");
    assert_eq!(hist[0].aggregate_id, post_id.to_string());
}

#[tokio::test]
async fn publishes_failed_event_to_bus() {
    let bus = Arc::new(IntegrationEventBus::new());
    let sink = MessagingSink::new(bus.clone());
    let source_id = Uuid::new_v4();
    let evt = PostingEvent::AccountingPostFailed(AccountingPostFailed {
        company_id: Uuid::new_v4(),
        source_type: "manual".into(),
        source_id,
        error_code: "unbalanced".into(),
        error_message: "unbalanced".into(),
        occurred_at: Utc::now(),
    });

    sink.publish_async(evt).await.unwrap();

    let hist = bus.history().await;
    assert_eq!(hist.len(), 1);
    assert_eq!(hist[0].event_type, "accounting.posting.failed");
    assert_eq!(hist[0].aggregate_id, source_id.to_string());
}

// ── End-to-end: PostingService → MessagingSink → bus ─────────────────────────
#[tokio::test]
async fn posting_service_publishes_to_bus() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string());
    let pool = PgPool::connect(&url).await.unwrap();

    // Fresh company + bank/revenue accounts.
    let company = Uuid::new_v4();
    let bank = Uuid::new_v4();
    let revenue = Uuid::new_v4();
    for (id, code, name, at, st, nb) in [
        (bank, "1100", "Bank", "asset", "bank", "debit"),
        (revenue, "4000", "Rev", "revenue", "operating_revenue", "credit"),
    ] {
        sqlx::query(
            r#"INSERT INTO accounting.accounts (id, company_id, account_number, account_code, name, account_type,
                account_subtype, normal_balance, is_detail, is_header, status)
               VALUES ($1,$2,$3,$3,$4,$5::account_type,$6::account_subtype,$7::normal_balance,TRUE,FALSE,'active'::account_status)"#,
        )
        .bind(id).bind(company).bind(code).bind(name).bind(at).bind(st).bind(nb)
        .execute(&pool).await.unwrap();
    }

    let bus = Arc::new(IntegrationEventBus::new());
    let svc = PostingService::with_sink(pool.clone(), Arc::new(MessagingSink::new(bus.clone())));

    let mut req = PostingRequest::original(company, "order", Uuid::new_v4(), chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap());
    req.lines = vec![
        PostingLine { account_id: bank, debit: dec("100.00"), credit: Decimal::ZERO, party_type: None, party_id: None, cost_center_id: None, project_id: None, department_id: None, description: None },
        PostingLine { account_id: revenue, debit: Decimal::ZERO, credit: dec("100.00"), party_type: None, party_id: None, cost_center_id: None, project_id: None, department_id: None, description: None },
    ];
    svc.post(req, None).await.unwrap();

    // Fire-and-forget publish runs on a spawned task; poll the bus history briefly.
    let mut found = false;
    for _ in 0..100 {
        if bus.history().await.iter().any(|e| e.event_type == "accounting.posting.posted") {
            found = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(found, "expected AccountingPostPosted on the bus after a real post");
}
