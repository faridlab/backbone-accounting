//! Golden cases for the reconciliation graph (partial edges → full groups).
//!
//! Runs against a real Postgres (DATABASE_URL, defaults to :5433/backbone_accounting).
//! Each test seeds its own company + chart of accounts — isolated and parallel-safe.
//!
//! Covers: the CLAMP, every guard's distinct refusal code, partial→full group
//! completion + flags, the matching-group union-find read, party residuals
//! (aging), cross-company fail-closed, the exchange-difference machinery with its
//! side-effecting unlink (nets zero), group repair after unlink, reverse-then-reconcile
//! pairing, and concurrent clamping (never over-edges).

use std::collections::HashMap;
use std::sync::Arc;

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_accounting::application::service::posting_service::{
    PostingLine, PostingRequest, PostingService,
};
use backbone_accounting::application::service::reconcile_write_service::ReconcileWriteService;
use backbone_accounting::domain::reconcile_graph::{
    LineLocator, PairRequest, ORIGIN_MANUAL, ORIGIN_SETTLEMENT,
};
use backbone_accounting::domain::repositories::posting_repository::PostingRepository;
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

/// Seed a fresh company with a reconcilable chart. Returns (company, code→account id).
async fn seed(pool: &PgPool) -> (Uuid, HashMap<&'static str, Uuid>) {
    let company_id = Uuid::new_v4();
    // (code, name, type, subtype, normal_balance, reconcilable)
    let coa: &[(&str, &str, &str, &str, &str, bool)] = &[
        ("1100", "Bank", "asset", "bank", "debit", true),
        ("1200", "AR", "asset", "accounts_receivable", "debit", true),
        (
            "2100",
            "AP",
            "liability",
            "accounts_payable",
            "credit",
            true,
        ),
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
    ];
    let mut map = HashMap::new();
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

fn reconcile_svc(pool: &PgPool) -> ReconcileWriteService {
    ReconcileWriteService::new(
        Arc::new(SqlxReconcileGraphRepository::new()),
        Arc::new(SqlxPostingRepository::new(pool.clone())),
        pool.clone(),
        None,
    )
}

fn reconcile_svc_with_fx(pool: &PgPool, fx: Uuid) -> ReconcileWriteService {
    ReconcileWriteService::new(
        Arc::new(SqlxReconcileGraphRepository::new()),
        Arc::new(SqlxPostingRepository::new(pool.clone())),
        pool.clone(),
        Some(fx),
    )
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

/// The journal-line id for (source identity, account) — the locator's storage shape.
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

async fn residual(pool: &PgPool, company: Uuid, line: Uuid) -> Decimal {
    sqlx::query_scalar(
        "SELECT (l.base_debit_amount + l.base_credit_amount) \
              - COALESCE((SELECT SUM(pr.amount) FROM accounting.partial_reconciles pr \
                          WHERE pr.company_id = l.company_id \
                            AND (pr.debit_move_id = l.id OR pr.credit_move_id = l.id)), 0) \
         FROM accounting.journal_lines l WHERE l.company_id=$1 AND l.id=$2",
    )
    .bind(company)
    .bind(line)
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

/// Invoice (Dr AR / Cr Revenue) for `party` under `company`.
async fn post_invoice(
    svc: &PostingService,
    company: Uuid,
    ar: Uuid,
    revenue: Uuid,
    party: Uuid,
    amount: &str,
) -> Uuid {
    let invoice_ref = Uuid::new_v4();
    post(
        svc,
        company,
        "order",
        invoice_ref,
        vec![
            line(ar, amount, "0", Some(party)),
            line(revenue, "0", amount, None),
        ],
    )
    .await;
    invoice_ref
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
    let payment_id = Uuid::new_v4();
    post(
        svc,
        company,
        "payment",
        payment_id,
        vec![
            line(bank, amount, "0", None),
            line(ar, "0", amount, Some(party)),
        ],
    )
    .await;
    payment_id
}

// =============================================================================
// Cases
// =============================================================================

#[tokio::test]
async fn clamps_to_the_smaller_residual_and_keeps_chain_partial() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let invoice = post_invoice(&posting, company, coa["1200"], coa["4000"], party, "100").await;
    let payment = post_receipt(&posting, company, coa["1100"], coa["1200"], party, "60").await;
    let svc = reconcile_svc(&pool);

    // Request 80 against residuals (100, 60) — CLAMP applies 60.
    let out = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa["1200"]),
            loc("payment", payment, coa["1200"]),
            "80",
        ))
        .await
        .expect("edge");
    assert_eq!(out.applied, dec("60"));
    assert!(out.partial_id.is_some());
    // Chain is still partial: AR residual 40, receipt 0, no group, no flags.
    let inv_line = line_id(&pool, company, "order", invoice, coa["1200"]).await;
    let pay_line = line_id(&pool, company, "payment", payment, coa["1200"]).await;
    assert_eq!(residual(&pool, company, inv_line).await, dec("40"));
    assert_eq!(residual(&pool, company, pay_line).await, dec("0"));
    let flags: (bool, Option<Uuid>) = sqlx::query_as(
        "SELECT is_reconciled, full_reconcile_id FROM accounting.journal_lines WHERE id=$1",
    )
    .bind(inv_line)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!flags.0 && flags.1.is_none());
    // The matching label is partial-shaped: P-<uuid8>.
    let g = svc.matching_group(company, inv_line).await.unwrap();
    assert!(g.label.starts_with("P-"), "label was {}", g.label);
    assert!(g.full_reconcile_id.is_none());
}

