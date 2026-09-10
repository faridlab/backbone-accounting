//! EMV(QRCPS)/QRIS display service — configuration storage + the invoice
//! payload render.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). The pure TLV/CRC
//! builder lives in `domain/emv_qr.rs`; this service owns the
//! `accounting.emv_qr_configs` row per unit-wide default (optionally per
//! bank-account slot) and renders payloads from it at display time. Payloads
//! are NEVER stored — a config correction immediately changes every
//! subsequently rendered QR, and nothing stale can be served from a cache of
//! rows.
//!
//! The display legs (amount / currency / invoice reference) arrive as call
//! parameters because invoices live in the billing module and this module
//! deliberately holds no Cargo edge into it: the composing host reads the
//! invoice there and calls this verb with the values. A missing configuration
//! refuses (`qr_config_missing`) — there is no fallback payload.
//!
//! Tenancy (ADR-0029): the module carries no tenancy of its own — the composing
//! service's tenancy decorator owns org scoping. The `company_id` lanes here are
//! the documented legacy twin: input/ack shapes and verb signatures keep them so
//! unstripped callers compile and run unchanged, but no statement keys on a
//! tenant column. Every verb relays the ambient org scope onto its transaction;
//! an undecorated deployment has no ambient scope and skips the relay entirely
//! (unfenced by design).

use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::emv_qr::{
    build_emv_merchant_payload, EmvMerchantProfile, EmvQrError,
};

/// The slot a config is written to: the company-wide default
/// (`bank_account_id = None`) or one bank account's override.
#[derive(Debug, Clone)]
pub struct EmvQrConfigInput {
    /// The legacy tenancy twin (ADR-0029) — kept so unstripped callers compile and
    /// run unchanged; no statement keys on it.
    pub company_id: Uuid,
    pub bank_account_id: Option<Uuid>,
    pub merchant_name: String,
    pub merchant_city: String,
    pub country: String,
    pub mcc: String,
    pub gui: String,
    pub merchant_identifier: String,
    pub currency: String,
    pub initiation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmvQrConfigAck {
    pub config_id: Uuid,
    /// The legacy tenancy twin (ADR-0029) — echoed verbatim for unstripped callers.
    pub company_id: Uuid,
    pub bank_account_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmvPayloadAck {
    /// The full TLV payload string (CRC included) a renderer encodes into the QR.
    pub payload: String,
    /// The currency actually rendered (request override or config default).
    pub currency: String,
    pub initiation_method: String,
}

#[derive(Debug)]
pub enum EmvQrServiceError {
    /// Validation failed in the domain builder; carries the typed cause.
    Build(EmvQrError),
    /// No configuration exists (for the requested slot or the unit-wide default).
    ConfigMissing(Uuid),
    /// Storage failure.
    Internal(String),
}

impl EmvQrServiceError {
    pub fn code(&self) -> &'static str {
        match self {
            EmvQrServiceError::Build(e) => e.code(),
            EmvQrServiceError::ConfigMissing(_) => "qr_config_missing",
            EmvQrServiceError::Internal(_) => "internal_error",
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            EmvQrServiceError::Build(_) => 422,
            EmvQrServiceError::ConfigMissing(_) => 404,
            EmvQrServiceError::Internal(_) => 500,
        }
    }
}

impl std::fmt::Display for EmvQrServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmvQrServiceError::Build(e) => write!(f, "{e}"),
            EmvQrServiceError::ConfigMissing(company_id) => write!(
                f,
                "qr_config_missing: no EMV QR configuration for company {company_id}"
            ),
            EmvQrServiceError::Internal(e) => write!(f, "internal_error: {e}"),
        }
    }
}
impl std::error::Error for EmvQrServiceError {}
impl From<EmvQrError> for EmvQrServiceError {
    fn from(e: EmvQrError) -> Self {
        EmvQrServiceError::Build(e)
    }
}

fn internal(e: impl std::fmt::Display) -> EmvQrServiceError {
    EmvQrServiceError::Internal(e.to_string())
}

#[derive(Clone)]
pub struct EmvQrService {
    pool: PgPool,
}

