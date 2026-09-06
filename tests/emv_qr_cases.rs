//! EMV(QRCPS)/QRIS display cases — config upsert, payload render, and the
//! fail-closed refusals — against a real Postgres. Requires DATABASE_URL
//! (defaults to the local scratch Postgres on :5433). Each test seeds its own
//! company_id, so tests are isolated and parallel-safe.

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::emv_qr_service::{
    EmvQrConfigInput, EmvQrServiceError, EmvQrService,
};

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}

fn input(company: Uuid) -> EmvQrConfigInput {
    EmvQrConfigInput {
        company_id: company,
        bank_account_id: None,
        merchant_name: "LAOPAY STORE".into(),
        merchant_city: "JAKARTA".into(),
        country: "id".into(), // upsert normalizes case
        mcc: "6012".into(),
        gui: "ID.CO.QRIS.WWW".into(),
        merchant_identifier: "9360012345678901".into(),
        currency: "IDR".into(),
        initiation: "11".into(),
    }
}

/// The payload is a well-formed merchant-presented TLV: known prefixes, the
/// IDR numeric currency tag, a 4-hex CRC tail, and the reference inside tag 62.
#[tokio::test]
async fn payload_renders_from_saved_config() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = EmvQrService::new(pool.clone());

    let ack = svc.upsert_config(input(company)).await.unwrap();
    assert_eq!(ack.company_id, company);
    assert!(ack.bank_account_id.is_none());

    let payload = svc
        .invoice_payload(
            company,
            None,
            Some(Decimal::new(150000_00, 2)),
            None,
            Some("INV/2026/0001".to_string()),
        )
        .await
        .unwrap();

    // Payload format indicators + point-of-initiation (static).
    assert!(payload.payload.starts_with("000201010211"));
    // QRIS GUI template carries the configured merchant identifier.
    assert!(payload.payload.contains("0014ID.CO.QRIS.WWW"));
    assert!(payload.payload.contains("9360012345678901"));
    // Merchant category code (52) and IDR numeric currency (53 = 360).
    assert!(payload.payload.contains("52046012"));
    assert!(payload.payload.contains("5303360"));
    // Transaction amount (54): 150000.00, dot separator, ≤13 chars.
    assert!(payload.payload.contains("5409150000.00"));
    // Country (58), merchant name (59), city (60).
    assert!(payload.payload.contains("5802ID"));
    assert!(payload.payload.contains("5912LAOPAY STORE"));
    assert!(payload.payload.contains("6007JAKARTA"));
    // Reference rides tag 62 sub-tag 01: 2+2+13 = 17 template chars.
    assert!(payload.payload.contains("62170113INV/2026/0001"));
    // CRC (63) is exactly 4 hex chars at the tail.
    let tail = &payload.payload[payload.payload.len() - 4..];
    assert!(
        tail.chars().all(|c| c.is_ascii_hexdigit()),
        "CRC tail not hex: {tail}"
    );
    assert!(payload.payload.ends_with(&format!("6304{tail}")));
    assert_eq!(payload.currency, "IDR");
    assert_eq!(payload.initiation_method, "11");
}

/// Walk the top-level TLV structure and return the tag ids in order. Panics on
/// malformed input (a well-formed payload is the assertion itself).
fn top_level_tags(payload: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut rest = payload;
    // The trailing CRC value is 4 chars after the "6304" template marker.
    while !rest.is_empty() {
        let tag: String = rest.chars().take(2).collect();
        let len: usize = rest[2..4].parse().expect("TLV length parses");
        let total = 4 + len; // id + length + value, all in characters
        assert!(
            rest.len() >= total,
            "TLV truncated at tag {tag}: {rest:?}"
        );
        tags.push(tag.clone());
        rest = &rest[total..];
    }
    tags
}

