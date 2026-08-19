//! Reconciliation write service — the verbs over the reconciliation graph.
//!
//! Hand-authored behavior (NOT generated). The **application** orchestration layer: it
//! loads lines through the `ReconcileGraphRepository` port (connection-taking — an edge
//! commits atomically with the caller's unit of work), validates with the pure guards
//! (`domain::services::reconcile_rules`), applies the CLAMP, and maintains the graph:
//! partial edges, full-reconcile groups, generated exchange-difference moves, and the
//! side-effecting unlink.
//!
//! The unlink contract (Odoo `account.partial.reconcile.unlink`, AF12/TF13): removing an
//! edge is NEVER a bare DELETE. Every move the edge generated is first reversed through
//! the GL-posting contract (a real reversal journal stamped `source_type='reconciliation'`,
//! `source_id=<partial id>` — the hook cash-basis tax deferrals ride later), then the
//! flags/groups are repaired, and only then the edge rows go away.
//!
//! The reverse-then-reconcile rule (`pair_reversal_counterparts`) runs INSIDE this
//! service after every link and unlink: when a line's residual returns to its full face
//! and the same source has a posted reversal journal on the same account, the two pair —
//! which structurally removes the N-asynchronous-unlinks ordering hazard for producers
//! (a reversed payment's lines end up fully reconciled against its reversal regardless of
//! allocation order).
//!
//! This file is user-owned (see `metaphor.codegen.yaml`) and survives regeneration.

use std::sync::Arc;

