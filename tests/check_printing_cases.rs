//! Check printing cases — sequence registration, atomic allocation, the
//! registry verbs, and the refusals — against a real Postgres. Requires
//! DATABASE_URL (defaults to the local scratch Postgres on :5433). Each test
//! seeds its own company_id, so tests are isolated and parallel-safe.

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::check_printing_service::{
    AllocationAck, CheckPrintingError, CheckPrintingService, MAX_CHECK_NUMBER, RecordCheck,
    RegisterSequence,
};

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}

fn register(company: Uuid, bank: Uuid, mode: &str, next: i64) -> RegisterSequence {
    RegisterSequence {
        company_id: company,
        bank_account_id: bank,
        numbering_mode: mode.into(),
        next_number: next,
    }
}

fn record(company: Uuid, bank: Uuid, number: Option<String>) -> RecordCheck {
    RecordCheck {
        company_id: company,
        bank_account_id: bank,
        payment_id: Uuid::new_v4(),
        payment_number: Some("PAY-1".into()),
        amount: Decimal::new(250_00, 2),
        payee_name: Some("PT CONTOH".into()),
        check_number: number,
        actor: Some(Uuid::new_v4()),
    }
}

/// Register → allocate → record: the auto happy path hands out consecutive
/// numbers, and the record verb allocates INSIDE its transaction.
#[tokio::test]
async fn auto_sequence_allocates_and_records() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let bank = Uuid::new_v4();
    let svc = CheckPrintingService::new(pool.clone());

    let seq = svc
        .register_sequence(register(company, bank, "auto", 1))
        .await
        .unwrap();
    assert_eq!(seq.numbering_mode, "auto");
    assert_eq!(seq.next_number, 1);

    let alloc: AllocationAck = svc
        .allocate_check_numbers(company, bank, 5)
        .await
        .unwrap();
    assert_eq!(alloc.numbers, vec![1, 2, 3, 4, 5]);
    assert_eq!(alloc.first, 1);
    assert_eq!(alloc.last, 5);

    // The record verb allocates the NEXT number itself (6) — no number
    // supplied by the caller.
    let ack = svc
        .record_printed_check(record(company, bank, None))
        .await
        .unwrap();
    assert_eq!(ack.check_number, "6");
    assert_eq!(ack.status, "printed");
}

/// Two CONCURRENT allocations receive distinct, non-overlapping number blocks:
/// the single-statement UPDATE-RETURNING serializes on the row lock, so there
/// is no read-then-write gap to race through.
#[tokio::test]
async fn concurrent_allocations_get_distinct_numbers() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let bank = Uuid::new_v4();
    let svc = CheckPrintingService::new(pool.clone());
    svc.register_sequence(register(company, bank, "auto", 1))
        .await
        .unwrap();

    let (a, b, c) = tokio::join!(
        svc.allocate_check_numbers(company, bank, 3),
        svc.allocate_check_numbers(company, bank, 3),
        svc.allocate_check_numbers(company, bank, 3),
    );

    let mut all: Vec<i64> = [a.unwrap().numbers, b.unwrap().numbers, c.unwrap().numbers]
        .concat();
    all.sort_unstable();
    assert_eq!(all, vec![1, 2, 3, 4, 5, 6, 7, 8, 9], "every number distinct");

    // The cursor advanced exactly by the total allocated.
    let cursor: i64 = sqlx::query_scalar(
        "SELECT next_number FROM accounting.bank_check_sequences WHERE company_id=$1 AND bank_account_id=$2",
    )
    .bind(company)
    .bind(bank)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cursor, 10);
}

/// A manual (prenumbered stock) sequence validates the officer's number and
/// refuses a cross-payment reuse through the registry's unique constraint.
#[tokio::test]
async fn manual_sequence_validates_and_refuses_duplicates() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let bank = Uuid::new_v4();
    let svc = CheckPrintingService::new(pool.clone());
    svc.register_sequence(register(company, bank, "manual", 1))
        .await
        .unwrap();

    let first = svc
        .record_printed_check(record(company, bank, Some("00100".into())))
        .await
        .unwrap();
    assert_eq!(first.check_number, "00100", "leading zeros preserved");

    // Same number on a DIFFERENT payment → typed duplicate refusal.
    let err = svc
        .record_printed_check(record(company, bank, Some("00100".into())))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "duplicate_check_number");
    assert_eq!(err.http_status(), 422);

    // Malformed numbers refuse before any write.
    for bad in ["", "12A4", "12345678901", "9999999999"] {
        let err = svc
            .record_printed_check(record(company, bank, Some(bad.into())))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_check_number", "number {bad:?}");
    }

    // Supplying a number to an AUTO sequence is a mode conflict; leaving it
    // out of a MANUAL sequence likewise.
    svc.register_sequence(register(company, bank, "auto", 50))
        .await
        .unwrap();
    let err = svc
        .record_printed_check(record(company, bank, Some("77".into())))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "numbering_mode_conflict");

    // Allocating from a MANUAL sequence is a mode conflict too.
    svc.register_sequence(register(company, bank, "manual", 1))
        .await
        .unwrap();
    let err = svc.allocate_check_numbers(company, bank, 1).await.unwrap_err();
    assert_eq!(err.code(), "numbering_mode_conflict");
}

