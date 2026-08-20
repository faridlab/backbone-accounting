//! Chart install engine — turns a registered [`ChartDataset`] into real `accounts`
//! rows for one company, in one transaction.
//!
//! Posture (Odoo 19's chart model): datasets are DATA; there are no template tables.
//! Installing writes ordinary, manager-editable account rows stamped with chart
//! provenance columns (`chart_code`/`chart_version`). Re-installing the same chart is
//! the update path: engine-owned fields (identity, classification, tree shape) are
//! rewritten, user-owned fields (name, status, balances, bank/tax/budget settings)
//! keep whatever the company last set. Manager renames survive; manager re-parents
//! are reverted (reporting walks `path` — half-updated paths are worse than reverted
//! ones); soft-deleted chart rows are resurrected.
//!
//! Refusals, all pre-write:
//! - `chart_has_postings` — the company already has journal lines. Installing over
//!   live books is refused outright rather than partially switched.
//! - `chart_account_number_conflict` — a non-deleted account already holds a number
//!   or code the dataset uses, and it is not the engine's own row (deterministic id +
//!   matching `chart_code`). Manual accounts and other charts' accounts are never
//!   absorbed; the error names the colliding numbers.
//!
//! Every row id is deterministic: `uuid5(NAMESPACE_URL, "account:{company}:{chart}:{code}")`.
//! That makes re-install an idempotent upsert by construction and gives tax
//! orchestration (which runs after install, keyed on account codes) a stable map.

use crate::domain::chart_dataset::{ChartDataset, DatasetError, validate_dataset};
use crate::domain::repositories::chart_install_repository::{
    ChartAccountRow, ChartInstallRepository, UpsertOutcome,
};
use backbone_orm::company_scope;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

/// Summary of one registered chart, for the listing endpoint.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChartInfo {
    pub code: String,
    pub version: String,
    pub name: String,
    pub accounts: usize,
}

/// What an install did.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallReport {
    pub chart_code: String,
    pub chart_version: String,
    pub company_id: Uuid,
    /// Rows freshly inserted.
    pub accounts_installed: usize,
    /// Own rows updated in place (re-install / version bump).
    pub accounts_updated: usize,
    /// Own rows that had been soft-deleted and were restored.
    pub accounts_resurrected: usize,
    /// `account_code -> account id` — the stable handle for follow-on wiring
    /// (tax templates and their repartition accounts reference these).
    pub account_ids: HashMap<String, Uuid>,
}

