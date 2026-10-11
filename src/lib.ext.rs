//! The module's extension (hand-written; ADR-0031): what the module adds beside its
//! generated services. The generated module and builder carry `ModuleExt` and
//! `ModuleBuilderExt` and dereference to them, so their fields read as `module.field`.

#[allow(unused_imports)]
use super::*;

/// State the module adds; read through the generated module's `Deref`.
pub struct ModuleExt {
    pub(crate) db_pool: PgPool,
    pub(crate) reconcile_write_service:
        Arc<crate::application::service::reconcile_write_service::ReconcileWriteService>,
    /// The check printing surface: per-bank-journal check-number sequences and
    /// the printed-check registry.
    pub(crate) check_printing_service:
        Arc<crate::application::service::check_printing_service::CheckPrintingService>,
    /// The EMV(QRCPS)/QRIS display surface: merchant QR configuration and the
    /// invoice payload render.
    pub(crate) emv_qr_service:
        Arc<crate::application::service::emv_qr_service::EmvQrService>,
    /// The tax-tag legal-change repair verb and its audit ledger read.
    pub(crate) tax_tag_repair_service:
        Arc<crate::application::service::tax_tag_repair_service::TaxTagRepairService>,
    /// Host-implemented cash-basis tax deferral lookup, kept so post-build
    /// service variants (e.g. the exchange-account one) keep the flip wired.
    pub(crate) deferred_tax: Option<Arc<dyn crate::domain::repositories::DeferredTaxLookup>>,
    /// Host-implemented budget control, kept so every posting path the host
    /// builds from this module (the HTTP posting verb, the journal workflow,
    /// the period close) can arm the consult from one registration.
    pub(crate) budget_control: Option<Arc<dyn crate::domain::repositories::BudgetControlPort>>,
    /// Chart install engine over the datasets registered via `with_chart_dataset`.
    pub(crate) chart_install_service:
        Arc<crate::application::service::chart_install_service::ChartInstallService>,
    /// On-the-fly financial reports (TB tree, BS, P&L, GL, partner ledger, aged AR/AP).
    pub(crate) reporting_service:
        Arc<crate::application::service::reporting_service::ReportingService>,
}

/// State the builder adds.
pub struct ModuleBuilderExt {
    deferred_tax: Option<Arc<dyn crate::domain::repositories::DeferredTaxLookup>>,
    budget_control: Option<Arc<dyn crate::domain::repositories::BudgetControlPort>>,
    chart_datasets: Vec<Arc<crate::domain::chart_dataset::ChartDataset>>,
}

impl Default for ModuleBuilderExt {
    fn default() -> Self {
        Self {
            deferred_tax: None,
            budget_control: None,
            chart_datasets: Vec::new(),
        }
    }
}

impl ModuleBuilderExt {
    /// Build the extension's state from what the generated build made.
    #[allow(unused_variables, clippy::redundant_clone)]
    pub(crate) fn build(self, parts: &ModuleParts<'_>) -> anyhow::Result<ModuleExt> {
        let db_pool = parts.db_pool.clone();
        // Reconciliation write service: graph port + posting port (reversals of
        // generated moves) over the shared pool; no FX account by default (fail-closed).
        let reconcile_graph_repository = std::sync::Arc::new(
            crate::infrastructure::persistence::reconcile_graph_repository::SqlxReconcileGraphRepository::new(),
        );
        let reconcile_posting_repository = std::sync::Arc::new(
            crate::infrastructure::persistence::posting_repository::SqlxPostingRepository::new(
                db_pool.clone(),
            ),
        );
        let reconcile_write_service = std::sync::Arc::new(
            crate::application::service::reconcile_write_service::ReconcileWriteService::new(
                reconcile_graph_repository,
                reconcile_posting_repository,
                db_pool.clone(),
                None,
            )
            .with_deferred_tax_if_set(self.deferred_tax.clone()),
        );
        // Chart install engine: datasets registered by the host drive idempotent
        // chart installs onto fresh companies (provenance columns identify own rows).
        let chart_install_service = std::sync::Arc::new(
            crate::application::service::chart_install_service::ChartInstallService::new(
                std::sync::Arc::new(
                    crate::infrastructure::persistence::chart_install_repository::SqlxChartInstallRepository::new(),
                ),
                db_pool.clone(),
                self.chart_datasets,
            ),
        );
        // Financial reports: computed on the fly over the scoped ledger reads.
        let reporting_service = std::sync::Arc::new(
            crate::application::service::reporting_service::ReportingService::new(
                std::sync::Arc::new(
                    crate::infrastructure::persistence::reporting_repository::SqlxReportingRepository::new(
                        db_pool.clone(),
                    ),
                ),
            ),
        );
        // Check printing: per-bank-journal number sequences + the printed-check
        // registry, all writes through the guarded verb set.
        let check_printing_service = std::sync::Arc::new(
            crate::application::service::check_printing_service::CheckPrintingService::new(
                db_pool.clone(),
            ),
        );
        // EMV(QRCPS)/QRIS display: merchant QR config + invoice payload render.
        let emv_qr_service = std::sync::Arc::new(
            crate::application::service::emv_qr_service::EmvQrService::new(db_pool.clone()),
        );
        // Tax-tag legal-change repair verb + its audit ledger read.
        let tax_tag_repair_service = std::sync::Arc::new(
            crate::application::service::tax_tag_repair_service::TaxTagRepairService::new(
                db_pool.clone(),
            ),
        );
        Ok(ModuleExt {
            db_pool,
            reconcile_write_service,
            deferred_tax: self.deferred_tax,
            budget_control: self.budget_control,
            chart_install_service,
            reporting_service,
            check_printing_service,
            emv_qr_service,
            tax_tag_repair_service,
        })
    }
}

