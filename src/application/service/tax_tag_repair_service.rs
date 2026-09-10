//! Tax-tag legal-change repair — a guarded officer verb, never a cron.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). After a legal
//! change moves the correct tax-report tagging (a repartition re-issue, a new
//! report tag), already-POSTED journal lines still carry the assignment that
//! was correct when they posted. This verb recomputes those assignments for a
//! date window: it applies an explicit, officer-supplied rule set to the
//! `tags` jsonb on posted journal lines and stamps an audit row per run.
//!
//! Guards (all typed refusals, all fail-closed):
//! - **Unit-fenced** — the run relays the ambient org scope onto its
//!   transaction (the composing service's tenancy decorator's fence), and the
//!   HTTP layer rejects a body company that disagrees with the ambient scope.
//! - **Reason mandatory** — `reason_required`; an empty justification refuses.
//! - **Lock posture** — a window overlapping a LOCKED fiscal period refuses
//!   outright (`locked_period_in_window`); overlapping a CLOSED period refuses
//!   unless the officer explicitly passes the override
//!   (`closed_period_requires_override`). This is the local rendering of the
//!   tax-lock-date posture, hardened from warn to refuse.
//! - **Audit-stamped** — one `tax_tag_repair_runs` row per run (dry runs
//!   included): window, the exact rules applied, counts, override flag,
//!   officer, reason.
//!
//! The rule set arrives as a parameter because the tax repartition it encodes
//! lives in the tax module and this module deliberately holds no Cargo edge
//! into it — the composing host reads the current repartition there and passes
//! the mapping here. The rules are applied set-at-a-time in SQL inside the
//! verb's transaction; the lines' amounts, accounts, and party data are never
//! touched.
//!
//! Tenancy (ADR-0029): the module carries no tenancy of its own — the composing
//! service's tenancy decorator owns org scoping. The `company_id` lanes here are
//! the documented legacy twin: request shapes and verb signatures keep them so
//! unstripped callers compile and run unchanged, but no statement keys on a
//! tenant column. An undecorated deployment has no ambient scope and skips the
//! relay entirely (unfenced by design).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// One re-assignment rule: which posted lines it selects, and the tag set
/// those lines must carry afterwards. At least one selector must be set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxTagRule {
    /// Restrict to lines posted on this GL account (None = any).
    pub account_id: Option<Uuid>,
    /// Restrict to tax lines (true) / base lines (false) (None = any).
    pub is_tax_line: Option<bool>,
    /// The tax-report tag identifiers (as issued by the tax module) the
    /// selected lines must carry after the repair.
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RepairRequest {
    /// The legacy tenancy twin (ADR-0029) — kept so unstripped callers compile and
    /// run unchanged; no statement keys on it.
    pub company_id: Uuid,
    pub date_from: NaiveDate,
    pub date_to: NaiveDate,
    pub rules: Vec<TaxTagRule>,
    /// Explicit override for windows overlapping CLOSED fiscal periods.
    pub allow_closed_periods: bool,
    /// Count and report without writing.
    pub dry_run: bool,
    /// The officer running the repair.
    pub actor: Option<Uuid>,
    /// Mandatory legal-change justification.
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleReport {
    pub account_id: Option<Uuid>,
    pub is_tax_line: Option<bool>,
    pub tags: Vec<String>,
    pub lines_examined: i64,
    pub lines_retagged: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairReport {
    pub run_id: Uuid,
    pub dry_run: bool,
    pub lines_examined: i64,
    pub lines_retagged: i64,
    pub per_rule: Vec<RuleReport>,
    /// Fiscal periods the run crossed with the override armed (audit copy).
    pub closed_periods_overridden: Vec<String>,
}

#[derive(Debug)]
pub enum TaxTagRepairError {
    /// The justification string is empty or whitespace.
    ReasonRequired,
    /// date_from is after date_to.
    WindowInverted,
    /// A rule selects nothing (both selectors null).
    RuleWithoutSelector(usize),
    /// A rule carries an empty tag set.
    RuleWithoutTags(usize),
    /// The window overlaps a LOCKED fiscal period — no override exists.
    LockedPeriodInWindow(Vec<String>),
    /// The window overlaps a CLOSED fiscal period and the override is unset.
    ClosedPeriodRequiresOverride(Vec<String>),
    /// Storage failure.
    Internal(String),
}

impl TaxTagRepairError {
    pub fn code(&self) -> &'static str {
        match self {
            TaxTagRepairError::ReasonRequired => "reason_required",
            TaxTagRepairError::WindowInverted => "window_inverted",
            TaxTagRepairError::RuleWithoutSelector(_) => "rule_without_selector",
            TaxTagRepairError::RuleWithoutTags(_) => "rule_without_tags",
            TaxTagRepairError::LockedPeriodInWindow(_) => "locked_period_in_window",
            TaxTagRepairError::ClosedPeriodRequiresOverride(_) => {
                "closed_period_requires_override"
            }
            TaxTagRepairError::Internal(_) => "internal_error",
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            TaxTagRepairError::Internal(_) => 500,
            _ => 422,
        }
    }
}

impl std::fmt::Display for TaxTagRepairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaxTagRepairError::ReasonRequired => write!(
                f,
                "reason_required: a legal-change repair must carry its justification"
            ),
            TaxTagRepairError::WindowInverted => {
                write!(f, "window_inverted: date_from is after date_to")
            }
            TaxTagRepairError::RuleWithoutSelector(i) => write!(
                f,
                "rule_without_selector: rule #{i} has neither account_id nor is_tax_line"
            ),
            TaxTagRepairError::RuleWithoutTags(i) => write!(
                f,
                "rule_without_tags: rule #{i} carries an empty tag set"
            ),
            TaxTagRepairError::LockedPeriodInWindow(periods) => write!(
                f,
                "locked_period_in_window: {} — locked periods are never retaggable",
                periods.join(", ")
            ),
            TaxTagRepairError::ClosedPeriodRequiresOverride(periods) => write!(
                f,
                "closed_period_requires_override: {} — pass allow_closed_periods=true to proceed",
                periods.join(", ")
            ),
            TaxTagRepairError::Internal(e) => write!(f, "internal_error: {e}"),
        }
    }
}
impl std::error::Error for TaxTagRepairError {}

