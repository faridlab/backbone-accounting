//! Golden cases for the cash-basis (on_payment) tax exigibility flip.
//!
//! Runs against a real Postgres (DATABASE_URL, defaults to :5433/backbone_accounting).
//! Each test seeds its own company + chart of accounts — isolated and parallel-safe.
//!
//! The deferral lookup is host-implemented in production; these tests pin the
//! accounting-side machinery with an in-test port that answers for `order`
//! documents only (the same producer gate the billing-backed host applies).

use std::sync::Arc;

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::posting_service::{
    PostingLine, PostingRequest, PostingService,
};
use backbone_accounting::application::service::reconcile_write_service::ReconcileWriteService;
use backbone_accounting::domain::reconcile_graph::{LineLocator, PairRequest, ORIGIN_SETTLEMENT};
use backbone_accounting::domain::repositories::{DeferredTaxLine, DeferredTaxLookup};
use backbone_accounting::infrastructure::persistence::{
    SqlxPostingRepository, SqlxReconcileGraphRepository,
};

fn dec(s: &str) -> Decimal {
    Decimal::from_str_exact(s).unwrap()
}

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_accounting".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}

/// Seed a fresh company with a cash-basis chart: AR/Bank settlement accounts,
/// revenue, a reconcilable tax TRANSITION account, the REAL tax account, an FX
/// account, and an expense account. Returns (company, code→account id).
async fn seed(pool: &PgPool) -> (Uuid, std::collections::HashMap<&'static str, Uuid>) {
    let company_id = Uuid::new_v4();
    // (code, name, type, subtype, normal_balance, reconcilable)
    let coa: &[(&str, &str, &str, &str, &str, bool)] = &[
        ("1100", "Bank", "asset", "bank", "debit", true),
        ("1200", "AR", "asset", "accounts_receivable", "debit", true),
        (
            "4000",
            "Revenue",
            "revenue",
            "operating_revenue",
            "credit",
            false,
        ),
        (
            "4900",
            "FX Gain/Loss",
            "other_income",
            "operating_revenue",
            "credit",
            false,
        ),
        // The transition account must be reconcilable: the flip's derived pairing
        // settles against it.
        ("2300", "PPN Transition", "liability", "tax", "credit", true),
        ("2310", "PPN Real", "liability", "tax", "credit", false),
        (
            "5000",
            "Expense",
            "expense",
            "operating_expense",
            "debit",
            false,
        ),
    ];
    let mut map = std::collections::HashMap::new();
    for (code, name, at, st, nb, reconcilable) in coa {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO accounting.accounts
                (id, company_id, account_number, account_code, name, account_type, account_subtype,
                 normal_balance, is_header, is_detail, status, is_reconcilable)
               VALUES ($1,$2,$3,$4,$5,$6::account_type,$7::account_subtype,$8::normal_balance,
                       FALSE,TRUE,'active'::account_status,$9)"#,
        )
        .bind(id)
        .bind(company_id)
        .bind(code)
        .bind(code)
        .bind(name)
        .bind(at)
        .bind(st)
        .bind(nb)
        .bind(reconcilable)
        .execute(pool)
        .await
        .expect("seed account");
        map.insert(*code, id);
    }
    (company_id, map)
}

fn posting_svc(pool: &PgPool) -> PostingService {
    PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone())))
}

fn reconcile_svc(pool: &PgPool, deferred: Vec<DeferredTaxLine>) -> ReconcileWriteService {
    ReconcileWriteService::new(
        Arc::new(SqlxReconcileGraphRepository::new()),
        Arc::new(SqlxPostingRepository::new(pool.clone())),
        pool.clone(),
        None,
    )
    .with_deferred_tax(Arc::new(OrderDeferredTax { lines: deferred }))
}

/// In-test deferral lookup: answers only for `order` documents (the billing
/// producer), everything else — payments included — carries no deferrals.
struct OrderDeferredTax {
    lines: Vec<DeferredTaxLine>,
}

#[async_trait::async_trait]
impl DeferredTaxLookup for OrderDeferredTax {
    async fn deferred_lines_on(
        &self,
        _conn: &mut sqlx::PgConnection,
        _company_id: Uuid,
        _journal_id: Uuid,
        source_type: Option<&str>,
        _source_id: Option<Uuid>,
    ) -> anyhow::Result<Vec<DeferredTaxLine>> {
        if source_type == Some("order") {
            Ok(self.lines.clone())
        } else {
            Ok(Vec::new())
        }
    }
}