impl crate::ThisModule {
    /// The registered budget-control port, if any — the single registration a
    /// host threads into every `PostingService` it builds from this module
    /// (`.with_budget_control_if_set(m.budget_control())`), so the HTTP
    /// posting verb, the journal workflow, and the period close all consult
    /// the same control.
    pub fn budget_control(
        &self,
    ) -> Option<Arc<dyn crate::domain::repositories::BudgetControlPort>> {
        self.budget_control.clone()
    }
    /// The on-the-fly financial reports (Trial Balance tree, Balance Sheet, Income
    /// Statement, General Ledger, Partner Ledger, Aged AR/AP). Read-only; scoped reads.
    pub fn reporting_service(
        &self,
    ) -> std::sync::Arc<crate::application::service::reporting_service::ReportingService> {
        self.reporting_service.clone()
    }
    /// The reconciliation write service — the ONLY sanctioned writer of the
    /// reconciliation graph (guards + CLAMP + side-effecting unlink). The graph tables
    /// carry no CRUD surface anywhere; `guarded_routes` mounts its verb routes.
    pub fn reconcile_write_service(
        &self,
    ) -> std::sync::Arc<crate::application::service::reconcile_write_service::ReconcileWriteService>
    {
        self.reconcile_write_service.clone()
    }
    /// Build a reconcile write service with the FX (exchange-difference) account
    /// configured. Without one, any rate divergence that demands an exchange move fails
    /// closed (`exchange_account_unconfigured`) — normal single-currency posts never
    /// generate one, so the default service is complete for them.
    pub fn reconcile_write_service_with_exchange_account(
        &self,
        exchange_account_id: uuid::Uuid,
    ) -> std::sync::Arc<crate::application::service::reconcile_write_service::ReconcileWriteService>
    {
        std::sync::Arc::new(
            crate::application::service::reconcile_write_service::ReconcileWriteService::new(
                std::sync::Arc::new(
                    crate::infrastructure::persistence::reconcile_graph_repository::SqlxReconcileGraphRepository::new(),
                ),
                std::sync::Arc::new(
                    crate::infrastructure::persistence::posting_repository::SqlxPostingRepository::new(
                        self.db_pool.clone(),
                    ),
                ),
                self.db_pool.clone(),
                Some(exchange_account_id),
            )
            .with_deferred_tax_if_set(self.deferred_tax.clone()),
        )
    }
    /// The chart install engine — validates, gates, and writes a registered chart
    /// dataset into the accounts, idempotently. The install VERB itself is
    /// host-mounted (it orchestrates beyond this module); this handle is the engine
    /// the host verb drives.
    pub fn chart_install_service(
        &self,
    ) -> std::sync::Arc<crate::application::service::chart_install_service::ChartInstallService>
    {
        self.chart_install_service.clone()
    }
    /// The check printing surface — sequence registration, atomic number
    /// allocation, the printed-check registry, and voids. `guarded_routes`
    /// mounts its verb routes; there is no CRUD path to these tables.
    pub fn check_printing_service(
        &self,
    ) -> std::sync::Arc<crate::application::service::check_printing_service::CheckPrintingService>
    {
        self.check_printing_service.clone()
    }
    /// The EMV(QRCPS)/QRIS display surface — merchant QR configuration and the
    /// invoice payload render (payloads are built on demand, never stored).
    pub fn emv_qr_service(
        &self,
    ) -> std::sync::Arc<crate::application::service::emv_qr_service::EmvQrService> {
        self.emv_qr_service.clone()
    }
    /// The tax-tag legal-change repair verb (reason mandatory, locked periods
    /// refused, closed periods overridden explicitly, every run audit-stamped)
    /// plus its audit-ledger read.
    pub fn tax_tag_repair_service(
        &self,
    ) -> std::sync::Arc<crate::application::service::tax_tag_repair_service::TaxTagRepairService>
    {
        self.tax_tag_repair_service.clone()
    }
}

impl crate::ThisModuleBuilder {
    /// Register a chart-of-accounts dataset installable through the chart install
    /// engine. Datasets are data (commonly shipped by an l10n module); registering
    /// one makes it listable via `GET /accounting/charts` and installable by the
    /// host-mounted install verb.
    pub fn with_chart_dataset(
        mut self,
        dataset: Arc<crate::domain::chart_dataset::ChartDataset>,
    ) -> Self {
        self.chart_datasets.push(dataset);
        self
    }
    /// Wire the host's cash-basis tax deferral lookup: partial reconciliations
    /// landing on receivable/payable lines flip deferred (on_payment) tax
    /// pro-rata from the transition account onto the real tax account. Hosts
    /// without a tax module leave it unset — no flips, everything else works.
    pub fn with_deferred_tax(
        mut self,
        port: Arc<dyn crate::domain::repositories::DeferredTaxLookup>,
    ) -> Self {
        self.deferred_tax = Some(port);
        self
    }
    /// Wire the host's budget control: postings through the GL-posting
    /// chokepoint are checked against confirmed budget positions (account x
    /// cost center x fiscal period) after the pure double-entry rules pass.
    /// Warn-enforcement breaches log and proceed; block-enforcement breaches
    /// refuse the posting. Hosts without a budget module leave it unset — no
    /// check, everything else works (fail-open).
    pub fn with_budget_control(
        mut self,
        port: Arc<dyn crate::domain::repositories::BudgetControlPort>,
    ) -> Self {
        self.budget_control = Some(port);
        self
    }
}