fn internal(e: impl std::fmt::Display) -> TaxTagRepairError {
    TaxTagRepairError::Internal(e.to_string())
}

#[derive(Clone)]
pub struct TaxTagRepairService {
    pool: PgPool,
}

impl TaxTagRepairService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// List the audit trail (most recent first) — the read side of the ledger.
    /// `company_id` is the legacy tenancy twin (ADR-0029): kept in the signature
    /// for unstripped callers, unused here.
    pub async fn list_runs(
        &self,
        _company_id: Uuid,
        limit: i64,
    ) -> Result<Vec<RepairRunRow>, TaxTagRepairError> {
        let mut tx = self.pool.begin().await.map_err(|e| internal(e))?;
        // Tenancy posture (ADR-0029): relay the AMBIENT request scope when the
        // caller bound one; an undecorated deployment skips this entirely.
        if let Some(scope) = backbone_orm::org_scope::current_org_scope() {
            backbone_orm::org_scope::bind_org_scope_on(&mut tx, &scope)
                .await
                .map_err(|e| internal(e))?;
        }
        let rows = sqlx::query_as::<_, RepairRunRow>(
            r#"SELECT id, date_from, date_to, rules, lines_examined, lines_retagged,
                      dry_run, overridden_closed_periods, actor, reason, ran_at
                 FROM accounting.tax_tag_repair_runs
                ORDER BY ran_at DESC
                LIMIT GREATEST(1, LEAST($1, 200))"#,
        )
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| internal(e))?;
        tx.commit().await.map_err(|e| internal(e))?;
        Ok(rows)
    }

    /// Recompute tax-tag assignments for posted lines in a date window.
    pub async fn recompute(
        &self,
        req: RepairRequest,
    ) -> Result<RepairReport, TaxTagRepairError> {
        if req.reason.trim().is_empty() {
            return Err(TaxTagRepairError::ReasonRequired);
        }
        if req.date_from > req.date_to {
            return Err(TaxTagRepairError::WindowInverted);
        }
        if req.rules.is_empty() {
            return Err(TaxTagRepairError::RuleWithoutTags(0));
        }
        for (i, rule) in req.rules.iter().enumerate() {
            if rule.account_id.is_none() && rule.is_tax_line.is_none() {
                return Err(TaxTagRepairError::RuleWithoutSelector(i));
            }
            if rule.tags.is_empty() {
                return Err(TaxTagRepairError::RuleWithoutTags(i));
            }
        }

        let mut tx = self.pool.begin().await.map_err(|e| internal(e))?;
        // Tenancy posture (ADR-0029): relay the AMBIENT request scope when the
        // caller bound one; an undecorated deployment skips this entirely.
        if let Some(scope) = backbone_orm::org_scope::current_org_scope() {
            backbone_orm::org_scope::bind_org_scope_on(&mut tx, &scope)
                .await
                .map_err(|e| internal(e))?;
        }

        // Lock-posture guards: the window must not cross a locked period, and
        // closed periods demand an explicit override.
        let overlapping = |status: &str| {
            format!(
                r#"SELECT period_code FROM accounting.fiscal_periods
                    WHERE status='{status}'
                      AND start_date <= $2 AND end_date >= $1
                    ORDER BY period_code"#
            )
        };
        let locked: Vec<String> = sqlx::query_scalar(&overlapping("locked"))
            .bind(req.date_from)
            .bind(req.date_to)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| internal(e))?;
        if !locked.is_empty() {
            return Err(TaxTagRepairError::LockedPeriodInWindow(locked));
        }
        let closed: Vec<String> = sqlx::query_scalar(&overlapping("closed"))
            .bind(req.date_from)
            .bind(req.date_to)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| internal(e))?;
        if !closed.is_empty() && !req.allow_closed_periods {
            return Err(TaxTagRepairError::ClosedPeriodRequiresOverride(closed));
        }

        let mut per_rule: Vec<RuleReport> = Vec::with_capacity(req.rules.len());
        for rule in &req.rules {
            // Dedup tags preserving order; serialize once for both the count
            // filter and the write so the two arms see the same value.
            let mut seen = std::collections::HashSet::new();
            let tags: Vec<String> = rule
                .tags
                .iter()
                .filter(|t| seen.insert((*t).clone()))
                .cloned()
                .collect();
            let tags_json = serde_json::to_value(&tags).map_err(|e| internal(e))?;

            // The selector placeholders are ALWAYS bound: a NULL selector
            // neutralizes its arm, which keeps the SQL shape (and the bind
            // count) constant regardless of which selectors a rule sets.
            // Postgres binds parameters positionally, so each statement gets
            // its own predicate with DENSE placeholder numbering — reusing a
            // predicate whose selector slots sit at $4/$5 in a statement that
            // binds no $3 shifts every later bind by one (the boolean lands in
            // the uuid slot and the call dies on "cannot cast type boolean to
            // uuid"). Journals soft-delete through their metadata bag (no
            // deleted_at column), so the live-row filter reads the metadata
            // key.
            let predicate = |acct: usize, tax: usize| {
                format!(
                    r#"jl.is_posted
                    AND (j.metadata->>'deleted_at') IS NULL
                    AND j.transaction_date BETWEEN $1 AND $2
                    AND (${acct}::uuid IS NULL OR jl.account_id = ${acct})
                    AND (${tax}::boolean IS NULL OR jl.is_tax_line = ${tax})"#
                )
            };

            let (examined, changed) = if req.dry_run {
                let row: (i64, i64) = sqlx::query_as(&format!(
                    r#"SELECT COUNT(*),
                              COUNT(*) FILTER (WHERE jl.tags IS DISTINCT FROM $3)
                         FROM accounting.journal_lines jl
                         JOIN accounting.journals j ON j.id = jl.journal_id
                        WHERE {}"#,
                    predicate(4, 5)
                ))
                .bind(req.date_from)
                .bind(req.date_to)
                .bind(&tags_json)
                .bind(rule.account_id)
                .bind(rule.is_tax_line)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| internal(e))?;
                (row.0, row.1)
            } else {
                // Set-at-a-time rewrite. journal_lines carries no updated_at
                // column (no audit metadata on lines) — the audit trail is the
                // repair-run row, not a per-line stamp.
                let changed = sqlx::query(&format!(
                    r#"UPDATE accounting.journal_lines jl
                          SET tags = $3
                         FROM accounting.journals j
                        WHERE j.id = jl.journal_id
                          AND {}
                          AND jl.tags IS DISTINCT FROM $3"#,
                    predicate(4, 5)
                ))
                .bind(req.date_from)
                .bind(req.date_to)
                .bind(&tags_json)
                .bind(rule.account_id)
                .bind(rule.is_tax_line)
                .execute(&mut *tx)
                .await
                .map_err(|e| internal(e))?
                .rows_affected() as i64;
                // This statement binds no tag value, so its selector slots are
                // $3/$4, not $4/$5.
                let examined = sqlx::query_scalar(&format!(
                    r#"SELECT COUNT(*)
                         FROM accounting.journal_lines jl
                         JOIN accounting.journals j ON j.id = jl.journal_id
                        WHERE {}"#,
                    predicate(3, 4)
                ))
                .bind(req.date_from)
                .bind(req.date_to)
                .bind(rule.account_id)
                .bind(rule.is_tax_line)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| internal(e))?;
                (examined, changed)
            };

            per_rule.push(RuleReport {
                account_id: rule.account_id,
                is_tax_line: rule.is_tax_line,
                tags: rule.tags.clone(),
                lines_examined: examined,
                lines_retagged: changed,
            });
        }

        let rules_json =
            serde_json::to_value(&req.rules).map_err(|e| internal(e))?;
        let lines_examined: i64 = per_rule.iter().map(|r| r.lines_examined).sum();
        let lines_retagged: i64 = per_rule.iter().map(|r| r.lines_retagged).sum();
        let overridden_json =
            serde_json::to_value(&closed).map_err(|e| internal(e))?;

        let run_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO accounting.tax_tag_repair_runs
                 (date_from, date_to, rules, lines_examined,
                  lines_retagged, dry_run, overridden_closed_periods, actor,
                  reason, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,NOW())
               RETURNING id"#,
        )
        .bind(req.date_from)
        .bind(req.date_to)
        .bind(&rules_json)
        .bind(lines_examined)
        .bind(lines_retagged)
        .bind(req.dry_run)
        .bind(&overridden_json)
        .bind(req.actor)
        .bind(req.reason.trim())
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| internal(e))?;

        tx.commit().await.map_err(|e| internal(e))?;
        Ok(RepairReport {
            run_id,
            dry_run: req.dry_run,
            lines_examined,
            lines_retagged,
            per_rule,
            closed_periods_overridden: closed,
        })
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct RepairRunRow {
    pub id: Uuid,
    pub date_from: NaiveDate,
    pub date_to: NaiveDate,
    pub rules: serde_json::Value,
    pub lines_examined: i64,
    pub lines_retagged: i64,
    pub dry_run: bool,
    pub overridden_closed_periods: serde_json::Value,
    pub actor: Option<Uuid>,
    pub reason: String,
    pub ran_at: chrono::DateTime<chrono::Utc>,
}