fn line(account_id: Uuid, debit: &str, credit: &str, party: Option<Uuid>) -> PostingLine {
    PostingLine {
        account_id,
        debit: dec(debit),
        credit: dec(credit),
        party_type: party.map(|_| "customer".to_string()),
        party_id: party,
        cost_center_id: None,
        project_id: None,
        department_id: None,
        description: None,
    }
}

async fn post(
    svc: &PostingService,
    company: Uuid,
    source_type: &str,
    source_id: Uuid,
    lines: Vec<PostingLine>,
) -> Uuid {
    let mut req = PostingRequest::original(
        company,
        source_type,
        source_id,
        chrono::NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
    );
    req.lines = lines;
    svc.post(req, None).await.expect("post").journal_id
}

async fn line_id(
    pool: &PgPool,
    company: Uuid,
    source_type: &str,
    source_id: Uuid,
    account_id: Uuid,
) -> Uuid {
    sqlx::query_scalar(
        "SELECT l.id FROM accounting.journal_lines l \
         JOIN accounting.journals j ON j.id = l.journal_id \
         WHERE l.company_id=$1 AND l.source_type=$2 AND l.source_id=$3 AND l.account_id=$4 \
         ORDER BY l.id LIMIT 1",
    )
    .bind(company)
    .bind(source_type)
    .bind(source_id)
    .bind(account_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Signed ledger balance of one account (debits positive).
async fn net(pool: &PgPool, company: Uuid, account: Uuid) -> Decimal {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(l.base_debit_amount - l.base_credit_amount), 0) \
         FROM accounting.journal_lines l WHERE l.company_id=$1 AND l.account_id=$2",
    )
    .bind(company)
    .bind(account)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Count the flip journals generated for a company (stamped `caba:`).
async fn flip_journal_count(pool: &PgPool, company: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.journals \
         WHERE company_id=$1 AND source_type='reconciliation' \
           AND source_reference LIKE 'caba:%'",
    )
    .bind(company)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn pair(company: Uuid, d: LineLocator, c: LineLocator, amount: &str) -> PairRequest {
    PairRequest {
        company_id: company,
        debit: d,
        credit: c,
        amount: dec(amount),
        origin: ORIGIN_SETTLEMENT.to_string(),
        actor: None,
    }
}

fn loc(source_type: &str, source_id: Uuid, account_id: Uuid) -> LineLocator {
    LineLocator::new(source_type, source_id, account_id)
}

/// An invoice whose sales tax was deferred at post: Dr AR / Cr Revenue / Cr
/// TRANSITION (the deferral). Returns (invoice ref, transition line id).
async fn post_deferred_invoice(
    svc: &PostingService,
    pool: &PgPool,
    company: Uuid,
    coa: &std::collections::HashMap<&'static str, Uuid>,
    party: Uuid,
    base: &str,
    tax: &str,
) -> (Uuid, Uuid) {
    let invoice = Uuid::new_v4();
    let revenue = dec(base) - dec(tax);
    post(
        svc,
        company,
        "order",
        invoice,
        vec![
            line(coa[&"1200"], base, "0", Some(party)),
            line(coa[&"4000"], "0", &revenue.to_string(), None),
            line(coa[&"2300"], "0", tax, None),
        ],
    )
    .await;
    let tr_line = line_id(pool, company, "order", invoice, coa[&"2300"]).await;
    (invoice, tr_line)
}

/// Receipt (Dr Bank / Cr AR) for `party`.
async fn post_receipt(
    svc: &PostingService,
    company: Uuid,
    bank: Uuid,
    ar: Uuid,
    party: Uuid,
    amount: &str,
) -> Uuid {
    let payment = Uuid::new_v4();
    post(
        svc,
        company,
        "payment",
        payment,
        vec![
            line(bank, amount, "0", None),
            line(ar, "0", amount, Some(party)),
        ],
    )
    .await;
    payment
}

fn deferred_line(
    tr_line: Uuid,
    coa: &std::collections::HashMap<&'static str, Uuid>,
    tax: &str,
) -> DeferredTaxLine {
    DeferredTaxLine {
        source_line_id: tr_line,
        transition_account_id: coa[&"2300"],
        real_account_id: coa[&"2310"],
        amount: dec(tax),
        // Output tax on a sale: the transition line is a credit.
        is_debit: false,
    }
}

// CABA-1: a 40% payment flips exactly 40% of the deferred tax onto the real
// account; the transition account retains the 60% residual; the component does
// NOT complete (the transition line is still open).
#[tokio::test]
async fn caba1_partial_flips_pro_rata() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let (invoice, tr_line) =
        post_deferred_invoice(&posting, &pool, company, &coa, party, "1000", "115").await;
    let payment = post_receipt(&posting, company, coa[&"1100"], coa[&"1200"], party, "400").await;

    let svc = reconcile_svc(&pool, vec![deferred_line(tr_line, &coa, "115")]);
    let out = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa[&"1200"]),
            loc("payment", payment, coa[&"1200"]),
            "400",
        ))
        .await
        .unwrap();
    assert_eq!(out.applied, dec("400"));

    // 40% of 115 = 46 on the real account; the transition keeps 69.
    assert_eq!(net(&pool, company, coa[&"2310"]).await, dec("-46.00"));
    assert_eq!(net(&pool, company, coa[&"2300"]).await, dec("-69.00"));
    // Partial settlement: no full-reconcile group yet.
    assert!(
        out.full_reconcile_id.is_none(),
        "transition still open — component must not complete"
    );
    assert_eq!(flip_journal_count(&pool, company).await, 1);
}

