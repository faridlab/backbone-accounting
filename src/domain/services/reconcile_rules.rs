//! Pure reconciliation-graph validation rules (no I/O).
//!
//! The graph-write guards G1–G8 (see `docs/adr/` reconciliation-graph note) against
//! already-loaded line snapshots. Pure like `posting_rules`: no `sqlx`, testable in
//! isolation. Only the structural CHECKs (amount > 0, distinct sides) also ride SQL;
//! everything else lives here and in the write service.

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::domain::reconcile_graph::{
    AccountReconcileFlags, ReconcileError, ReconcileLineSnapshot,
};

/// Is this account subtype a party (settlement-dimension) account?
fn is_party_subtype(subtype: &str) -> bool {
    subtype == "accounts_receivable" || subtype == "accounts_payable"
}

/// Validate a candidate edge and return the CLAMPED amount to apply.
///
/// - G1: both lines belong to the requesting company
/// - G2: both lines sit on the SAME account
/// - G2b: both lines share the same document currency (cross-currency edges are the
///   deferred FX surface — fail closed rather than post a wrong-rate match)
/// - G3: the account is reconcilable
/// - G4: opposite directions — the debit side carries a debit, the credit side a credit
/// - G5: both lines are posted under posted journals
/// - G6: settlement-dimension-bound — on a party account both lines carry the SAME party;
///       non-party accounts are unconstrained
/// - G7: CLAMP — `applied = min(amount, residual_debit, residual_credit)`; a zero clamp is
///       a NO-OP success (the on-account remainder stays unreconciled), never an error
///
/// G8 (period-open for the exchange-move date) is checked by the write service only when
/// an exchange difference actually arises.
pub fn validate_pair(
    company_id: Uuid,
    debit: &ReconcileLineSnapshot,
    credit: &ReconcileLineSnapshot,
    flags: &AccountReconcileFlags,
    amount: Decimal,
    residual_debit: Decimal,
    residual_credit: Decimal,
) -> Result<Decimal, ReconcileError> {
    // G1 — same company. (Locators resolve company-scoped, so a cross-company locator is a
    // 404 before this runs; the guard stays as defense in depth.)
    if debit.company_id != company_id || credit.company_id != company_id {
        return Err(ReconcileError::SameCompanyRequired);
    }
    // G2 — one reconcilable control account, both sides.
    if debit.account_id != credit.account_id {
        return Err(ReconcileError::SameAccountRequired);
    }
    // G3 — the control account must be reconcilable.
    if !flags.is_reconcilable {
        return Err(ReconcileError::AccountNotReconcilable);
    }
    // G4 — opposite directions.
    if debit.debit_amount <= Decimal::ZERO || credit.credit_amount <= Decimal::ZERO {
        return Err(ReconcileError::DirectionMismatch);
    }
    // G5 — both posted under posted journals.
    if !debit.is_posted
        || debit.journal_status != "posted"
        || !credit.is_posted
        || credit.journal_status != "posted"
    {
        return Err(ReconcileError::LineNotPosted);
    }
    // G2b — same document currency (the deferred FX surface is fail-closed).
    if debit.currency != credit.currency {
        return Err(ReconcileError::CurrencyMismatch(
            debit.currency.clone(),
            credit.currency.clone(),
        ));
    }
    // G6 — settlement-dimension-bound.
    if is_party_subtype(&flags.subtype) {
        let same = debit.party_id.is_some()
            && debit.party_id == credit.party_id
            && debit.party_type == credit.party_type;
        if !same {
            return Err(ReconcileError::PartyMismatch);
        }
    }
    // G7 — clamp to the smaller residual. A zero clamp is a no-op success.
    let applied = amount.min(residual_debit).min(residual_credit);
    if applied < Decimal::ZERO {
        return Err(ReconcileError::Conflict("negative clamp".into()));
    }
    Ok(applied)
}