#[tokio::test]
async fn guard_refusals_carry_distinct_codes() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let invoice = post_invoice(&posting, company, coa["1200"], coa["4000"], party, "100").await;
    let payment = post_receipt(&posting, company, coa["1100"], coa["1200"], party, "60").await;
    let other_party = Uuid::new_v4();
    let payment2 = post_receipt(
        &posting,
        company,
        coa["1100"],
        coa["1200"],
        other_party,
        "60",
    )
    .await;
    let svc = reconcile_svc(&pool);

    let code =
        |e: backbone_accounting::domain::reconcile_graph::ReconcileError| e.code().to_string();

    // Split accounts.
    let e = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa["1200"]),
            loc("payment", payment, coa["1100"]), // bank, not AR
            "10",
        ))
        .await
        .unwrap_err();
    assert_eq!(code(e), "same_account_required");

    // Non-reconcilable account (revenue — both locators must resolve first).
    let invoice2 = post_invoice(&posting, company, coa["1200"], coa["4000"], party, "50").await;
    let e = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa["4000"]),
            loc("order", invoice2, coa["4000"]),
            "10",
        ))
        .await
        .unwrap_err();
    assert_eq!(code(e), "account_not_reconcilable");

    // Same direction: two receipt bank legs are both debits.
    let e = svc
        .reconcile_pair(&pair(
            company,
            loc("payment", payment, coa["1100"]),
            loc("payment", payment2, coa["1100"]),
            "10",
        ))
        .await
        .unwrap_err();
    assert_eq!(code(e), "direction_mismatch");

    // Party mismatch on a party account.
    let e = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa["1200"]),
            loc("payment", payment2, coa["1200"]),
            "10",
        ))
        .await
        .unwrap_err();
    assert_eq!(code(e), "party_mismatch");

    // Cross-currency.
    let pay_line = line_id(&pool, company, "payment", payment, coa["1200"]).await;
    sqlx::query("UPDATE accounting.journal_lines SET currency='USD' WHERE id=$1")
        .bind(pay_line)
        .execute(&pool)
        .await
        .unwrap();
    let e = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa["1200"]),
            loc("payment", payment, coa["1200"]),
            "10",
        ))
        .await
        .unwrap_err();
    assert_eq!(code(e), "currency_mismatch");
    sqlx::query("UPDATE accounting.journal_lines SET currency='IDR' WHERE id=$1")
        .bind(pay_line)
        .execute(&pool)
        .await
        .unwrap();

    // Unposted line: a draft journal's AR line.
    let draft_journal = Uuid::new_v4();
    let draft_src = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.journals
             (id, company_id, journal_number, journal_type, source, source_type, source_id,
              transaction_date, posting_date, description, currency, status, is_reversing)
           VALUES ($1,$2,'DRAFT-1','general'::journal_type,'manual'::journal_source,'payment',$3,
                   '2026-06-15','2026-06-15','draft probe','IDR','draft'::journal_status,FALSE)"#,
    )
    .bind(draft_journal)
    .bind(company)
    .bind(draft_src)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO accounting.journal_lines
             (id, journal_id, company_id, line_number, account_id, account_number, account_name,
              debit_amount, credit_amount, currency, base_debit_amount, base_credit_amount,
              is_posted, source_type, source_id)
           VALUES ($1,$2,$3,1,$4,'1200','AR',0,50,'IDR',0,50,FALSE,'payment',$5)"#,
    )
    .bind(Uuid::new_v4())
    .bind(draft_journal)
    .bind(company)
    .bind(coa["1200"])
    .bind(draft_src)
    .execute(&pool)
    .await
    .unwrap();
    let e = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa["1200"]),
            loc("payment", draft_src, coa["1200"]),
            "10",
        ))
        .await
        .unwrap_err();
    assert_eq!(code(e), "line_not_posted");

    // Unknown locator → 404-shaped not-found.
    let e = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa["1200"]),
            loc("payment", Uuid::new_v4(), coa["1200"]),
            "10",
        ))
        .await
        .unwrap_err();
    assert_eq!(code(e), "line_not_found");
}