use chrono::{Datelike, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use backbone_orm::company_scope;

use crate::domain::gl_posting::PostingLine;
use crate::domain::reconcile_graph::{
    EdgeOutcome, LineLocator, LocatorResolution, MatchingGroup, NewPartial, PairRequest,
    PartyResidual, ReconcileError, ReconcileLineSnapshot, ORIGIN_MANUAL,
};
use crate::domain::repositories::posting_repository::{PostingRepository, PostingWrite};
use crate::domain::repositories::reconcile_graph_repository::ReconcileGraphRepository;
use crate::domain::services::reconcile_rules;

pub use crate::domain::reconcile_graph::PartialRow;

/// Map a persistence (`anyhow`) failure into the reconcile error type.
fn internal(e: anyhow::Error) -> ReconcileError {
    ReconcileError::Internal(e.to_string())
}

/// Round to the company-currency scale (2dp).
fn round2(d: Decimal) -> Decimal {
    d.round_dp(2)
}

/// Residual lookup by line id (the repo returns rows ordered by id, not by request).
fn res_of(residuals: &[(Uuid, Decimal)], line_id: Uuid) -> Decimal {
    residuals
        .iter()
        .find(|(id, _)| *id == line_id)
        .map(|(_, r)| *r)
        .unwrap_or(Decimal::ZERO)
}

/// A zero/abs posting line pair builder for the exchange move (one FX line, one
/// reconcilable-account line, mirrored orientation).
fn fx_pair(
    fx_account: Uuid,
    account_id: Uuid,
    party_type: Option<String>,
    party_id: Option<Uuid>,
    diff: Decimal,
) -> (PostingLine, PostingLine) {
    let abs = diff.abs();
    let desc = Some("exchange difference".to_string());
    let fx_dr = diff > Decimal::ZERO;
    let (fx_d, fx_c) = if fx_dr { (abs, Decimal::ZERO) } else { (Decimal::ZERO, abs) };
    let (ar_d, ar_c) = if fx_dr { (Decimal::ZERO, abs) } else { (abs, Decimal::ZERO) };
    (
        PostingLine {
            account_id: fx_account,
            debit: fx_d,
            credit: fx_c,
            party_type: None,
            party_id: None,
            cost_center_id: None,
            project_id: None,
            department_id: None,
            description: desc.clone(),
        },
        PostingLine {
            account_id,
            debit: ar_d,
            credit: ar_c,
            party_type,
            party_id,
            cost_center_id: None,
            project_id: None,
            department_id: None,
            description: desc,
        },
    )
}

/// The reconciliation write service. Holds the graph port, the posting port (for
/// generated-move reversals), and the optional FX account — fail-closed when an exchange
/// difference arises without one. The pool is used ONLY by the pool-verb wrappers (the
/// HTTP surface); the `_on` verbs ride the caller's connection.
#[derive(Clone)]
pub struct ReconcileWriteService {
    repo: Arc<dyn ReconcileGraphRepository>,
    posting: Arc<dyn PostingRepository>,
    pool: PgPool,
    exchange_account_id: Option<Uuid>,
}

impl ReconcileWriteService {
    pub fn new(
        repo: Arc<dyn ReconcileGraphRepository>,
        posting: Arc<dyn PostingRepository>,
        pool: PgPool,
        exchange_account_id: Option<Uuid>,
    ) -> Self {
        Self { repo, posting, pool, exchange_account_id }
    }

    // =========================================================================
    // Link (conn-taking) — the settlement / clearing / manual verb
    // =========================================================================

    /// Create (or grow) the debit↔credit edge between two located lines, riding the
    /// caller's transaction. CLAMP: the applied amount is `min(requested, residual_d,
    /// residual_c)`; a zero clamp is a no-op success (no edge; the on-account remainder
    /// stays unreconciled).
    pub async fn reconcile_pair_on(
        &self,
        conn: &mut sqlx::PgConnection,
        req: &PairRequest,
    ) -> Result<EdgeOutcome, ReconcileError> {
        // Resolve + lock both sides; concurrent reconciles over a shared line serialize
        // on the line lock.
        let mut lines = Vec::with_capacity(2);
        for locator in [&req.debit, &req.credit] {
            match self
                .repo
                .lock_line_by_locator(conn, req.company_id, locator)
                .await
                .map_err(|e| internal(e.into()))?
            {
                LocatorResolution::One(l) => lines.push(l),
                LocatorResolution::NotFound => return Err(ReconcileError::LineNotFound),
                LocatorResolution::Ambiguous(n) => {
                    return Err(ReconcileError::AmbiguousLocator(n))
                }
            }
        }
        let (debit, credit) = (lines[0].clone(), lines[1].clone());

        // Guards + clamp.
        let flags = self
            .repo
            .account_flags(conn, req.company_id, debit.account_id)
            .await
            .map_err(|e| internal(e.into()))?
            .ok_or_else(|| ReconcileError::Conflict("account not found".into()))?;
        let residuals = self
            .repo
            .residuals_of(conn, req.company_id, &[debit.id, credit.id])
            .await
            .map_err(|e| internal(e.into()))?;
        let applied = reconcile_rules::validate_pair(
            req.company_id,
            &debit,
            &credit,
            &flags,
            req.amount,
            res_of(&residuals, debit.id),
            res_of(&residuals, credit.id),
        )?;
        if applied == Decimal::ZERO {
            return Ok(EdgeOutcome {
                partial_id: None,
                applied: Decimal::ZERO,
                full_reconcile_id: None,
            });
        }

        let now = Utc::now();
        let max_date = debit.transaction_date.max(credit.transaction_date);
        let partial_id = self
            .repo
            .insert_partial(
                conn,
                &NewPartial {
                    company_id: req.company_id,
                    debit_move_id: debit.id,
                    credit_move_id: credit.id,
                    amount: applied,
                    currency: debit.currency.clone(),
                    max_date,
                    origin: req.origin.clone(),
                    source_type: Some(req.origin.clone()),
                    source_id: credit.source_id,
                    metadata: serde_json::json!({
                        "actor": req.actor,
                        "debit_source": debit.source_type,
                        "credit_source": credit.source_type,
                    }),
                },
            )
            .await
            .map_err(|e| internal(e.into()))?;

        // Exchange difference — only when the posting rates differ AND exactly one side's
        // residual hit zero with the other retaining the predicted rate delta
        // (`applied × (max_rate/min_rate − 1)`). Anything else (partial application,
        // under-payment, plain remainder) leaves the residual open on purpose. Normal IDR
        // posts carry rate 1 and never enter this branch; probes seed rate ≠ 1.
        let mut exchange_total = Decimal::ZERO;
        if debit.exchange_rate != credit.exchange_rate {
            let after = self
                .repo
                .residuals_of(conn, req.company_id, &[debit.id, credit.id])
                .await
                .map_err(|e| internal(e.into()))?;
            let res_d = res_of(&after, debit.id);
            let res_c = res_of(&after, credit.id);
            let (hi, lo) = if debit.exchange_rate > credit.exchange_rate {
                (debit.exchange_rate, credit.exchange_rate)
            } else {
                (credit.exchange_rate, debit.exchange_rate)
            };
            let predicted = round2(applied * (hi - lo) / lo);
            let tolerance = Decimal::new(5, 3); // half a company-currency cent
            // Signed from the reconcilable account's perspective: positive = the account
            // takes a credit (the debit side retained the delta), negative = a debit.
            let diff = if res_c == Decimal::ZERO
                && res_d > Decimal::ZERO
                && (res_d - predicted).abs() <= tolerance
            {
                Some(res_d)
            } else if res_d == Decimal::ZERO
                && res_c > Decimal::ZERO
                && (res_c - predicted).abs() <= tolerance
            {
                Some(-res_c)
            } else {
                None
            };
            if let Some(diff) = diff {
                let journal_id = self
                    .post_exchange_move(conn, req, &debit, partial_id, diff, max_date)
                    .await?;
                self.repo
                    .set_exchange_move(conn, req.company_id, partial_id, journal_id)
                    .await
                    .map_err(|e| internal(e.into()))?;
                // The derived second edge: pair the exchange journal's reconcilable line
                // with the original line that retained the delta, so the whole component
                // reaches zero residual together.
                let exch_line = self
                    .repo
                    .journal_lines_with_ids(conn, req.company_id, journal_id)
                    .await
                    .map_err(|e| internal(e.into()))?
                    .into_iter()
                    .find(|(_, l)| l.account_id == debit.account_id)
                    .map(|(id, _)| id)
                    .ok_or_else(|| {
                        ReconcileError::Internal("exchange journal missing account line".into())
                    })?;
                let (d_id, c_id) = if diff > Decimal::ZERO {
                    (debit.id, exch_line)
                } else {
                    (exch_line, credit.id)
                };
                self.repo
                    .insert_partial(
                        conn,
                        &NewPartial {
                            company_id: req.company_id,
                            debit_move_id: d_id,
                            credit_move_id: c_id,
                            amount: diff.abs(),
                            currency: debit.currency.clone(),
                            max_date,
                            origin: req.origin.clone(),
                            source_type: Some(req.origin.clone()),
                            // Derived edges carry the parent partial id — the unlink
                            // closure walks this to take them out with their parent.
                            source_id: Some(partial_id),
                            metadata: serde_json::json!({
                                "rule": "exchange_pair",
                                "parent_partial": partial_id,
                            }),
                        },
                    )
                    .await
                    .map_err(|e| internal(e.into()))?;
                exchange_total = diff;
            }
        }

        // Full-group completion + reverse-then-reconcile pairing, then report.
        let full = self
            .complete_group_for(conn, req.company_id, &[debit.id, credit.id], exchange_total, now)
            .await
            .map_err(|e| internal(e.into()))?;
        self.pair_reversal_counterparts(conn, req.company_id, &[debit.id, credit.id])
            .await
            .map_err(|e| internal(e.into()))?;

        Ok(EdgeOutcome {
            partial_id: Some(partial_id),
            applied,
            full_reconcile_id: full,
        })
    }

    /// The exchange-difference adjustment journal: `source_type="reconciliation"`,
    /// `source_id=<partial id>`, `idempotency_key="exch:<partial id>"`,
    /// `posting_type="adjustment"`. One line on the FX account, one on the shared
    /// reconcilable account (party-stamped), oriented to true the rate difference.
    async fn post_exchange_move(
        &self,
        conn: &mut sqlx::PgConnection,
        req: &PairRequest,
        debit: &ReconcileLineSnapshot,
        partial_id: Uuid,
        diff: Decimal,
        posting_date: chrono::NaiveDate,
    ) -> Result<Uuid, ReconcileError> {
        let fx = self
            .exchange_account_id
            .ok_or(ReconcileError::ExchangeAccountUnconfigured)?;
        // G8 — the exchange-move date must fall in an open period.
        if self
            .repo
            .period_closed(conn, req.company_id, posting_date)
            .await
            .map_err(|e| internal(e.into()))?
        {
            return Err(ReconcileError::PeriodClosed);
        }

        let (fx_line, ar_line) = fx_pair(
            fx,
            debit.account_id,
            debit.party_type.clone(),
            debit.party_id,
            diff,
        );
        let write = PostingWrite {
            company_id: req.company_id,
            branch_id: None,
            source_type: "reconciliation".to_string(),
            source_id: partial_id,
            source_reference: Some(format!("exch:{partial_id}")),
            posting_date,
            fiscal_period_id: None,
            fiscal_year: posting_date.year(),
            fiscal_month: posting_date.month() as i32,
            currency: debit.currency.clone(),
            posting_type: "adjustment".to_string(),
            reverses_post_id: None,
            reverses_journal_id: None,
            description: Some("exchange difference on reconciliation".into()),
            idempotency_key: Some(format!("exch:{partial_id}")),
            posted_by: req.actor,
            now: Utc::now(),
            lines: vec![fx_line, ar_line],
        };
        let commit = self
            .posting
            .commit_posting_on(conn, write)
            .await
            .map_err(|e| internal(e.into()))?;
        Ok(commit.journal_id)
    }

    /// After a link/unlink: if every line in the affected component is at zero residual,
    /// create the full-reconcile group and stamp everything (Odoo sets the flags only at
    /// full-group completion). Returns the new group id, or `None` while the chain is
    /// still partial.
    async fn complete_group_for(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        seeds: &[Uuid],
        exchange_total: Decimal,
        now: chrono::DateTime<Utc>,
    ) -> anyhow::Result<Option<Uuid>> {
        let comp_lines = self.repo.component_line_ids(conn, company_id, seeds).await?;
        if comp_lines.is_empty() {
            return Ok(None);
        }
        // Lock the component in id order before deciding (two verbs completing
        // overlapping components must serialize).
        self.repo.lock_lines(conn, company_id, &comp_lines).await?;
        let residuals = self.repo.residuals_of(conn, company_id, &comp_lines).await?;
        if residuals.iter().any(|(_, r)| *r != Decimal::ZERO) {
            return Ok(None);
        }
        // Idempotence against a completion that already grouped (part of) this component — the
        // concurrent case: two verbs complete overlapping components, serialize on the line locks
        // above, and the waiter re-sees all-zero residuals because group creation touches no
        // partials. Without this check the waiter would mint a second group over already-grouped
        // lines, orphaning the first. One stamp = re-attach that group (also heals the unlink
        // path's partially-cleared survivor lines back onto their surviving group); divergent
        // stamps are unreachable by construction and left untouched.
        let stamps = self
            .repo
            .distinct_group_stamps(conn, company_id, &comp_lines)
            .await?;
        let group = match stamps.as_slice() {
            [] => self
                .repo
                .create_full_group(conn, company_id, exchange_total, now)
                .await?,
            [existing] => *existing,
            _ => return Ok(None),
        };
        let comp_partials = self
            .repo
            .component_partial_ids(conn, company_id, &comp_lines)
            .await?;
        self.repo
            .attach_group(conn, company_id, group, &comp_lines, &comp_partials, now)
            .await?;
        Ok(Some(group))
    }

    /// Reverse-then-reconcile pairing for the affected lines (see the module docs). Lines
    /// generated BY the reconciliation machinery itself are excluded — pairing an exchange
    /// move against its own reversal adds noise without settling anything.
    async fn pair_reversal_counterparts(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        line_ids: &[Uuid],
    ) -> anyhow::Result<()> {
        for id in line_ids {
            let line = self.repo.lock_line(conn, company_id, *id).await?;
            let Some(line) = line else { continue };
            if line.source_type.as_deref() == Some("reconciliation") {
                continue;
            }
            let residual = res_of(
                &self.repo.residuals_of(conn, company_id, &[line.id]).await?,
                line.id,
            );
            if !reconcile_rules::fully_unapplied(&line, residual) {
                continue;
            }
            if let Some(counterpart) =
                self.repo.reversal_counterpart(conn, company_id, &line).await?
            {
                let c_residual = res_of(
                    &self
                        .repo
                        .residuals_of(conn, company_id, &[counterpart.id])
                        .await?,
                    counterpart.id,
                );
                if !reconcile_rules::fully_unapplied(&counterpart, c_residual) {
                    continue;
                }
                let (d, c) = if line.debit_amount > Decimal::ZERO {
                    (line.clone(), counterpart)
                } else {
                    (counterpart, line.clone())
                };
                self.repo
                    .insert_partial(
                        conn,
                        &NewPartial {
                            company_id,
                            debit_move_id: d.id,
                            credit_move_id: c.id,
                            amount: d.base_amount.min(c.base_amount),
                            currency: d.currency.clone(),
                            max_date: d.transaction_date.max(c.transaction_date),
                            origin: ORIGIN_MANUAL.to_string(),
                            source_type: Some(ORIGIN_MANUAL.to_string()),
                            source_id: d.source_id,
                            metadata: serde_json::json!({"rule": "reverse_then_reconcile"}),
                        },
                    )
                    .await?;
                self.complete_group_for(
                    conn,
                    company_id,
                    &[d.id, c.id],
                    Decimal::ZERO,
                    Utc::now(),
                )
                .await?;
            }
        }
        Ok(())
    }

    // =========================================================================
    // Unlink (conn-taking) — the side-effecting verb
    // =========================================================================

    /// Unlink every partial between the located pair (side-effecting: reverses generated
    /// moves, repairs flags/groups, then deletes the edges). Idempotent — no partials
    /// between the pair is a no-op success.
    pub async fn unreconcile_pair_on(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        debit: &LineLocator,
        credit: &LineLocator,
    ) -> Result<(), ReconcileError> {
        let mut lines = Vec::with_capacity(2);
        for locator in [debit, credit] {
            match self
                .repo
                .lock_line_by_locator(conn, company_id, locator)
                .await
                .map_err(|e| internal(e.into()))?
            {
                LocatorResolution::One(l) => lines.push(l),
                LocatorResolution::NotFound => return Err(ReconcileError::LineNotFound),
                LocatorResolution::Ambiguous(n) => {
                    return Err(ReconcileError::AmbiguousLocator(n))
                }
            }
        }
        let mut closure = self
            .repo
            .partials_between(conn, company_id, lines[0].id, lines[1].id)
            .await
            .map_err(|e| internal(e.into()))?;
        if closure.is_empty() {
            return Ok(());
        }
        // Include edges the matched partials derived (the FX second edge).
        let mut seen: Vec<Uuid> = closure.iter().map(|p| p.id).collect();
        for p in closure.clone() {
            for d in self
                .repo
                .derived_partials(conn, company_id, p.id)
                .await
                .map_err(|e| internal(e.into()))?
            {
                if !seen.contains(&d.id) {
                    seen.push(d.id);
                    closure.push(d);
                }
            }
        }
        self.unlink_partials(conn, company_id, &closure).await
    }

    /// Unlink one partial by id (the HTTP verb). Fail-closed on a foreign partial.
    pub async fn unreconcile_on(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        partial_id: Uuid,
        actor: Option<Uuid>,
    ) -> Result<(), ReconcileError> {
        let partial = self
            .repo
            .load_partial(conn, company_id, partial_id)
            .await
            .map_err(|e| internal(e.into()))?
            .ok_or(ReconcileError::LineNotFound)?;
        // Lock the affected lines before mutating.
        self.repo
            .lock_lines(conn, company_id, &[partial.debit_move_id, partial.credit_move_id])
            .await
            .map_err(|e| internal(e.into()))?;
        let mut closure = vec![partial.clone()];
        for d in self
            .repo
            .derived_partials(conn, company_id, partial.id)
            .await
            .map_err(|e| internal(e.into()))?
        {
            if d.id != partial.id {
                closure.push(d);
            }
        }
        self.unlink_partials_with_actor(conn, company_id, &closure, actor)
            .await
    }

    /// The shared unlink core (no actor → the reversal's `posted_by` stays empty).
    async fn unlink_partials(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        closure: &[PartialRow],
    ) -> Result<(), ReconcileError> {
        self.unlink_partials_with_actor(conn, company_id, closure, None)
            .await
    }

    /// The unlink core: reverse generated moves → delete edges → repair flags/groups →
    /// re-run reverse-then-reconcile pairing.
    async fn unlink_partials_with_actor(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        closure: &[PartialRow],
        actor: Option<Uuid>,
    ) -> Result<(), ReconcileError> {
        let ids: Vec<Uuid> = closure.iter().map(|p| p.id).collect();

        // 1) Reverse every journal these partials generated (AF12 — never a bare DELETE).
        //    Idempotency-keyed per journal, so a retried unlink redelivers nothing.
        let journal_ids = self
            .repo
            .generated_journal_ids(conn, company_id, &ids)
            .await
            .map_err(|e| internal(e.into()))?;
        for jid in journal_ids {
            self.reverse_generated_journal(conn, company_id, jid, actor)
                .await?;
        }

        // 2) Snapshot the affected groups/lines from the closure, then delete the edges.
        let mut affected_groups: Vec<Uuid> =
            closure.iter().filter_map(|p| p.full_reconcile_id).collect();
        affected_groups.sort_unstable();
        affected_groups.dedup();
        let mut affected_lines: Vec<Uuid> = closure
            .iter()
            .flat_map(|p| [p.debit_move_id, p.credit_move_id])
            .collect();
        affected_lines.sort_unstable();
        affected_lines.dedup();

        self.repo
            .delete_partials(conn, company_id, &ids)
            .await
            .map_err(|e| internal(e.into()))?;

        // 3) Clear the flags on every affected line BEFORE any group dissolve (the
        //    dissolve's FK discipline depends on it). Zero-residual components get
        //    re-flagged onto a fresh group in step 4 — the observable Odoo outcome (fully
        //    reconciled lines carry a group) is preserved; only the group id is new.
        self.repo
            .clear_line_flags(conn, company_id, &affected_lines)
            .await
            .map_err(|e| internal(e.into()))?;
        for group in &affected_groups {
            let survivors = self
                .repo
                .group_partial_ids(conn, company_id, *group)
                .await
                .map_err(|e| internal(e.into()))?;
            if survivors.is_empty() {
                self.repo
                    .dissolve_group(conn, company_id, *group)
                    .await
                    .map_err(|e| internal(e.into()))?;
            }
        }

        // 4) Re-group any component whose lines all reached zero residual through OTHER
        //    edges (the deleted ones were not the whole story).
        self.complete_group_for(conn, company_id, &affected_lines, Decimal::ZERO, Utc::now())
            .await
            .map_err(|e| internal(e.into()))?;

        // 5) Reverse-then-reconcile for every affected line.
        self.pair_reversal_counterparts(conn, company_id, &affected_lines)
            .await
            .map_err(|e| internal(e.into()))?;
        Ok(())
    }

    /// Derive + commit the reversal of a reconciliation-generated journal through the
    /// GL-posting contract on the caller's connection. Debit/credit swapped; reversal date
    /// is TODAY (unlinking an old document must not bounce off its own closed period).
    async fn reverse_generated_journal(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        journal_id: Uuid,
        actor: Option<Uuid>,
    ) -> Result<(), ReconcileError> {
        let Some(meta) = self
            .repo
            .journal_reversal_meta(conn, company_id, journal_id)
            .await
            .map_err(|e| internal(e.into()))?
        else {
            return Ok(());
        };
        let lines = self
            .repo
            .journal_lines_with_ids(conn, company_id, journal_id)
            .await
            .map_err(|e| internal(e.into()))?;
        if lines.is_empty() {
            return Ok(());
        }
        // Debit/credit swapped.
        let swapped: Vec<PostingLine> = lines
            .iter()
            .map(|(_, l)| PostingLine {
                account_id: l.account_id,
                debit: l.credit,
                credit: l.debit,
                party_type: l.party_type.clone(),
                party_id: l.party_id,
                cost_center_id: l.cost_center_id,
                project_id: l.project_id,
                department_id: l.department_id,
                description: l.description.clone(),
            })
            .collect();
        let today = Utc::now().date_naive();
        let write = PostingWrite {
            company_id,
            branch_id: meta.branch_id,
            source_type: "reconciliation".to_string(),
            // Links the reversal to the journal it undoes; together with the key below,
            // a retried unlink is a no-op reuse.
            source_id: meta.journal_id,
            source_reference: Some(format!("unlink:{journal_id}")),
            posting_date: today,
            fiscal_period_id: None,
            fiscal_year: today.year(),
            fiscal_month: today.month() as i32,
            currency: meta.currency,
            posting_type: "reversal".to_string(),
            reverses_post_id: meta.reverses_post_id,
            reverses_journal_id: Some(meta.journal_id),
            description: Some("reconciliation unlink reversal".into()),
            idempotency_key: Some(format!("unlink:{journal_id}")),
            posted_by: actor,
            now: Utc::now(),
            lines: swapped,
        };
        self.posting
            .commit_posting_on(conn, write)
            .await
            .map_err(|e| internal(e.into()))?;
        Ok(())
    }

    // =========================================================================
    // Reads
    // =========================================================================

    /// The matching-group read for one line.
    pub async fn matching_group_on(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        line_id: Uuid,
    ) -> Result<MatchingGroup, ReconcileError> {
        self.repo
            .matching_group(conn, company_id, line_id)
            .await
            .map_err(internal)
    }

    /// Open AR/AP lines for one party on one account (the aging read).
    pub async fn residuals_for_party_on(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        account_id: Uuid,
        party_type: &str,
        party_id: Uuid,
    ) -> Result<Vec<PartyResidual>, ReconcileError> {
        self.repo
            .residuals_for_party(conn, company_id, account_id, party_type, party_id)
            .await
            .map_err(internal)
    }

    // =========================================================================
    // Pool-verb wrappers (the HTTP surface; hosts with their own transaction call the
    // `_on` verbs directly)
    // =========================================================================

    /// Pool wrapper over [`Self::reconcile_pair_on`].
    pub async fn reconcile_pair(&self, req: &PairRequest) -> Result<EdgeOutcome, ReconcileError> {
        let mut tx = self.pool.begin().await.map_err(|e| internal(e.into()))?;
        company_scope::bind_company_on(&mut tx, req.company_id)
            .await
            .map_err(|e| internal(e.into()))?;
        let out = self.reconcile_pair_on(&mut tx, req).await?;
        tx.commit().await.map_err(|e| internal(e.into()))?;
        Ok(out)
    }

    /// Pool wrapper over [`Self::unreconcile_on`].
    pub async fn unreconcile(
        &self,
        company_id: Uuid,
        partial_id: Uuid,
        actor: Option<Uuid>,
    ) -> Result<(), ReconcileError> {
        let mut tx = self.pool.begin().await.map_err(|e| internal(e.into()))?;
        company_scope::bind_company_on(&mut tx, company_id)
            .await
            .map_err(|e| internal(e.into()))?;
        self.unreconcile_on(&mut tx, company_id, partial_id, actor)
            .await?;
        tx.commit().await.map_err(|e| internal(e.into()))?;
        Ok(())
    }

    /// Pool wrapper over [`Self::matching_group_on`].
    ///
    /// The bind runs inside a transaction on purpose: `bind_company_on` uses
    /// `set_config(..., is_local)`, whose setting only lives for the current
    /// transaction. On a bare pooled connection it evaporates before the next
    /// statement, the ADR-0014 fence then sees no company, and the read
    /// silently returns an empty group — invisible on owner/superuser DSNs
    /// (they bypass RLS), fatal on any fenced app-role deployment.
    pub async fn matching_group(
        &self,
        company_id: Uuid,
        line_id: Uuid,
    ) -> Result<MatchingGroup, ReconcileError> {
        let mut tx = self.pool.begin().await.map_err(|e| internal(e.into()))?;
        company_scope::bind_company_on(&mut tx, company_id)
            .await
            .map_err(|e| internal(e.into()))?;
        let group = self.matching_group_on(&mut tx, company_id, line_id).await?;
        tx.commit().await.map_err(|e| internal(e.into()))?;
        Ok(group)
    }

    /// Pool wrapper over [`Self::residuals_for_party_on`] — same
    /// transaction-scoped bind discipline as [`Self::matching_group`].
    pub async fn residuals_for_party(
        &self,
        company_id: Uuid,
        account_id: Uuid,
        party_type: &str,
        party_id: Uuid,
    ) -> Result<Vec<PartyResidual>, ReconcileError> {
        let mut tx = self.pool.begin().await.map_err(|e| internal(e.into()))?;
        company_scope::bind_company_on(&mut tx, company_id)
            .await
            .map_err(|e| internal(e.into()))?;
        let rows = self
            .residuals_for_party_on(&mut tx, company_id, account_id, party_type, party_id)
            .await?;
        tx.commit().await.map_err(|e| internal(e.into()))?;
        Ok(rows)
    }
}