// CABA-2: a full payment flips the whole deferral; the transition account nets
// zero and the component completes into a full-reconcile group.
#[tokio::test]
async fn caba2_full_flips_full() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let (invoice, tr_line) =
        post_deferred_invoice(&posting, &pool, company, &coa, party, "1000", "115").await;
    let payment = post_receipt(&posting, company, coa[&"1100"], coa[&"1200"], party, "1000").await;

    let svc = reconcile_svc(&pool, vec![deferred_line(tr_line, &coa, "115")]);
    let out = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa[&"1200"]),
            loc("payment", payment, coa[&"1200"]),
            "1000",
        ))
        .await
        .unwrap();
    assert_eq!(out.applied, dec("1000"));
    assert_eq!(net(&pool, company, coa[&"2300"]).await, Decimal::ZERO);
    assert_eq!(net(&pool, company, coa[&"2310"]).await, dec("-115"));
    assert!(out.full_reconcile_id.is_some(), "everything reached zero");
}

// CABA-3: unlinking the payment partial REVERSES the flip (a real reversal
// journal, never a bare delete) — the transition account is restored to its
// full deferral and the derived caba_pair edge is gone with its parent.
#[tokio::test]
async fn caba3_unreconcile_reverses_and_restores_transition() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let (invoice, tr_line) =
        post_deferred_invoice(&posting, &pool, company, &coa, party, "1000", "115").await;
    let payment = post_receipt(&posting, company, coa[&"1100"], coa[&"1200"], party, "400").await;

    let svc = reconcile_svc(&pool, vec![deferred_line(tr_line, &coa, "115")]);
    let out = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa[&"1200"]),
            loc("payment", payment, coa[&"1200"]),
            "400",
        ))
        .await
        .unwrap();
    let partial = out.partial_id.unwrap();

    // The flip journal is stamped with the partial — the unlink walk reverses
    // every journal carrying that stamp.
    let ar_line = line_id(&pool, company, "order", invoice, coa[&"1200"]).await;
    let flip_journal: Uuid = sqlx::query_scalar(
        "SELECT j.id FROM accounting.journals j \
         JOIN accounting.accounting_posts ap ON ap.journal_id = j.id \
         WHERE j.company_id=$1 AND ap.idempotency_key=$2",
    )
    .bind(company)
    .bind(format!("caba:{partial}:{ar_line}"))
    .fetch_one(&pool)
    .await
    .unwrap();

    svc.unreconcile(company, partial, None).await.unwrap();

    // Reversal journal exists and both tax accounts are back to pre-payment.
    let _rev: Uuid = sqlx::query_scalar(
        "SELECT j.id FROM accounting.journals j \
         JOIN accounting.accounting_posts ap ON ap.journal_id = j.id \
         WHERE j.company_id=$1 AND ap.idempotency_key=$2",
    )
    .bind(company)
    .bind(format!("unlink:{flip_journal}"))
    .fetch_one(&pool)
    .await
    .expect("flip reversal journal");
    assert_eq!(net(&pool, company, coa[&"2300"]).await, dec("-115"));
    assert_eq!(net(&pool, company, coa[&"2310"]).await, Decimal::ZERO);
    let caba_pairs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.partial_reconciles \
         WHERE company_id=$1 AND metadata->>'rule'='caba_pair'",
    )
    .bind(company)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        caba_pairs, 0,
        "derived caba_pair edges die with their parent"
    );
}