impl EmvQrService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn profile_of(c: &EmvQrConfigInput) -> EmvMerchantProfile {
        EmvMerchantProfile {
            merchant_name: c.merchant_name.clone(),
            merchant_city: c.merchant_city.clone(),
            country: c.country.clone(),
            mcc: c.mcc.clone(),
            gui: c.gui.clone(),
            merchant_identifier: c.merchant_identifier.clone(),
            initiation: c.initiation.clone(),
        }
    }

    /// Write (or replace) the QR display configuration for a slot.
    ///
    /// The profile is validated through the domain builder BEFORE the write —
    /// an overlong merchant name or unsupported currency refuses here, so a
    /// saved config always renders.
    pub async fn upsert_config(
        &self,
        input: EmvQrConfigInput,
    ) -> Result<EmvQrConfigAck, EmvQrServiceError> {
        // Dry build: exercises every field validation without persisting.
        build_emv_merchant_payload(&Self::profile_of(&input), &input.currency, None, None)?;

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| internal(e))?;
        // Tenancy posture (ADR-0029): relay the AMBIENT request scope onto this
        // transaction when the caller bound one. An undecorated deployment has no
        // ambient scope and skips this entirely (unfenced by design).
        if let Some(scope) = backbone_orm::org_scope::current_org_scope() {
            backbone_orm::org_scope::bind_org_scope_on(&mut tx, &scope)
                .await
                .map_err(|e| internal(e))?;
        }

        // One atomic upsert. The conflict target is the slot's tenant-free unique
        // expression index (COALESCE folds the NULL unit-default slot into the same
        // key space as bank-specific ones; bank_account_id is globally unique, so
        // the org-free form is exactly the per-unit constraint), so an
        // insert-or-update race between two writers converges on one row here in a
        // single statement — a caught unique violation could not be retried inside
        // the same transaction anyway (the failed statement aborts it).
        let config_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO accounting.emv_qr_configs
                 (bank_account_id, merchant_name, merchant_city,
                  country_code, mcc, gui, merchant_identifier, currency,
                  initiation_method, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,NOW())
               ON CONFLICT
                 (COALESCE(bank_account_id, '00000000-0000-0000-0000-000000000000'::uuid))
               DO UPDATE SET
                 merchant_name     = EXCLUDED.merchant_name,
                 merchant_city     = EXCLUDED.merchant_city,
                 country_code      = EXCLUDED.country_code,
                 mcc               = EXCLUDED.mcc,
                 gui               = EXCLUDED.gui,
                 merchant_identifier = EXCLUDED.merchant_identifier,
                 currency          = EXCLUDED.currency,
                 initiation_method = EXCLUDED.initiation_method,
                 updated_at        = NOW()
               RETURNING id"#,
        )
        .bind(input.bank_account_id)
        .bind(&input.merchant_name)
        .bind(&input.merchant_city)
        .bind(input.country.trim().to_ascii_uppercase())
        .bind(&input.mcc)
        .bind(&input.gui)
        .bind(&input.merchant_identifier)
        .bind(&input.currency)
        .bind(&input.initiation)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| internal(e))?;

        tx.commit().await.map_err(|e| internal(e))?;
        Ok(EmvQrConfigAck {
            config_id,
            company_id: input.company_id,
            bank_account_id: input.bank_account_id,
        })
    }

    /// Render the merchant-presented payload for one invoice display.
    ///
    /// Slot resolution: the bank-account-specific config when present, else the
    /// unit-wide default; neither → `qr_config_missing` (fail closed, no
    /// fallback payload). `currency` overrides the configured default when
    /// given; `amount` of `None` renders a static QR without tag 54.
    ///
    /// The read relays the ambient org scope inside its own transaction on
    /// purpose: the relay uses `set_config(..., is_local)`, whose setting only
    /// lives for the current transaction. On a bare pooled connection the
    /// decorator's fence (ADR-0029) sees no scope, the read silently returns no
    /// rows, and every render degrades to `qr_config_missing` — invisible on
    /// owner/superuser DSNs (they bypass RLS), fatal on any fenced app-role
    /// deployment.
    pub async fn invoice_payload(
        &self,
        company_id: Uuid,
        bank_account_id: Option<Uuid>,
        amount: Option<Decimal>,
        currency: Option<String>,
        reference: Option<String>,
    ) -> Result<EmvPayloadAck, EmvQrServiceError> {
        let mut tx = self.pool.begin().await.map_err(|e| internal(e))?;
        // Tenancy posture (ADR-0029): relay the AMBIENT request scope when the
        // caller bound one; an undecorated deployment skips this entirely.
        if let Some(scope) = backbone_orm::org_scope::current_org_scope() {
            backbone_orm::org_scope::bind_org_scope_on(&mut tx, &scope)
                .await
                .map_err(|e| internal(e))?;
        }

        let row = sqlx::query_as::<_, StoredConfig>(
            r#"SELECT merchant_name, merchant_city, country_code, mcc, gui,
                      merchant_identifier, currency, initiation_method
                 FROM accounting.emv_qr_configs
                WHERE (bank_account_id IS NOT DISTINCT FROM $1
                       OR bank_account_id IS NULL)
                ORDER BY (bank_account_id IS NOT DISTINCT FROM $1) DESC
                LIMIT 1"#,
        )
        .bind(bank_account_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| internal(e))?;
        tx.commit().await.map_err(|e| internal(e))?;

        let Some(cfg) = row else {
            return Err(EmvQrServiceError::ConfigMissing(company_id));
        };

        let profile = EmvMerchantProfile {
            merchant_name: cfg.merchant_name,
            merchant_city: cfg.merchant_city,
            country: cfg.country_code,
            mcc: cfg.mcc,
            gui: cfg.gui,
            merchant_identifier: cfg.merchant_identifier,
            initiation: cfg.initiation_method.clone(),
        };
        let currency = currency.unwrap_or(cfg.currency);
        let payload =
            build_emv_merchant_payload(&profile, &currency, amount, reference.as_deref())?;

        Ok(EmvPayloadAck {
            payload,
            currency,
            initiation_method: cfg.initiation_method,
        })
    }
}

#[derive(sqlx::FromRow)]
struct StoredConfig {
    merchant_name: String,
    merchant_city: String,
    country_code: String,
    mcc: String,
    gui: String,
    merchant_identifier: String,
    currency: String,
    initiation_method: String,
}