#[tokio::test]
async fn partial_then_full_group_sets_flags_and_label() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let invoice = post_invoice(&posting, company, coa["1200"], coa["4000"], party, "100").await;
    let p1 = post_receipt(&posting, company, coa["1100"], coa["1200"], party, "60").await;
    let p2 = post_receipt(&posting, company, coa["1100"], coa["1200"], party, "40").await;
    let svc = reconcile_svc(&pool);

    svc.reconcile_pair(&pair(
        company,
        loc("order", invoice, coa["1200"]),
        loc("payment", p1, coa["1200"]),
        "60",
    ))
    .await
    .unwrap();

    // Second edge completes the component → full group + flags + F-label.
    let out = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa["1200"]),
            loc("payment", p2, coa["1200"]),
            "40",
        ))
        .await
        .unwrap();
    assert_eq!(out.applied, dec("40"));
    let group = out.full_reconcile_id.expect("full group");

    let inv_line = line_id(&pool, company, "order", invoice, coa["1200"]).await;
    for lid in [
        inv_line,
        line_id(&pool, company, "payment", p1, coa["1200"]).await,
        line_id(&pool, company, "payment", p2, coa["1200"]).await,
    ] {
        let row: (bool, Option<Uuid>, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
            "SELECT is_reconciled, full_reconcile_id, reconciled_at \
             FROM accounting.journal_lines WHERE id=$1",
        )
        .bind(lid)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(row.0, "line must be flagged reconciled");
        assert_eq!(row.1, Some(group));
        assert!(row.2.is_some());
    }
    let g = svc.matching_group(company, inv_line).await.unwrap();
    assert!(g.label.starts_with("F-"), "label was {}", g.label);
    assert_eq!(g.full_reconcile_id, Some(group));
    assert_eq!(g.line_ids.len(), 3);
    assert!(g.residuals.iter().all(|(_, r)| *r == Decimal::ZERO));
    // All three partials link the group.
    let linked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.partial_reconciles WHERE company_id=$1 AND full_reconcile_id=$2",
    )
    .bind(company)
    .bind(group)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked, 2);
}

#[tokio::test]
async fn matching_group_reads_a_multi_payment_chain() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let invoice = post_invoice(&posting, company, coa["1200"], coa["4000"], party, "100").await;
    let mut payments = Vec::new();
    for _ in 0..4 {
        payments.push(post_receipt(&posting, company, coa["1100"], coa["1200"], party, "25").await);
    }
    let svc = reconcile_svc(&pool);

    for p in &payments {
        svc.reconcile_pair(&pair(
            company,
            loc("order", invoice, coa["1200"]),
            loc("payment", *p, coa["1200"]),
            "25",
        ))
        .await
        .unwrap();
    }

    // Any line in the 5-line component sees the same group: 4 partials, all zero residual.
    let inv_line = line_id(&pool, company, "order", invoice, coa["1200"]).await;
    let g = svc.matching_group(company, inv_line).await.unwrap();
    assert!(g.label.starts_with("F-"));
    assert_eq!(g.line_ids.len(), 5);
    assert_eq!(g.partial_ids.len(), 4);
    assert!(g.residuals.iter().all(|(_, r)| *r == Decimal::ZERO));

    // From a payment line too — union-find over the same component.
    let pay_line = line_id(&pool, company, "payment", payments[2], coa["1200"]).await;
    let g2 = svc.matching_group(company, pay_line).await.unwrap();
    assert_eq!(g2.label, g.label);
    assert_eq!(g2.full_reconcile_id, g.full_reconcile_id);
}