// CABA-4: re-requesting an already-consumed settlement clamps to zero — no
// second flip journal, no double-posting.
#[tokio::test]
async fn caba4_flip_idempotent() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let (invoice, tr_line) =
        post_deferred_invoice(&posting, &pool, company, &coa, party, "1000", "115").await;
    let payment = post_receipt(&posting, company, coa[&"1100"], coa[&"1200"], party, "1000").await;

    let svc = reconcile_svc(&pool, vec![deferred_line(tr_line, &coa, "115")]);
    let req = pair(
        company,
        loc("order", invoice, coa[&"1200"]),
        loc("payment", payment, coa[&"1200"]),
        "1000",
    );
    let out = svc.reconcile_pair(&req).await.unwrap();
    assert_eq!(out.applied, dec("1000"));

    // Same settlement again: clamped to zero, no new partial, no new flip.
    let again = svc.reconcile_pair(&req).await.unwrap();
    assert_eq!(again.applied, Decimal::ZERO);
    assert!(again.partial_id.is_none());
    assert_eq!(flip_journal_count(&pool, company).await, 1);
    assert_eq!(net(&pool, company, coa[&"2310"]).await, dec("-115"));
}

// CABA-5: a partial on lines whose subtype is NOT receivable/payable never
// consults the deferral port — bank-to-bank settlement has no document tax.
#[tokio::test]
async fn caba5_non_rp_partial_no_flip() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let posting = posting_svc(&pool);
    // Two `order` documents (the producer the fake port answers for) whose
    // paired lines sit on the BANK account, not AR/AP.
    let a = Uuid::new_v4();
    post(
        &posting,
        company,
        "order",
        a,
        vec![
            line(coa[&"1100"], "100", "0", None),
            line(coa[&"4000"], "0", "100", None),
        ],
    )
    .await;
    let b = Uuid::new_v4();
    post(
        &posting,
        company,
        "order",
        b,
        vec![
            line(coa[&"5000"], "100", "0", None),
            line(coa[&"1100"], "0", "100", None),
        ],
    )
    .await;

    let svc = reconcile_svc(&pool, vec![deferred_line(Uuid::new_v4(), &coa, "115")]);
    let out = svc
        .reconcile_pair(&pair(
            company,
            loc("order", a, coa[&"1100"]),
            loc("order", b, coa[&"1100"]),
            "100",
        ))
        .await
        .unwrap();
    assert_eq!(out.applied, dec("100"));
    assert_eq!(flip_journal_count(&pool, company).await, 0);
    assert_eq!(net(&pool, company, coa[&"2310"]).await, Decimal::ZERO);
}

// CABA-6: an unwired port (host without a tax module) degrades to no flips —
// the reconciliation itself succeeds untouched.
#[tokio::test]
async fn caba6_unwired_port_no_flip() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let (invoice, _tr_line) =
        post_deferred_invoice(&posting, &pool, company, &coa, party, "1000", "115").await;
    let payment = post_receipt(&posting, company, coa[&"1100"], coa[&"1200"], party, "400").await;

    let svc = ReconcileWriteService::new(
        Arc::new(SqlxReconcileGraphRepository::new()),
        Arc::new(SqlxPostingRepository::new(pool.clone())),
        pool.clone(),
        None,
    );
    let out = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa[&"1200"]),
            loc("payment", payment, coa[&"1200"]),
            "400",
        ))
        .await
        .unwrap();
    assert_eq!(out.applied, dec("400"));
    assert_eq!(flip_journal_count(&pool, company).await, 0);
    assert_eq!(net(&pool, company, coa[&"2300"]).await, dec("-115"));
}

