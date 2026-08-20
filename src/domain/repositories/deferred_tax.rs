//! Cash-basis (on_payment) tax deferral lookup port.
//!
//! When a partial reconciliation lands on a receivable/payable line whose
//! document carried cash-basis tax, the deferred portion becomes exigible and
//! must flip pro-rata from the tax transition account to the real tax account.
//! Accounting owns the flip machinery but cannot know where the deferrals
//! live: the tax engine records the posture at compute time and the producing
//! module (billing, on its invoice tax lines) persists the marker. The
//! deployed app sees both modules and implements this port against its own
//! schema — a host-side ACL, not a module edge.
//!
//! The port rides the caller's connection so a flip journal commits atomically
//! with the partial that triggered it. `None` (unwired) simply means no flips:
//! accounting keeps working for hosts without a cash-basis tax module.

use async_trait::async_trait;
use rust_decimal::Decimal;
use uuid::Uuid;

/// One deferred tax amount waiting on a payment event.
#[derive(Debug, Clone)]
pub struct DeferredTaxLine {
    /// The ORIGINAL journal line that carried the deferred tax on the
    /// transition account — the pairing target that drives the transition
    /// account's residual to zero as flips land.
    pub source_line_id: Uuid,
    pub transition_account_id: Uuid,
    pub real_account_id: Uuid,
    /// Face amount, always positive; orientation is `is_debit`.
    pub amount: Decimal,
    /// Orientation of the ORIGINAL transition line (the flip's real-account
    /// leg repeats it; the aggregate transition leg mirrors it).
    pub is_debit: bool,
}

#[async_trait]
pub trait DeferredTaxLookup: Send + Sync {
    /// Deferred tax lines tied to one reconciled document line's journal. An
    /// empty result means the document accrued no cash-basis tax — nothing to
    /// flip. Implementations must resolve `source_id` against the producing
    /// module's own tables (unknown producers ⇒ empty, never a guess) so a
    /// shared `source_type` cannot cross-wire documents.
    ///
    /// CONTRACT — the flip's pro-rata base is the SINGLE reconciled line's face:
    /// implementations must key on `journal_id` + `source_id`, return one row
    /// per distinct `source_line_id` (the transition line the flips retire —
    /// the anchored cumulative math reads that line's residual per deferral),
    /// and the producing module must post ONE AR/AP settlement line per
    /// document face. A document settled through split AR/AP legs would
    /// pro-rate the FULL deferral against EACH leg's face and recognize the
    /// tax twice; nothing on the accounting side can detect that shape, so the
    /// producer-side single-line posting is part of this port's contract.
    async fn deferred_lines_on(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        journal_id: Uuid,
        source_type: Option<&str>,
        source_id: Option<Uuid>,
    ) -> anyhow::Result<Vec<DeferredTaxLine>>;
}