#[tokio::test]
async fn residuals_for_party_lists_only_open_lines() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let i1 = post_invoice(&posting, company, coa["1200"], coa["4000"], party, "100").await;
    let i2 = post_invoice(&posting, company, coa["1200"], coa["4000"], party, "70").await;
    let payment = post_receipt(&posting, company, coa["1100"], coa["1200"], party, "100").await;
    let svc = reconcile_svc(&pool);
    svc.reconcile_pair(&pair(
        company,
        loc("order", i1, coa["1200"]),
        loc("payment", payment, coa["1200"]),
        "100",
    ))
    .await
    .unwrap();

    let open = svc
        .residuals_for_party(company, coa["1200"], "customer", party)
        .await
        .unwrap();
    // Only invoice 2's line remains open — the settled one and the receipt are gone.
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].residual, dec("70"));
    let open_line = line_id(&pool, company, "order", i2, coa["1200"]).await;
    assert_eq!(open[0].line_id, open_line);

    // A different party sees nothing.
    let none = svc
        .residuals_for_party(company, coa["1200"], "customer", Uuid::new_v4())
        .await
        .unwrap();
    assert!(none.is_empty());
}

#[tokio::test]
async fn cross_company_locator_is_not_found() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let (other, _) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let invoice = post_invoice(&posting, company, coa["1200"], coa["4000"], party, "100").await;
    let payment = post_receipt(&posting, company, coa["1100"], coa["1200"], party, "100").await;
    let svc = reconcile_svc(&pool);

    // Ask under the OTHER company's scope: the fence + explicit predicates hide the lines.
    let e = svc
        .reconcile_pair(&pair(
            other,
            loc("order", invoice, coa["1200"]),
            loc("payment", payment, coa["1200"]),
            "10",
        ))
        .await
        .unwrap_err();
    assert_eq!(e.code(), "line_not_found");
}

#[tokio::test]
async fn unreconcile_restores_outstanding_and_repairs_the_group() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let invoice = post_invoice(&posting, company, coa["1200"], coa["4000"], party, "100").await;
    let payment = post_receipt(&posting, company, coa["1100"], coa["1200"], party, "100").await;
    let svc = reconcile_svc(&pool);
    // The accountant verb: origin manual (vs the settlement seam's origin elsewhere).
    let out = svc
        .reconcile_pair(&PairRequest {
            company_id: company,
            debit: loc("order", invoice, coa["1200"]),
            credit: loc("payment", payment, coa["1200"]),
            amount: dec("100"),
            origin: ORIGIN_MANUAL.to_string(),
            actor: None,
        })
        .await
        .unwrap();
    let group = out.full_reconcile_id.expect("group");
    let partial = out.partial_id.unwrap();

    svc.unreconcile(company, partial, None).await.unwrap();

    let inv_line = line_id(&pool, company, "order", invoice, coa["1200"]).await;
    let pay_line = line_id(&pool, company, "payment", payment, coa["1200"]).await;
    assert_eq!(residual(&pool, company, inv_line).await, dec("100"));
    assert_eq!(residual(&pool, company, pay_line).await, dec("100"));
    // Flags cleared, group dissolved, no partials left.
    let flags: (bool, Option<Uuid>) = sqlx::query_as(
        "SELECT is_reconciled, full_reconcile_id FROM accounting.journal_lines WHERE id=$1",
    )
    .bind(inv_line)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!flags.0 && flags.1.is_none());
    let groups: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounting.full_reconciles WHERE id=$1")
            .bind(group)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(groups, 0, "emptied group must dissolve");
    let edges: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.partial_reconciles WHERE company_id=$1",
    )
    .bind(company)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(edges, 0);
}