// CABA-7: the flip rides the SAME partial as an exchange-difference move; both
// journals post, and unlinking reverses BOTH (each nets zero with its
// reversal) — the two generated-journal mechanisms do not interfere.
#[tokio::test]
async fn caba7_flip_rides_unlink_closure_alongside_exchange() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let (invoice, tr_line) =
        post_deferred_invoice(&posting, &pool, company, &coa, party, "100", "11.5").await;

    // Invoice books at rate 1.1 in base: AR 110 and revenue 98.5 (transition
    // stays 11.5) — journal balanced, payment books 100 at rate 1.0. The 10
    // gap is the exchange difference; the flip pro-rates at 100/110.
    for (account, base_col, base) in [
        (coa[&"1200"], "base_debit_amount", "110"),
        (coa[&"4000"], "base_credit_amount", "98.5"),
    ] {
        sqlx::query(&format!(
            "UPDATE accounting.journal_lines SET exchange_rate=1.1, {base_col}={base} \
             WHERE id = (SELECT l.id FROM accounting.journal_lines l \
                         JOIN accounting.journals j ON j.id=l.journal_id \
                         WHERE l.company_id=$1 AND j.source_id=$2 AND l.account_id=$3 \
                         ORDER BY l.id LIMIT 1)"
        ))
        .bind(company)
        .bind(invoice)
        .bind(account)
        .execute(&pool)
        .await
        .unwrap();
    }
    let payment = post_receipt(&posting, company, coa[&"1100"], coa[&"1200"], party, "100").await;

    let svc = ReconcileWriteService::new(
        Arc::new(SqlxReconcileGraphRepository::new()),
        Arc::new(SqlxPostingRepository::new(pool.clone())),
        pool.clone(),
        Some(coa[&"4900"]),
    )
    .with_deferred_tax(Arc::new(OrderDeferredTax {
        lines: vec![deferred_line(tr_line, &coa, "11.5")],
    }));
    let out = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa[&"1200"]),
            loc("payment", payment, coa[&"1200"]),
            "100",
        ))
        .await
        .unwrap();
    let partial = out.partial_id.unwrap();

    // Flip: round2(11.5 × 100/110) = 10.45 on the real account.
    assert_eq!(net(&pool, company, coa[&"2310"]).await, dec("-10.45"));
    // Exchange move: +10 AR credit against FX debit.
    let exch_journal: Uuid = sqlx::query_scalar(
        "SELECT exchange_move_id FROM accounting.partial_reconciles WHERE id=$1",
    )
    .bind(partial)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(exch_journal, Uuid::nil());
    assert_eq!(flip_journal_count(&pool, company).await, 1);

    // Unlink: both generated journals get their reversal.
    svc.unreconcile(company, partial, None).await.unwrap();
    let flip_journal: Uuid = sqlx::query_scalar(
        "SELECT j.id FROM accounting.journals j \
         JOIN accounting.accounting_posts ap ON ap.journal_id = j.id \
         WHERE j.company_id=$1 AND ap.source_reference LIKE 'caba:%' \
         ORDER BY j.id LIMIT 1",
    )
    .bind(company)
    .fetch_one(&pool)
    .await
    .unwrap();
    for journal in [exch_journal, flip_journal] {
        let _rev: Uuid = sqlx::query_scalar(
            "SELECT j.id FROM accounting.journals j \
             JOIN accounting.accounting_posts ap ON ap.journal_id = j.id \
             WHERE j.company_id=$1 AND ap.idempotency_key=$2",
        )
        .bind(company)
        .bind(format!("unlink:{journal}"))
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|_| panic!("reversal for {journal}"));
    }
    // Everything restored: transition full, real zero, FX zero.
    assert_eq!(net(&pool, company, coa[&"2300"]).await, dec("-11.5"));
    assert_eq!(net(&pool, company, coa[&"2310"]).await, Decimal::ZERO);
    assert_eq!(net(&pool, company, coa[&"4900"]).await, Decimal::ZERO);
}

// CABA-8: a deferral whose pro-rata share rounds below a cent posts NOTHING —
// no zero-amount journal, no panic; the remainder flips with a later payment.
#[tokio::test]
async fn caba8_zero_value_flip_skipped() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let (invoice, tr_line) =
        post_deferred_invoice(&posting, &pool, company, &coa, party, "1000", "0.01").await;
    let payment = post_receipt(&posting, company, coa[&"1100"], coa[&"1200"], party, "400").await;

    let svc = reconcile_svc(&pool, vec![deferred_line(tr_line, &coa, "0.01")]);
    let out = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa[&"1200"]),
            loc("payment", payment, coa[&"1200"]),
            "400",
        ))
        .await
        .unwrap();
    assert_eq!(out.applied, dec("400"));
    assert_eq!(
        flip_journal_count(&pool, company).await,
        0,
        "0.004 rounds to zero"
    );
    assert_eq!(net(&pool, company, coa[&"2310"]).await, Decimal::ZERO);
    assert_eq!(net(&pool, company, coa[&"2300"]).await, dec("-0.01"));
}