/// A line is FULLY unapplied when its residual equals its face amount.
pub fn fully_unapplied(line: &ReconcileLineSnapshot, residual: Decimal) -> bool {
    residual == line.base_amount
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::reconcile_graph::ReconcileLineSnapshot;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    fn line(
        id: Uuid,
        account: Uuid,
        debit: &str,
        credit: &str,
        party: Option<Uuid>,
    ) -> ReconcileLineSnapshot {
        ReconcileLineSnapshot {
            id,
            journal_id: Uuid::new_v4(),
            company_id: id,
            account_id: account,
            account_subtype: "accounts_receivable".into(),
            party_type: party.map(|_| "customer".to_string()),
            party_id: party,
            debit_amount: dec(debit),
            credit_amount: dec(credit),
            currency: "IDR".into(),
            exchange_rate: dec("1"),
            transaction_date: NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
            is_posted: true,
            journal_status: "posted".into(),
            journal_is_reversing: false,
            source_type: Some("order".into()),
            source_id: Some(Uuid::new_v4()),
            is_reconciled: false,
            full_reconcile_id: None,
            base_amount: dec(debit) + dec(credit),
        }
    }

    fn flags(reconcilable: bool, subtype: &str) -> AccountReconcileFlags {
        AccountReconcileFlags {
            is_reconcilable: reconcilable,
            subtype: subtype.into(),
        }
    }

    fn pair() -> (Uuid, ReconcileLineSnapshot, ReconcileLineSnapshot) {
        let company = Uuid::new_v4();
        let acct = Uuid::new_v4();
        let party = Some(Uuid::new_v4());
        let mut d = line(Uuid::new_v4(), acct, "100", "0", party);
        d.company_id = company;
        let mut c = line(Uuid::new_v4(), acct, "0", "100", party);
        c.company_id = company;
        (company, d, c)
    }

    #[test]
    fn clamps_to_the_smaller_residual() {
        let (company, d, c) = pair();
        let applied = validate_pair(
            company,
            &d,
            &c,
            &flags(true, "accounts_receivable"),
            dec("60"),
            dec("100"),
            dec("40"),
        )
        .unwrap();
        assert_eq!(applied, dec("40"));
    }

    #[test]
    fn zero_clamp_is_a_no_op_success() {
        let (company, d, c) = pair();
        let applied = validate_pair(
            company,
            &d,
            &c,
            &flags(true, "accounts_receivable"),
            dec("60"),
            dec("100"),
            Decimal::ZERO,
        )
        .unwrap();
        assert_eq!(applied, Decimal::ZERO);
    }

    #[test]
    fn rejects_split_accounts() {
        let (company, mut d, c) = pair();
        d.account_id = Uuid::new_v4();
        let err = validate_pair(
            company,
            &d,
            &c,
            &flags(true, "cash"),
            dec("10"),
            dec("10"),
            dec("10"),
        )
        .unwrap_err();
        assert_eq!(err.code(), "same_account_required");
    }

    #[test]
    fn rejects_non_reconcilable_account() {
        let (company, d, c) = pair();
        let err = validate_pair(
            company,
            &d,
            &c,
            &flags(false, "cash"),
            dec("10"),
            dec("10"),
            dec("10"),
        )
        .unwrap_err();
        assert_eq!(err.code(), "account_not_reconcilable");
    }

    #[test]
    fn rejects_same_direction_pair() {
        let (company, d, mut c) = pair();
        c.debit_amount = dec("100");
        c.credit_amount = dec("0");
        let err = validate_pair(
            company,
            &d,
            &c,
            &flags(true, "cash"),
            dec("10"),
            dec("10"),
            dec("10"),
        )
        .unwrap_err();
        assert_eq!(err.code(), "direction_mismatch");
    }

    #[test]
    fn rejects_party_mismatch_on_party_accounts_only() {
        let (company, d, mut c) = pair();
        c.party_id = Some(Uuid::new_v4());
        let err = validate_pair(
            company,
            &d,
            &c,
            &flags(true, "accounts_receivable"),
            dec("10"),
            dec("10"),
            dec("10"),
        )
        .unwrap_err();
        assert_eq!(err.code(), "party_mismatch");
        // A non-party account doesn't care about parties.
        assert!(validate_pair(
            company,
            &d,
            &c,
            &flags(true, "cash"),
            dec("10"),
            dec("10"),
            dec("10")
        )
        .is_ok());
    }

    #[test]
    fn rejects_unposted_or_draft_lines() {
        let (company, mut d, c) = pair();
        d.journal_status = "draft".into();
        let err = validate_pair(
            company,
            &d,
            &c,
            &flags(true, "cash"),
            dec("10"),
            dec("10"),
            dec("10"),
        )
        .unwrap_err();
        assert_eq!(err.code(), "line_not_posted");
    }

    #[test]
    fn rejects_cross_currency_pairs() {
        let (company, mut d, mut c) = pair();
        d.currency = "USD".into();
        c.currency = "IDR".into();
        let err = validate_pair(
            company,
            &d,
            &c,
            &flags(true, "cash"),
            dec("10"),
            dec("10"),
            dec("10"),
        )
        .unwrap_err();
        assert_eq!(err.code(), "currency_mismatch");
    }
}