#[tokio::test]
async fn exchange_difference_arises_and_unlink_nets_zero() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let invoice = post_invoice(&posting, company, coa["1200"], coa["4000"], party, "100").await;
    let payment = post_receipt(&posting, company, coa["1100"], coa["1200"], party, "100").await;

    // Seed multi-rate postings by hand: both docs are 100, but the invoice books at
    // rate 1.1 (base 110) while the receipt books at 1.0 (base 100). The 10 gap is
    // the exchange difference the edge must materialize.
    let inv_line = line_id(&pool, company, "order", invoice, coa["1200"]).await;
    let pay_line = line_id(&pool, company, "payment", payment, coa["1200"]).await;
    sqlx::query(
        "UPDATE accounting.journal_lines SET exchange_rate=1.1, base_debit_amount=110 \
         WHERE id=$1",
    )
    .bind(inv_line)
    .execute(&pool)
    .await
    .unwrap();
    // Keep the invoice journal balanced in base: revenue leg books at the same 1.1.
    sqlx::query(
        "UPDATE accounting.journal_lines SET exchange_rate=1.1, base_credit_amount=110 \
         WHERE company_id=$1 AND source_id=$2 AND account_id=$3",
    )
    .bind(company)
    .bind(invoice)
    .bind(coa["4000"])
    .execute(&pool)
    .await
    .unwrap();

    let svc = reconcile_svc_with_fx(&pool, coa["4900"]);
    let out = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa["1200"]),
            loc("payment", payment, coa["1200"]),
            "100",
        ))
        .await
        .unwrap();
    assert_eq!(out.applied, dec("100"));
    let partial = out.partial_id.unwrap();
    assert!(out.full_reconcile_id.is_some(), "component must reach zero");

    // The edge carries an exchange move; the AR legs of exch + nothing else changed yet.
    let exch_journal: Uuid = sqlx::query_scalar(
        "SELECT exchange_move_id FROM accounting.partial_reconciles WHERE id=$1",
    )
    .bind(partial)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(exch_journal, Uuid::nil());
    let exch_lines: Vec<(Uuid, Decimal, Decimal)> = sqlx::query_as(
        "SELECT account_id, base_debit_amount, base_credit_amount \
         FROM accounting.journal_lines WHERE journal_id=$1 ORDER BY id",
    )
    .bind(exch_journal)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(exch_lines.len(), 2);
    let ar_leg = exch_lines
        .iter()
        .find(|l| l.0 == coa["1200"])
        .expect("AR leg");
    let fx_leg = exch_lines
        .iter()
        .find(|l| l.0 == coa["4900"])
        .expect("FX leg");
    assert_eq!(ar_leg.2, dec("10"), "AR takes the credit for +10 delta");
    assert_eq!(fx_leg.1, dec("10"), "FX takes the debit");

    // --- Unlink: the generated move must be REVERSED (never a bare delete). ---
    svc.unreconcile(company, partial, None).await.unwrap();

    // The exchange journal AND its unlink reversal both exist, canceling each other.
    let unlink_rev: Uuid = sqlx::query_scalar(
        "SELECT j.id FROM accounting.journals j \
         JOIN accounting.accounting_posts ap ON ap.journal_id = j.id \
         WHERE j.company_id=$1 AND ap.idempotency_key=$2",
    )
    .bind(company)
    .bind(format!("unlink:{exch_journal}"))
    .fetch_one(&pool)
    .await
    .expect("unlink reversal journal");

    // Per-account ledger nets: AR legs of exch + reversal cancel; FX account nets zero.
    let ar_net: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(l.base_debit_amount - l.base_credit_amount),0) \
         FROM accounting.journal_lines l \
         WHERE l.company_id=$1 AND l.account_id=$2 AND l.journal_id IN ($3, $4)",
    )
    .bind(company)
    .bind(coa["1200"])
    .bind(exch_journal)
    .bind(unlink_rev)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ar_net, Decimal::ZERO);
    let fx_net: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(l.base_debit_amount - l.base_credit_amount),0) \
         FROM accounting.journal_lines l \
         WHERE l.company_id=$1 AND l.account_id=$2 AND l.journal_id IN ($3, $4)",
    )
    .bind(company)
    .bind(coa["4900"])
    .bind(exch_journal)
    .bind(unlink_rev)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(fx_net, Decimal::ZERO);

    // Graph: no edges, no groups, both original lines restored to full face.
    let edges: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.partial_reconciles WHERE company_id=$1",
    )
    .bind(company)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(edges, 0);
    assert_eq!(residual(&pool, company, inv_line).await, dec("110"));
    assert_eq!(residual(&pool, company, pay_line).await, dec("100"));
}

