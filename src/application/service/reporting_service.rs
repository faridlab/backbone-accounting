//! Financial-statement generation — Trial Balance, Balance Sheet, Income Statement.
//!
//! Hand-authored behavior (user-owned; see `metaphor.codegen.yaml`). Computes statements from
//! the immutable `ledgers` table (never from cached `accounts.current_balance`, so any as-of
//! date / period works). Proven by `tests/reporting_golden_cases.rs`.
//!
//! Sign convention (matches posting_service): a debit-normal account's balance = Σdebit−Σcredit;
//! a credit-normal account's balance = Σcredit−Σdebit. The global ledger is always balanced
//! (Σdebit = Σcredit), so a Trial Balance always foots and a Balance Sheet always balances via
//! `Assets = Liabilities + Equity + CurrentEarnings` (retained earnings are not closed here —
//! period-close is a separate concern).

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// One account's activity within a date window.
struct AcctSum {
    account_type: String,
    account_number: String,
    name: String,
    debit: Decimal,
    credit: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrialBalanceLine {
    pub account_number: String,
    pub name: String,
    pub account_type: String,
    pub debit: Decimal,  // balance shown on its normal side
    pub credit: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrialBalance {
    pub company_id: Uuid,
    pub as_of: NaiveDate,
    pub lines: Vec<TrialBalanceLine>,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub balanced: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BalanceSheet {
    pub company_id: Uuid,
    pub as_of: NaiveDate,
    pub assets: Decimal,
    pub liabilities: Decimal,
    pub equity: Decimal,
    /// Undistributed profit/loss since inception (Revenue − Expenses), not yet closed to equity.
    pub current_earnings: Decimal,
    pub total_liabilities_and_equity: Decimal,
    pub balanced: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncomeStatement {
    pub company_id: Uuid,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub revenue: Decimal,
    pub other_income: Decimal,
    pub cogs: Decimal,
    pub expenses: Decimal,
    pub other_expense: Decimal,
    pub net_income: Decimal,
}

/// Report generator over the GL.
#[derive(Clone)]
pub struct ReportingService {
    db_pool: PgPool,
}

impl ReportingService {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    /// Per-detail-account debit/credit sums over `[lo, hi]` (lo = None → since inception).
    async fn account_sums(
        &self,
        company_id: Uuid,
        lo: Option<NaiveDate>,
        hi: NaiveDate,
    ) -> Result<Vec<AcctSum>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT a.account_type::text AS at, a.account_number AS num, a.name AS name,
                      COALESCE(SUM(l.debit_amount),0) AS dr,
                      COALESCE(SUM(l.credit_amount),0) AS cr
               FROM accounting.accounts a
               LEFT JOIN accounting.ledgers l
                 ON l.account_id = a.id
                AND l.posting_date <= $2
                AND ($3::date IS NULL OR l.posting_date >= $3)
               WHERE a.company_id = $1
                 AND a.is_detail = TRUE
                 AND (a.metadata->>'deleted_at') IS NULL
               GROUP BY a.id, a.account_type, a.account_number, a.name
               ORDER BY a.account_number"#,
        )
        .bind(company_id)
        .bind(hi)
        .bind(lo)
        .fetch_all(&self.db_pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| AcctSum {
                account_type: r.get("at"),
                account_number: r.get("num"),
                name: r.get("name"),
                debit: r.get("dr"),
                credit: r.get("cr"),
            })
            .collect())
    }

    /// Signed balance on the account's normal side.
    fn normal_balance(at: &str, debit: Decimal, credit: Decimal) -> Decimal {
        match at {
            "asset" | "expense" | "cogs" | "other_expense" => debit - credit,
            _ => credit - debit, // liability | equity | revenue | other_income
        }
    }

    pub async fn trial_balance(
        &self,
        company_id: Uuid,
        as_of: NaiveDate,
    ) -> Result<TrialBalance, sqlx::Error> {
        let sums = self.account_sums(company_id, None, as_of).await?;
        let mut lines = Vec::new();
        let mut total_debit = Decimal::ZERO;
        let mut total_credit = Decimal::ZERO;
        for s in &sums {
            let net = s.debit - s.credit; // positive → net debit
            if net == Decimal::ZERO {
                continue; // omit zero-activity accounts
            }
            let (debit, credit) = if net > Decimal::ZERO {
                (net, Decimal::ZERO)
            } else {
                (Decimal::ZERO, -net)
            };
            total_debit += debit;
            total_credit += credit;
            lines.push(TrialBalanceLine {
                account_number: s.account_number.clone(),
                name: s.name.clone(),
                account_type: s.account_type.clone(),
                debit,
                credit,
            });
        }
        Ok(TrialBalance {
            company_id,
            as_of,
            lines,
            total_debit,
            total_credit,
            balanced: total_debit == total_credit,
        })
    }

    pub async fn balance_sheet(
        &self,
        company_id: Uuid,
        as_of: NaiveDate,
    ) -> Result<BalanceSheet, sqlx::Error> {
        let sums = self.account_sums(company_id, None, as_of).await?;
        let mut assets = Decimal::ZERO;
        let mut liabilities = Decimal::ZERO;
        let mut equity = Decimal::ZERO;
        let mut current_earnings = Decimal::ZERO;
        for s in &sums {
            let bal = Self::normal_balance(&s.account_type, s.debit, s.credit);
            match s.account_type.as_str() {
                "asset" => assets += bal,
                "liability" => liabilities += bal,
                "equity" => equity += bal,
                // Current earnings = Revenue − Expenses. `bal` is the account's normal-side
                // magnitude (revenue → cr−dr, expense → dr−cr), so revenue adds and expenses
                // subtract.
                "revenue" | "other_income" => current_earnings += bal,
                "expense" | "cogs" | "other_expense" => current_earnings -= bal,
                _ => {}
            }
        }
        let total_liabilities_and_equity = liabilities + equity + current_earnings;
        Ok(BalanceSheet {
            company_id,
            as_of,
            assets,
            liabilities,
            equity,
            current_earnings,
            total_liabilities_and_equity,
            balanced: assets == total_liabilities_and_equity,
        })
    }

    pub async fn income_statement(
        &self,
        company_id: Uuid,
        period_start: NaiveDate,
        period_end: NaiveDate,
    ) -> Result<IncomeStatement, sqlx::Error> {
        let sums = self
            .account_sums(company_id, Some(period_start), period_end)
            .await?;
        let mut revenue = Decimal::ZERO;
        let mut other_income = Decimal::ZERO;
        let mut cogs = Decimal::ZERO;
        let mut expenses = Decimal::ZERO;
        let mut other_expense = Decimal::ZERO;
        for s in &sums {
            let bal = Self::normal_balance(&s.account_type, s.debit, s.credit);
            match s.account_type.as_str() {
                "revenue" => revenue += bal,
                "other_income" => other_income += bal,
                "cogs" => cogs += bal,
                "expense" => expenses += bal,
                "other_expense" => other_expense += bal,
                _ => {}
            }
        }
        let net_income = revenue + other_income - cogs - expenses - other_expense;
        Ok(IncomeStatement {
            company_id,
            period_start,
            period_end,
            revenue,
            other_income,
            cogs,
            expenses,
            other_expense,
            net_income,
        })
    }
}
