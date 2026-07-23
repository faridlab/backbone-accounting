//! Financial-statement generation — Trial Balance, Balance Sheet, Income Statement.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Application orchestration over the
//! `ReportingRepository` port — no `sqlx`/`PgPool` here. Computes statements from the immutable
//! `ledgers` table (never from cached `accounts.current_balance`, so any as-of date / period
//! works). Proven by `tests/reporting_golden_cases.rs`.
//!
//! Sign convention: a debit-normal account's balance = Σdebit−Σcredit; a credit-normal account's
//! balance = Σcredit−Σdebit. The global ledger is always balanced (Σdebit = Σcredit), so a Trial
//! Balance always foots and a Balance Sheet always balances via
//! `Assets = Liabilities + Equity + CurrentEarnings`.

use std::sync::Arc;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use crate::domain::repositories::reporting_repository::ReportingRepository;

#[derive(Debug, Clone, Serialize)]
pub struct TrialBalanceLine {
    pub account_number: String,
    pub name: String,
    pub account_type: String,
    pub debit: Decimal,
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
    repo: Arc<dyn ReportingRepository>,
}

impl ReportingService {
    pub fn new(repo: Arc<dyn ReportingRepository>) -> Self {
        Self { repo }
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
    ) -> anyhow::Result<TrialBalance> {
        let sums = self.repo.account_sums(company_id, None, as_of).await?;
        let mut lines = Vec::new();
        let mut total_debit = Decimal::ZERO;
        let mut total_credit = Decimal::ZERO;
        for s in &sums {
            let net = s.debit - s.credit;
            if net == Decimal::ZERO {
                continue;
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
    ) -> anyhow::Result<BalanceSheet> {
        let sums = self.repo.account_sums(company_id, None, as_of).await?;
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
    ) -> anyhow::Result<IncomeStatement> {
        let sums = self
            .repo
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