#[tokio::test]
async fn reverse_then_reconcile_pairs_a_reversed_payment_automatically() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let invoice = post_invoice(&posting, company, coa["1200"], coa["4000"], party, "100").await;
    let payment = post_receipt(&posting, company, coa["1100"], coa["1200"], party, "100").await;
    let svc = reconcile_svc(&pool);

    // Settle fully, then reverse the payment: unlink restores both lines, and the
    // pairing rule must reconcile the payment's AR line against its reversal's AR line.
    let out = svc
        .reconcile_pair(&pair(
            company,
            loc("order", invoice, coa["1200"]),
            loc("payment", payment, coa["1200"]),
            "100",
        ))
        .await
        .unwrap();

    // The payment's reversal (same source identity, is_reversing journal).
    let orig_post: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounting.accounting_posts WHERE company_id=$1 AND source_type='payment' AND source_id=$2",
    )
    .bind(company)
    .bind(payment)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut rev_req = PostingRequest::original(
        company,
        "payment",
        payment,
        chrono::NaiveDate::from_ymd_opt(2026, 6, 16).unwrap(),
    );
    rev_req.posting_type = "reversal".to_string();
    rev_req.reverses_post_id = Some(orig_post);
    posting.post(rev_req, None).await.expect("reversal posts");

    // Unlink the settlement edge — the ordering hazard the rule removes.
    svc.unreconcile(company, out.partial_id.unwrap(), None)
        .await
        .unwrap();

    // The payment AR line + reversal AR line now form their own FULL group.
    let pay_line = line_id(&pool, company, "payment", payment, coa["1200"]).await;
    let g = svc.matching_group(company, pay_line).await.unwrap();
    assert!(g.label.starts_with("F-"), "label was {}", g.label);
    assert_eq!(g.line_ids.len(), 2);
    assert!(g.residuals.iter().all(|(_, r)| *r == Decimal::ZERO));
    let rule: String = sqlx::query_scalar(
        "SELECT metadata->>'rule' FROM accounting.partial_reconciles WHERE company_id=$1 LIMIT 1",
    )
    .bind(company)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rule, "reverse_then_reconcile");
    // And the invoice line is back to fully open.
    let inv_line = line_id(&pool, company, "order", invoice, coa["1200"]).await;
    assert_eq!(residual(&pool, company, inv_line).await, dec("100"));
}

#[tokio::test]
async fn concurrent_reconciles_never_over_edge() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let invoice = post_invoice(&posting, company, coa["1200"], coa["4000"], party, "100").await;
    let p1 = post_receipt(&posting, company, coa["1100"], coa["1200"], party, "60").await;
    let p2 = post_receipt(&posting, company, coa["1100"], coa["1200"], party, "60").await;
    let svc = Arc::new(reconcile_svc(&pool));

    // Two 60-claims race for one 100 invoice: 60 + 40 (clamped), never 120.
    let first = pair(
        company,
        loc("order", invoice, coa["1200"]),
        loc("payment", p1, coa["1200"]),
        "60",
    );
    let second = pair(
        company,
        loc("order", invoice, coa["1200"]),
        loc("payment", p2, coa["1200"]),
        "60",
    );
    let (a, b) = tokio::join!(svc.reconcile_pair(&first), svc.reconcile_pair(&second),);
    let applied: Decimal = [a.unwrap().applied, b.unwrap().applied].iter().sum();
    assert_eq!(applied, dec("100"));

    let inv_line = line_id(&pool, company, "order", invoice, coa["1200"]).await;
    assert_eq!(residual(&pool, company, inv_line).await, Decimal::ZERO);
    let total: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount),0) FROM accounting.partial_reconciles WHERE company_id=$1",
    )
    .bind(company)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(total, dec("100"));
}