// CABA-9: multi-payment cent drift, under-sum shape. Three receipts whose
// independent pro-rata shares each round DOWN (33.34/33.33/33.33 against a
// 100 face with 10 tax) sum to 9.99 — an independent per-partial flip strands
// that cent on the transition line forever. The anchored cumulative form paces
// the mid-chain flip (3.33 + 3.34 + 3.33) so the total telescopes to exactly
// the face and the component completes.
#[tokio::test]
async fn caba9_multi_payment_under_sum_converges() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let (invoice, tr_line) =
        post_deferred_invoice(&posting, &pool, company, &coa, party, "100", "10").await;

    let svc = reconcile_svc(&pool, vec![deferred_line(tr_line, &coa, "10")]);
    let mut last = None;
    for amount in ["33.34", "33.33", "33.33"] {
        let payment =
            post_receipt(&posting, company, coa[&"1100"], coa[&"1200"], party, amount).await;
        last = Some(
            svc.reconcile_pair(&pair(
                company,
                loc("order", invoice, coa[&"1200"]),
                loc("payment", payment, coa[&"1200"]),
                amount,
            ))
            .await
            .unwrap(),
        );
    }
    let out = last.unwrap();
    assert_eq!(out.applied, dec("33.33"));
    assert_eq!(
        net(&pool, company, coa[&"2300"]).await,
        Decimal::ZERO,
        "transition fully retired — no stranded cent"
    );
    assert_eq!(
        net(&pool, company, coa[&"2310"]).await,
        dec("-10.00"),
        "the whole deferral flipped — independent rounding would stop at 9.99"
    );
    assert!(
        out.full_reconcile_id.is_some(),
        "component completes at full settlement"
    );
}

// CABA-10: multi-payment cent drift, over-sum shape. Receipts whose shares
// each round UP (16.67/16.67/66.66) sum to 10.01 — an independent flip would
// post past the face, drive the transition line's residual to −0.01, and
// poison it (a later reconcile hard-errors a negative clamp). The anchored
// form never posts more than the remaining residual: total lands exactly on
// the face and the line stays clean.
#[tokio::test]
async fn caba10_multi_payment_over_sum_never_over_applies() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let (invoice, tr_line) =
        post_deferred_invoice(&posting, &pool, company, &coa, party, "100", "10").await;

    let svc = reconcile_svc(&pool, vec![deferred_line(tr_line, &coa, "10")]);
    let mut last_payment = None;
    let mut last = None;
    for amount in ["16.67", "16.67", "66.66"] {
        let payment =
            post_receipt(&posting, company, coa[&"1100"], coa[&"1200"], party, amount).await;
        last = Some(
            svc.reconcile_pair(&pair(
                company,
                loc("order", invoice, coa[&"1200"]),
                loc("payment", payment, coa[&"1200"]),
                amount,
            ))
            .await
            .unwrap(),
        );
        last_payment = Some(payment);
    }
    let out = last.unwrap();
    assert_eq!(out.applied, dec("66.66"));
    assert_eq!(
        net(&pool, company, coa[&"2300"]).await,
        Decimal::ZERO,
        "never driven negative — the residual hit zero, not −0.01"
    );
    assert_eq!(
        net(&pool, company, coa[&"2310"]).await,
        dec("-10.00"),
        "exactly the face — independent rounding would post 10.01"
    );
    assert!(out.full_reconcile_id.is_some());
    // The transition line was never poisoned: re-requesting the last
    // settlement clamps cleanly to zero instead of erroring a negative clamp.
    let again = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa[&"1200"]),
            loc("payment", last_payment.unwrap(), coa[&"1200"]),
            "66.66",
        ))
        .await
        .unwrap();
    assert_eq!(again.applied, Decimal::ZERO);
}