/// Install refusals and failures. Displays carry the operative detail (chart codes,
/// colliding numbers, the underlying database error) — callers surface them verbatim.
#[derive(Debug, thiserror::Error)]
pub enum ChartInstallError {
    #[error("chart_has_postings: company {1} already has journal entries; refusing to install chart '{0}' onto posted books")]
    ChartHasPostings(String, Uuid),
    #[error("chart_account_number_conflict: existing accounts hold numbers/codes the chart '{0}' uses but are not rows this chart's current codes install onto (manual rows, or this chart's rows under earlier numbering) — (number, code): {1:?}")]
    AccountNumberConflict(String, Vec<(String, String)>),
    #[error("unknown chart '{0}' (registered: {1:?})")]
    UnknownChart(String, Vec<String>),
    #[error("invalid dataset '{0}': {1}")]
    InvalidDataset(String, #[source] DatasetError),
    #[error("chart install database error: {0}")]
    Db(#[from] anyhow::Error),
    #[error("chart install transaction error: {0}")]
    Tx(#[from] sqlx::Error),
}

/// The engine. Holds the pool, the persistence port, and the registry of datasets
/// registered by the composing service.
pub struct ChartInstallService {
    repo: Arc<dyn ChartInstallRepository>,
    pool: PgPool,
    datasets: Vec<Arc<ChartDataset>>,
}

impl ChartInstallService {
    pub fn new(
        repo: Arc<dyn ChartInstallRepository>,
        pool: PgPool,
        datasets: Vec<Arc<ChartDataset>>,
    ) -> Self {
        Self {
            repo,
            pool,
            datasets,
        }
    }

    /// Registered charts, in registration order.
    pub fn list_charts(&self) -> Vec<ChartInfo> {
        self.datasets
            .iter()
            .map(|ds| ChartInfo {
                code: ds.code.clone(),
                version: ds.version.clone(),
                name: ds.name.clone(),
                accounts: ds.accounts.len(),
            })
            .collect()
    }

    /// Deterministic id for one dataset row — stable across reinstalls and versions.
    fn deterministic_id(company_id: Uuid, chart_code: &str, account_code: &str) -> Uuid {
        Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            format!("account:{company_id}:{chart_code}:{account_code}").as_bytes(),
        )
    }

    /// Resolve the tree facts the dataset does not carry: level, path, header/detail.
    /// `is_header` is purely "has children in this dataset".
    fn derive_rows(&self, company_id: Uuid, ds: &ChartDataset) -> Vec<ChartAccountRow> {
        let parents: HashSet<&str> = ds
            .accounts
            .iter()
            .filter_map(|a| a.parent_code.as_deref())
            .collect();

        // code -> (level, path) of already-derived ancestors (parents-first order
        // guarantees the parent is present when the child is reached).
        let mut derived: HashMap<&str, (i32, String)> = HashMap::new();
        let mut rows = Vec::with_capacity(ds.accounts.len());

        for def in &ds.accounts {
            let (level, path, parent_id) = match def.parent_code.as_deref() {
                None => (0, def.number.clone(), None),
                Some(p) => {
                    let (plvl, ppath) = derived.get(p).expect("validated dataset: parent known");
                    (
                        plvl + 1,
                        format!("{ppath}/{}", def.number),
                        Some(Self::deterministic_id(company_id, &ds.code, p)),
                    )
                }
            };
            let is_header = parents.contains(def.code.as_str());
            derived.insert(def.code.as_str(), (level, path.clone()));
            rows.push(ChartAccountRow {
                id: Self::deterministic_id(company_id, &ds.code, &def.code),
                company_id,
                def: def.clone(),
                parent_id,
                level,
                path,
                is_header,
                is_detail: !is_header,
                chart_code: ds.code.clone(),
                chart_version: ds.version.clone(),
            });
        }
        rows
    }

    /// Install `chart_code` onto `company_id`. One transaction; the tenant is bound on
    /// the transaction first so the strict fence accepts the writes as the app role.
    pub async fn install(
        &self,
        company_id: Uuid,
        chart_code: &str,
    ) -> Result<InstallReport, ChartInstallError> {
        let ds = self
            .datasets
            .iter()
            .find(|d| d.code == chart_code)
            .ok_or_else(|| {
                ChartInstallError::UnknownChart(
                    chart_code.to_string(),
                    self.datasets.iter().map(|d| d.code.clone()).collect(),
                )
            })?
            .clone();

        validate_dataset(&ds).map_err(|e| ChartInstallError::InvalidDataset(ds.code.clone(), e))?;

        let rows = self.derive_rows(company_id, &ds);

        let mut tx = self.pool.begin().await?;
        // Strict fence: writes need the company GUC for the RLS WITH CHECK, reads to
        // see the company's rows at all.
        company_scope::bind_company_on(&mut tx, company_id)
            .await
            .map_err(anyhow::Error::from)?;

        if self.repo.company_has_postings(&mut tx, company_id).await? {
            return Err(ChartInstallError::ChartHasPostings(ds.code.clone(), company_id));
        }

        // Overlap gate — an existing non-deleted account colliding on number or code
        // is fine ONLY if it is exactly our own row (deterministic id + our chart_code,
        // which also covers version bumps). Everything else (manual rows, another
        // chart's rows) is a named conflict, never absorbed.
        let overlaps = self.repo.overlapping_accounts(&mut tx, company_id, &ds).await?;
        let conflicts: Vec<(String, String)> = overlaps
            .into_iter()
            .filter(|o| {
                let ours = rows.iter().any(|r| {
                    r.def.number == o.account_number
                        && r.id == o.id
                        && o.chart_code.as_deref() == Some(ds.code.as_str())
                }) || rows.iter().any(|r| {
                    r.def.code == o.account_code
                        && r.id == o.id
                        && o.chart_code.as_deref() == Some(ds.code.as_str())
                });
                !ours
            })
            .map(|o| (o.account_number, o.account_code))
            .collect();
        if !conflicts.is_empty() {
            return Err(ChartInstallError::AccountNumberConflict(
                ds.code.clone(),
                conflicts,
            ));
        }

        let mut installed = 0usize;
        let mut updated = 0usize;
        let mut resurrected = 0usize;
        let mut account_ids: HashMap<String, Uuid> = HashMap::with_capacity(rows.len());

        // Dataset order is parents-first, so the parent_id FK is always satisfied.
        for row in &rows {
            match self.repo.upsert_account(&mut tx, row).await? {
                UpsertOutcome::Inserted => installed += 1,
                UpsertOutcome::Updated => updated += 1,
                UpsertOutcome::Resurrected => resurrected += 1,
            }
            account_ids.insert(row.def.code.clone(), row.id);
        }

        tx.commit().await?;

        Ok(InstallReport {
            chart_code: ds.code.clone(),
            chart_version: ds.version.clone(),
            company_id,
            accounts_installed: installed,
            accounts_updated: updated,
            accounts_resurrected: resurrected,
            account_ids,
        })
    }
}