/// Two racing reconciles that COMPLETE the same component (everything consumed to zero, unlike
/// the on-account race above) must stamp exactly ONE full-reconcile group. The completions
/// serialize on the component's line locks; the waiter re-sees all-zero residuals because group
/// creation touches no partials — without the already-stamped guard in the completion it would
/// mint a second group over the winner's, orphaning it. One group, uniform stamp, no orphans.
#[tokio::test]
async fn concurrent_completions_stamp_one_group() {
    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let party = Uuid::new_v4();
    let posting = posting_svc(&pool);
    let invoice = post_invoice(&posting, company, coa["1200"], coa["4000"], party, "120").await;
    let p1 = post_receipt(&posting, company, coa["1100"], coa["1200"], party, "60").await;
    let p2 = post_receipt(&posting, company, coa["1100"], coa["1200"], party, "60").await;
    let svc = Arc::new(reconcile_svc(&pool));

    let first = pair(
        company,
        loc("order", invoice, coa["1200"]),
        loc("payment", p1, coa["1200"]),
        "60",
    );
    let second = pair(
        company,
        loc("order", invoice, coa["1200"]),
        loc("payment", p2, coa["1200"]),
        "60",
    );
    let (a, b) = tokio::join!(svc.reconcile_pair(&first), svc.reconcile_pair(&second));
    let applied: Decimal = [a.unwrap().applied, b.unwrap().applied].iter().sum();
    assert_eq!(
        applied,
        dec("120"),
        "the two claims exactly consume the invoice"
    );

    let inv_line = line_id(&pool, company, "order", invoice, coa["1200"]).await;
    let p1_line = line_id(&pool, company, "payment", p1, coa["1200"]).await;
    let p2_line = line_id(&pool, company, "payment", p2, coa["1200"]).await;
    for l in [inv_line, p1_line, p2_line] {
        assert_eq!(residual(&pool, company, l).await, Decimal::ZERO);
    }
    let groups: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounting.full_reconciles WHERE company_id=$1")
            .bind(company)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        groups, 1,
        "overlapping completions share one group, no orphan"
    );
    let stamps: Vec<Option<Uuid>> = sqlx::query_scalar(
        "SELECT full_reconcile_id FROM accounting.journal_lines WHERE id = ANY($1) ORDER BY id",
    )
    .bind(&[inv_line, p1_line, p2_line][..])
    .fetch_all(&pool)
    .await
    .unwrap();
    let uniform = stamps.iter().map(|s| *s).collect::<Option<Vec<Uuid>>>();
    let uniform = uniform.expect("every completed line carries a group");
    assert!(
        uniform.windows(2).all(|w| w[0] == w[1]),
        "one stamp across the component"
    );
    let orphans: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.full_reconciles fr WHERE company_id=$1 AND NOT EXISTS \
         (SELECT 1 FROM accounting.partial_reconciles p WHERE p.full_reconcile_id = fr.id)",
    )
    .bind(company)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(orphans, 0, "the group keeps its partials");
}

// ── Tenant consistency on the verb routes ─────────────────────────────────────
//
// The verb services bind RLS from the REQUEST's company_id; when a host has an
// ambient company scope (company_auth's `with_company_scope` task-local), the
// request's company must agree with it — otherwise an authenticated tenant
// could name any company in the body and read or reshape its books. Without an
// ambient scope the verbs keep their standalone (trusted-host) shape.
#[tokio::test]
async fn reconcile_verbs_refuse_company_mismatch_under_ambient_scope() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    let pool = pool().await;
    let (company, coa) = seed(&pool).await;
    let other = Uuid::new_v4();
    let app =
        backbone_accounting::presentation::http::reconcile_handler::create_reconcile_verb_routes(
            Arc::new(reconcile_svc(&pool)),
        );

    let pair_body = serde_json::json!({
        "company_id": company,
        "debit": {"source_type": "order", "source_id": Uuid::new_v4(), "account_id": coa["1200"]},
        "credit": {"source_type": "payment", "source_id": Uuid::new_v4(), "account_id": coa["1200"]},
        "amount": "1",
        "origin": "manual",
    })
    .to_string();

    // POST /accounting/reconcile naming another company → 403 company_mismatch,
    // before any DB work (the locators above are random uuids; reaching the
    // service would answer 404 line_not_found instead).
    let resp = backbone_orm::with_company_scope(Some(other), async {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/accounting/reconcile")
                    .header("content-type", "application/json")
                    .body(Body::from(pair_body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap()
    })
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes).to_string();
    assert!(text.contains("company_mismatch"), "got: {text}");

    // Matching ambient scope passes the tenant gate (and then fails on the
    // random locators — proving the gate, not the guard, answered).
    let resp = backbone_orm::with_company_scope(Some(company), async {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/accounting/reconcile")
                    .header("content-type", "application/json")
                    .body(Body::from(pair_body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap()
    })
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // GET /accounting/reconciliation-groups naming another company → 403.
    let resp = backbone_orm::with_company_scope(Some(other), async {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/accounting/reconciliation-groups/{}?company_id={}",
                        Uuid::new_v4(),
                        company
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    })
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // POST /accounting/unreconcile naming another company → 403.
    let resp = backbone_orm::with_company_scope(Some(other), async {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/accounting/unreconcile/{}", Uuid::new_v4()))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"company_id": company}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    })
    .await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