/// An allocation that would cross MAX_INT32 refuses and consumes NOTHING: the
/// transaction rolls the cursor bump back with it, and the numbers that would
/// have crossed remain allocatable afterwards.
#[tokio::test]
async fn overflow_refuses_without_consuming_a_number() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let bank = Uuid::new_v4();
    let svc = CheckPrintingService::new(pool.clone());

    // Two numbers fit below the cap, three would cross it.
    svc.register_sequence(register(company, bank, "auto", MAX_CHECK_NUMBER - 1))
        .await
        .unwrap();

    let err = svc.allocate_check_numbers(company, bank, 3).await.unwrap_err();
    assert_eq!(err.code(), "check_number_overflow");
    assert!(matches!(err, CheckPrintingError::CheckNumberOverflow));

    // The refused allocation left the cursor untouched — nothing consumed.
    let cursor: i64 = sqlx::query_scalar(
        "SELECT next_number FROM accounting.bank_check_sequences WHERE company_id=$1 AND bank_account_id=$2",
    )
    .bind(company)
    .bind(bank)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cursor, MAX_CHECK_NUMBER - 1);

    // The numbers that would have crossed are still allocatable — exactly the
    // two that fit.
    let ok = svc.allocate_check_numbers(company, bank, 2).await.unwrap();
    assert_eq!(ok.numbers, vec![MAX_CHECK_NUMBER - 1, MAX_CHECK_NUMBER]);

    // Past the cap the sequence is exhausted; even one more refuses.
    let err = svc.allocate_check_numbers(company, bank, 1).await.unwrap_err();
    assert_eq!(err.code(), "check_number_overflow");
}

/// Unregistered bank journals and unknown void targets refuse typed.
#[tokio::test]
async fn unregistered_and_unknown_refuse_typed() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let bank = Uuid::new_v4();
    let svc = CheckPrintingService::new(pool.clone());

    let err = svc.allocate_check_numbers(company, bank, 1).await.unwrap_err();
    assert_eq!(err.code(), "sequence_not_registered");
    assert_eq!(err.http_status(), 404);

    let err = svc
        .record_printed_check(record(company, bank, None))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "sequence_not_registered");

    let err = svc
        .void_printed_check(company, Uuid::new_v4(), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "check_not_found");
    assert_eq!(err.http_status(), 404);
}

/// Voiding flips printed → voided exactly once; the number stays consumed.
#[tokio::test]
async fn void_flips_once_and_keeps_the_number_consumed() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let bank = Uuid::new_v4();
    let svc = CheckPrintingService::new(pool.clone());
    svc.register_sequence(register(company, bank, "manual", 1))
        .await
        .unwrap();

    let ack = svc
        .record_printed_check(record(company, bank, Some("00042".into())))
        .await
        .unwrap();

    let voided = svc
        .void_printed_check(company, ack.printed_check_id, Some(Uuid::new_v4()))
        .await
        .unwrap();
    assert_eq!(voided.status, "voided");
    assert_eq!(voided.check_number, "00042");

    // Second void → typed refusal.
    let err = svc
        .void_printed_check(company, ack.printed_check_id, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "check_already_voided");

    // The voided number is still consumed: re-registering it refuses.
    let err = svc
        .record_printed_check(record(company, bank, Some("00042".into())))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "duplicate_check_number");
}

/// Non-positive amounts refuse before any allocation or write.
#[tokio::test]
async fn non_positive_amount_refuses() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let bank = Uuid::new_v4();
    let svc = CheckPrintingService::new(pool.clone());
    svc.register_sequence(register(company, bank, "auto", 1))
        .await
        .unwrap();

    let mut req = record(company, bank, None);
    req.amount = Decimal::ZERO;
    let err = svc.record_printed_check(req).await.unwrap_err();
    assert_eq!(err.code(), "invalid_amount");

    // The refused call allocated nothing.
    let cursor: i64 = sqlx::query_scalar(
        "SELECT next_number FROM accounting.bank_check_sequences WHERE company_id=$1 AND bank_account_id=$2",
    )
    .bind(company)
    .bind(bank)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cursor, 1);
}
