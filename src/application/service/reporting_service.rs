//! Financial-report generation — Trial Balance, Balance Sheet, Income Statement, General
//! Ledger, Partner Ledger, Aged Receivables / Payables.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Application orchestration over the
//! `ReportingRepository` port — no `sqlx`/`PgPool` here. Computes every report from the
//! immutable ledger (never from cached `accounts.current_balance`, so any as-of date / period
//! works). Proven by `tests/reporting_golden_cases.rs`.
//!
//! Sign convention: a debit-normal account's balance = Σdebit−Σcredit; a credit-normal account's
//! balance = Σcredit−Σdebit. The global ledger is always balanced (Σdebit = Σcredit), so a Trial
//! Balance always foots and a Balance Sheet always balances via
//! `Assets = Liabilities + Equity + CurrentEarnings`.
//!
//! Reports are computed on the fly and never persisted — the `financial_statement` entity is
//! not the compute target (the Odoo community report engine is absent; these reads stand on
//! our own spec).

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use crate::domain::repositories::reporting_repository::{
    AccountNodeRow, PartyLedgerRow, ReportingRepository,
};

#[derive(Debug, Clone, Serialize)]
pub struct TrialBalanceLine {
    pub account_number: String,
    pub name: String,
    pub account_type: String,
    pub debit: Decimal,
    pub credit: Decimal,
}