/// Omitting the amount renders a static QR with NO tag 54 and NO tag 62.
#[tokio::test]
async fn payload_without_amount_omits_tags_54_and_62() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = EmvQrService::new(pool.clone());
    svc.upsert_config(input(company)).await.unwrap();

    let payload = svc.invoice_payload(company, None, None, None, None).await.unwrap();
    assert!(payload.payload.starts_with("000201010211"));

    // Walk the full payload (the CRC value is tag 63's four chars) and check
    // the top-level tag set.
    let tags = top_level_tags(&payload.payload);
    assert_eq!(tags.first().map(String::as_str), Some("00"));
    assert!(!tags.iter().any(|t| t == "54"), "no amount tag: {tags:?}");
    assert!(!tags.iter().any(|t| t == "62"), "no reference tag: {tags:?}");
    assert_eq!(tags.last().map(String::as_str), Some("63"), "CRC template present: {tags:?}");
}

/// Re-upserting the same slot converges on ONE row (update path, same id).
#[tokio::test]
async fn upsert_is_idempotent_per_slot() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = EmvQrService::new(pool.clone());

    let first = svc.upsert_config(input(company)).await.unwrap();
    let mut second = input(company);
    second.merchant_name = "LAOPAY STORE 2".into();
    let second = svc.upsert_config(second).await.unwrap();

    assert_eq!(first.config_id, second.config_id, "slot must converge on one row");

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounting.emv_qr_configs WHERE company_id=$1")
            .bind(company)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

/// A bank-account-specific config wins over the company default; an unknown
/// slot falls back to the company default.
#[tokio::test]
async fn slot_resolution_prefers_bank_specific_config() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let bank = Uuid::new_v4();
    let svc = EmvQrService::new(pool.clone());

    svc.upsert_config(input(company)).await.unwrap();
    let mut bank_cfg = input(company);
    bank_cfg.bank_account_id = Some(bank);
    bank_cfg.merchant_name = "BANK SLOT SHOP".into();
    svc.upsert_config(bank_cfg).await.unwrap();

    let specific = svc.invoice_payload(company, Some(bank), None, None, None).await.unwrap();
    assert!(specific.payload.contains("BANK SLOT SHOP"));

    let fallback = svc.invoice_payload(company, Some(Uuid::new_v4()), None, None, None)
        .await
        .unwrap();
    assert!(fallback.payload.contains("LAOPAY STORE"));
}

/// No config → typed refusal `qr_config_missing` (404). No fallback payload.
#[tokio::test]
async fn missing_config_refuses_fail_closed() {
    let pool = pool().await;
    let svc = EmvQrService::new(pool.clone());

    let err = svc
        .invoice_payload(Uuid::new_v4(), None, Some(Decimal::ONE), None, Some("R".to_string()))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "qr_config_missing");
    assert_eq!(err.http_status(), 404);
    assert!(matches!(err, EmvQrServiceError::ConfigMissing(_)));
}

/// Validation runs BEFORE the write: an unsupported currency or overlong name
/// refuses at upsert with the domain builder's typed code, and nothing lands.
#[tokio::test]
async fn invalid_config_refuses_before_write() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = EmvQrService::new(pool.clone());

    let mut bad_currency = input(company);
    bad_currency.currency = "XYZ".into();
    let err = svc.upsert_config(bad_currency).await.unwrap_err();
    assert_eq!(err.code(), "currency_not_supported");
    assert_eq!(err.http_status(), 422);

    let mut bad_name = input(company);
    bad_name.merchant_name = "THIS MERCHANT NAME IS FAR TOO LONG FOR EMV".into();
    let err = svc.upsert_config(bad_name).await.unwrap_err();
    assert_eq!(err.code(), "merchant_name_too_long");
    assert_eq!(err.http_status(), 422);

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounting.emv_qr_configs WHERE company_id=$1")
            .bind(company)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0, "refused configs must not land");
}

/// Requesting an unsupported currency at render time refuses too (the config
/// default was valid, the override is not).
#[tokio::test]
async fn render_time_currency_override_is_validated() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = EmvQrService::new(pool.clone());
    svc.upsert_config(input(company)).await.unwrap();

    let err = svc
        .invoice_payload(company, None, Some(Decimal::ONE), Some("XYZ".into()), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "currency_not_supported");
    assert_eq!(err.http_status(), 422);
}