// CABA-11: unlink-and-replace. After full settlement through three payments,
// the middle payment bounces: unlink reverses its flip journal, the
// replacement receipt re-reconciles. A ratio re-derived from the payment
// sequence (F(cum_after) − F(cum_before)) strands a cent here — only the
// posted-residual anchor re-trues to exactly the face. This test pins that
// distinction.
#[tokio::test]
async fn caba11_unlink_replace_converges() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let (invoice, tr_line) =
        post_deferred_invoice(&posting, &pool, company, &coa, party, "100", "10").await;

    let svc = reconcile_svc(&pool, vec![deferred_line(tr_line, &coa, "10")]);
    let mut middle_partial = None;
    for (i, amount) in ["33.34", "33.33", "33.33"].iter().enumerate() {
        let payment =
            post_receipt(&posting, company, coa[&"1100"], coa[&"1200"], party, amount).await;
        let out = svc
            .reconcile_pair(&pair(
                company,
                loc("order", invoice, coa[&"1200"]),
                loc("payment", payment, coa[&"1200"]),
                amount,
            ))
            .await
            .unwrap();
        if i == 1 {
            middle_partial = out.partial_id;
        }
    }
    assert_eq!(net(&pool, company, coa[&"2300"]).await, Decimal::ZERO);

    // The middle payment bounces and is replaced by an identical receipt.
    svc.unreconcile(company, middle_partial.unwrap(), None)
        .await
        .unwrap();
    let replacement = post_receipt(
        &posting,
        company,
        coa[&"1100"],
        coa[&"1200"],
        party,
        "33.33",
    )
    .await;
    let out = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa[&"1200"]),
            loc("payment", replacement, coa[&"1200"]),
            "33.33",
        ))
        .await
        .unwrap();
    assert_eq!(out.applied, dec("33.33"));
    assert_eq!(
        net(&pool, company, coa[&"2300"]).await,
        Decimal::ZERO,
        "the live flip set re-trues to the face — a sequence-derived ratio would strand a cent here"
    );
    assert_eq!(net(&pool, company, coa[&"2310"]).await, dec("-10.00"));
}

// CABA-12: multi-payment against a document booked at a non-1 rate. The flip
// pro-rates in BASE-currency terms (the AR line's base face and base-applied
// partials) — three receipts totaling the base face still telescope the
// deferral exactly.
#[tokio::test]
async fn caba12_fx_multi_payment_converges() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let (invoice, tr_line) =
        post_deferred_invoice(&posting, &pool, company, &coa, party, "100", "11.5").await;

    // Invoice books at rate 1.1 in base: AR 110, revenue 98.5, transition 11.5.
    for (account, base_col, base) in [
        (coa[&"1200"], "base_debit_amount", "110"),
        (coa[&"4000"], "base_credit_amount", "98.5"),
    ] {
        sqlx::query(&format!(
            "UPDATE accounting.journal_lines SET exchange_rate=1.1, {base_col}={base} \
             WHERE id = (SELECT l.id FROM accounting.journal_lines l \
                         JOIN accounting.journals j ON j.id=l.journal_id \
                         WHERE l.company_id=$1 AND j.source_id=$2 AND l.account_id=$3 \
                         ORDER BY l.id LIMIT 1)"
        ))
        .bind(company)
        .bind(invoice)
        .bind(account)
        .execute(&pool)
        .await
        .unwrap();
    }

    let svc = reconcile_svc(&pool, vec![deferred_line(tr_line, &coa, "11.5")]);
    let mut last = None;
    for amount in ["36.74", "36.63", "36.63"] {
        let payment =
            post_receipt(&posting, company, coa[&"1100"], coa[&"1200"], party, amount).await;
        last = Some(
            svc.reconcile_pair(&pair(
                company,
                loc("order", invoice, coa[&"1200"]),
                loc("payment", payment, coa[&"1200"]),
                amount,
            ))
            .await
            .unwrap(),
        );
    }
    let out = last.unwrap();
    assert_eq!(out.applied, dec("36.63"));
    assert_eq!(
        net(&pool, company, coa[&"2300"]).await,
        Decimal::ZERO,
        "base-currency telescoping: 3.84 + 3.83 + 3.83 retires the 11.5 face"
    );
    assert_eq!(net(&pool, company, coa[&"2310"]).await, dec("-11.50"));
    assert!(out.full_reconcile_id.is_some());
}