/// One node of the rolled-up trial-balance tree (headers aggregate their detail descendants).
#[derive(Debug, Clone, Serialize)]
pub struct TrialBalanceNode {
    pub account_number: String,
    pub name: String,
    pub account_type: String,
    pub level: i32,
    pub is_header: bool,
    pub debit: Decimal,
    pub credit: Decimal,
    pub children: Vec<TrialBalanceNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrialBalance {
    pub company_id: Uuid,
    pub as_of: NaiveDate,
    pub lines: Vec<TrialBalanceLine>,
    pub tree: Vec<TrialBalanceNode>,
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

/// One chronological GL line as returned over HTTP.
#[derive(Debug, Clone, Serialize)]
pub struct GeneralLedgerLine {
    pub journal_number: String,
    pub transaction_date: NaiveDate,
    pub posting_date: NaiveDate,
    pub description: String,
    pub reference: Option<String>,
    pub currency: String,
    pub debit: Decimal,
    pub credit: Decimal,
    pub balance_after: Decimal,
    pub party_type: Option<String>,
    pub party_id: Option<Uuid>,
    pub source_type: Option<String>,
    pub source_reference: Option<String>,
    pub is_reconciled: bool,
}

/// One account section of the GL.
#[derive(Debug, Clone, Serialize)]
pub struct GeneralLedgerSection {
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    pub opening_balance: Decimal,
    pub lines: Vec<GeneralLedgerLine>,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    pub closing_balance: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneralLedger {
    pub company_id: Uuid,
    pub from_date: Option<NaiveDate>,
    pub to_date: NaiveDate,
    pub limit: i64,
    pub offset: i64,
    pub sections: Vec<GeneralLedgerSection>,
}

/// One AR/AP line of a partner ledger.
#[derive(Debug, Clone, Serialize)]
pub struct PartnerLedgerLine {
    pub line_id: Uuid,
    pub journal_number: String,
    pub transaction_date: NaiveDate,
    pub account_number: String,
    pub account_name: String,
    pub description: Option<String>,
    pub debit: Decimal,
    pub credit: Decimal,
    pub currency: String,
    pub residual: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartnerLedger {
    pub company_id: Uuid,
    pub party_type: String,
    pub party_id: Uuid,
    pub as_of: NaiveDate,
    pub lines: Vec<PartnerLedgerLine>,
    pub total_debit: Decimal,
    pub total_credit: Decimal,
    /// Σ line residuals as of `as_of` — the party's open position.
    pub open_residual: Decimal,
}

/// One party's aged row: open residual bucketed by the age of the oldest open item's date.
#[derive(Debug, Clone, Serialize, Default)]
pub struct AgedPartyRow {
    pub party_type: String,
    pub party_id: Uuid,
    pub bucket_0_30: Decimal,
    pub bucket_31_60: Decimal,
    pub bucket_61_90: Decimal,
    pub bucket_91_plus: Decimal,
    pub total: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgedReport {
    pub company_id: Uuid,
    pub as_of: NaiveDate,
    pub account_subtype: String,
    pub parties: Vec<AgedPartyRow>,
    pub totals: AgedPartyRow,
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

    /// Build the rolled-up trial-balance tree: detail accounts carry their net debit/credit;
    /// every ancestor aggregates its descendants. The chart comes from the account directory,
    /// so headers with zero activity still appear (a flat chart degrades to the old output).
    fn trial_balance_tree(
        directory: &[AccountNodeRow],
        lines: &[(Uuid, Decimal, Decimal)], // (detail account id, debit, credit)
    ) -> Vec<TrialBalanceNode> {
        use std::collections::HashMap;

        // Detail sums per account id.
        let mut sums: HashMap<Uuid, (Decimal, Decimal)> = HashMap::new();
        for (id, debit, credit) in lines {
            let e = sums.entry(*id).or_insert((Decimal::ZERO, Decimal::ZERO));
            e.0 += *debit;
            e.1 += *credit;
        }
        // Propagate every detail's sums up its ancestor chain. Iterate an owned copy of the
        // detail entries — the map itself grows as ancestors aggregate.
        let by_id: HashMap<Uuid, &AccountNodeRow> = directory.iter().map(|n| (n.id, n)).collect();
        let detail_entries: Vec<(Uuid, Decimal, Decimal)> =
            sums.iter().map(|(id, (d, c))| (*id, *d, *c)).collect();
        for (id, debit, credit) in detail_entries {
            let mut cur = by_id.get(&id).and_then(|n| n.parent_id);
            // Step budget guards against a parent CYCLE (a→b→a): no constraint prevents
            // one in the accounts table, and an unguarded walk would hang the report.
            let mut steps = directory.len() + 1;
            while let Some(pid) = cur {
                if steps == 0 {
                    break; // cycle — aggregate what we walked, stop rather than loop
                }
                steps -= 1;
                if let Some(node) = by_id.get(&pid) {
                    let e = sums.entry(pid).or_insert((Decimal::ZERO, Decimal::ZERO));
                    e.0 += debit;
                    e.1 += credit;
                    cur = node.parent_id;
                } else {
                    break; // dangling parent — stop rather than loop
                }
            }
        }
        // Materialize the tree, children ordered by account number.
        let mut children_of: HashMap<Uuid, Vec<&AccountNodeRow>> = HashMap::new();
        let mut roots: Vec<&AccountNodeRow> = Vec::new();
        for n in directory {
            match n.parent_id {
                Some(p) => children_of.entry(p).or_default().push(n),
                None => roots.push(n),
            }
        }
        fn build(
            n: &AccountNodeRow,
            children_of: &HashMap<Uuid, Vec<&AccountNodeRow>>,
            sums: &HashMap<Uuid, (Decimal, Decimal)>,
        ) -> TrialBalanceNode {
            let (debit, credit) = sums
                .get(&n.id)
                .cloned()
                .unwrap_or((Decimal::ZERO, Decimal::ZERO));
            let mut kids: Vec<TrialBalanceNode> = children_of
                .get(&n.id)
                .map(|cs| {
                    let mut v: Vec<&AccountNodeRow> = cs.clone();
                    v.sort_by(|a, b| a.account_number.cmp(&b.account_number));
                    v.into_iter().map(|c| build(c, children_of, sums)).collect()
                })
                .unwrap_or_default();
            // Zero-activity headers with no zero-activity children collapse away.
            kids.retain(|k| {
                k.debit != Decimal::ZERO || k.credit != Decimal::ZERO || !k.children.is_empty()
            });
            TrialBalanceNode {
                account_number: n.account_number.clone(),
                name: n.name.clone(),
                account_type: n.account_type.clone(),
                level: n.level,
                is_header: n.is_header,
                debit,
                credit,
                children: kids,
            }
        }
        roots.sort_by(|a, b| a.account_number.cmp(&b.account_number));
        let mut tree: Vec<TrialBalanceNode> = roots
            .iter()
            .map(|r| build(r, &children_of, &sums))
            .collect();
        tree.retain(|t| {
            t.debit != Decimal::ZERO || t.credit != Decimal::ZERO || !t.children.is_empty()
        });
        tree
    }

    pub async fn trial_balance(
        &self,
        company_id: Uuid,
        as_of: NaiveDate,
    ) -> anyhow::Result<TrialBalance> {
        let (sums, directory) = tokio::join!(
            self.repo.account_sums(company_id, None, as_of),
            self.repo.account_directory(company_id)
        );
        let sums = sums?;
        let directory = directory?;
        let mut lines = Vec::new();
        let mut total_debit = Decimal::ZERO;
        let mut total_credit = Decimal::ZERO;
        let mut detail_net: Vec<(Uuid, Decimal, Decimal)> = Vec::new();
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
            detail_net.push((s.account_id, debit, credit));
            lines.push(TrialBalanceLine {
                account_number: s.account_number.clone(),
                name: s.name.clone(),
                account_type: s.account_type.clone(),
                debit,
                credit,
            });
        }
        let tree = Self::trial_balance_tree(&directory, &detail_net);
        Ok(TrialBalance {
            company_id,
            as_of,
            lines,
            tree,
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

    /// General ledger: per-account chronological sections with opening/closing balances.
    /// `limit` is clamped to 1..=1000 (default 200); `offset` pages within the same window.
    pub async fn general_ledger(
        &self,
        company_id: Uuid,
        account_id: Option<Uuid>,
        from_date: Option<NaiveDate>,
        to_date: NaiveDate,
        limit: Option<i64>,
        offset: i64,
    ) -> anyhow::Result<GeneralLedger> {
        let limit = limit.unwrap_or(200).clamp(1, 1000);
        let offset = offset.max(0);
        let rows = self
            .repo
            .gl_lines(company_id, account_id, from_date, to_date, limit, offset)
            .await?;

        // Group consecutive rows by account (the read is ordered by account number).
        let mut sections: Vec<GeneralLedgerSection> = Vec::new();
        for r in rows {
            let start_new = sections
                .last()
                .map(|s| s.account_id != r.account_id)
                .unwrap_or(true);
            if start_new {
                sections.push(GeneralLedgerSection {
                    account_id: r.account_id,
                    account_number: r.account_number.clone(),
                    account_name: r.account_name.clone(),
                    // The materialized running balance: the first line in the window
                    // opens at its balance_before (absolute since inception).
                    opening_balance: r.balance_before,
                    lines: Vec::new(),
                    total_debit: Decimal::ZERO,
                    total_credit: Decimal::ZERO,
                    closing_balance: r.balance_before,
                });
            }
            let section = sections.last_mut().expect("section just pushed");
            section.lines.push(GeneralLedgerLine {
                journal_number: r.journal_number.clone(),
                transaction_date: r.transaction_date,
                posting_date: r.posting_date,
                description: r.description.clone(),
                reference: r.reference.clone(),
                currency: r.currency.clone(),
                debit: r.debit,
                credit: r.credit,
                balance_after: r.balance_after,
                party_type: r.party_type.clone(),
                party_id: r.party_id,
                source_type: r.source_type.clone(),
                source_reference: r.source_reference.clone(),
                is_reconciled: r.is_reconciled,
            });
            section.total_debit += r.debit;
            section.total_credit += r.credit;
            section.closing_balance = r.balance_after;
        }
        Ok(GeneralLedger {
            company_id,
            from_date,
            to_date,
            limit,
            offset,
            sections,
        })
    }

    /// Partner ledger: every party-stamped line through `as_of`, each with its residual as of
    /// that date. `open_residual` is the party's still-open position.
    pub async fn partner_ledger(
        &self,
        company_id: Uuid,
        party_type: &str,
        party_id: Uuid,
        as_of: NaiveDate,
    ) -> anyhow::Result<PartnerLedger> {
        let rows = self
            .repo
            .party_ledger_lines(company_id, party_type, party_id, as_of)
            .await?;
        let mut total_debit = Decimal::ZERO;
        let mut total_credit = Decimal::ZERO;
        let mut open_residual = Decimal::ZERO;
        let lines: Vec<PartnerLedgerLine> = rows
            .into_iter()
            .map(|r: PartyLedgerRow| {
                total_debit += r.debit;
                total_credit += r.credit;
                open_residual += r.residual;
                PartnerLedgerLine {
                    line_id: r.line_id,
                    journal_number: r.journal_number,
                    transaction_date: r.transaction_date,
                    account_number: r.account_number,
                    account_name: r.account_name,
                    description: r.description,
                    debit: r.debit,
                    credit: r.credit,
                    currency: r.currency,
                    residual: r.residual,
                }
            })
            .collect();
        Ok(PartnerLedger {
            company_id,
            party_type: party_type.to_string(),
            party_id,
            as_of,
            lines,
            total_debit,
            total_credit,
            open_residual,
        })
    }

    /// Aged receivables/payables: open items bucketed by age ((as_of − item date) days).
    /// Buckets are fixed 0–30 / 31–60 / 61–90 / 91+; `account_subtype` selects the side
    /// (`accounts_receivable` / `accounts_payable`).
    pub async fn aged_report(
        &self,
        company_id: Uuid,
        account_subtype: &str,
        as_of: NaiveDate,
    ) -> anyhow::Result<AgedReport> {
        let items = self
            .repo
            .aged_open_items(company_id, account_subtype, as_of)
            .await?;
        let mut parties: BTreeMap<(String, Uuid), AgedPartyRow> = BTreeMap::new();
        let mut totals = AgedPartyRow::default();
        for it in items {
            let days = (as_of - it.transaction_date).num_days();
            let row = parties
                .entry((it.party_type.clone(), it.party_id))
                .or_insert_with(|| AgedPartyRow {
                    party_type: it.party_type.clone(),
                    party_id: it.party_id,
                    ..Default::default()
                });
            Self::bucket(row, days, it.residual);
            Self::bucket(&mut totals, days, it.residual);
        }
        Ok(AgedReport {
            company_id,
            as_of,
            account_subtype: account_subtype.to_string(),
            parties: parties.into_values().collect(),
            totals,
        })
    }

    pub async fn aged_receivables(
        &self,
        company_id: Uuid,
        as_of: NaiveDate,
    ) -> anyhow::Result<AgedReport> {
        self.aged_report(company_id, "accounts_receivable", as_of)
            .await
    }

    pub async fn aged_payables(
        &self,
        company_id: Uuid,
        as_of: NaiveDate,
    ) -> anyhow::Result<AgedReport> {
        self.aged_report(company_id, "accounts_payable", as_of)
            .await
    }

    fn bucket(row: &mut AgedPartyRow, days: i64, amount: Decimal) {
        row.total += amount;
        if days <= 30 {
            row.bucket_0_30 += amount;
        } else if days <= 60 {
            row.bucket_31_60 += amount;
        } else if days <= 90 {
            row.bucket_61_90 += amount;
        } else {
            row.bucket_91_plus += amount;
        }
    }
}
