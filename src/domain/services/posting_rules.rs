//! Pure GL-posting validation rules (no I/O).
//!
//! These encode the double-entry invariants R1–R6 from the BRD against already-loaded account
//! snapshots. Keeping them pure (no `sqlx`, no `&PgPool`) means the domain rules are testable
//! in isolation and the persistence layer can't leak into them.

use std::collections::HashMap;

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::domain::gl_posting::{PostingError, PostingLine};
use crate::domain::repositories::posting_repository::PostableAccount;

/// Validate a set of posting lines against loaded accounts + the period-open flag.
///
/// - R2: ≥ 2 lines
/// - R1: Σdebit = Σcredit
/// - R6: every line's account is `is_detail`, not `is_header`, `status = active`
/// - R3: party required iff account_subtype ∈ {accounts_receivable, accounts_payable}
/// - R4: not into a closed/locked period
pub fn validate(
    lines: &[PostingLine],
    accounts: &HashMap<Uuid, PostableAccount>,
    period_closed: bool,
) -> Result<(), PostingError> {
    if lines.len() < 2 {
        return Err(PostingError::TooFewLines);
    }
    // R1a — sign constraint: each line must be a clean debit-OR-credit. Both sides non-negative,
    // exactly one strictly positive. Rejects negative amounts (a balanced {debit:-100,credit:-100}
    // would otherwise pass the balance check and corrupt normal-balance semantics), both-sides
    // postings, and zero lines.
    for line in lines {
        let clean = line.debit >= Decimal::ZERO
            && line.credit >= Decimal::ZERO
            && ((line.debit > Decimal::ZERO) ^ (line.credit > Decimal::ZERO));
        if !clean {
            return Err(PostingError::InvalidLineAmount);
        }
    }
    let total_debit: Decimal = lines.iter().map(|l| l.debit).sum();
    let total_credit: Decimal = lines.iter().map(|l| l.credit).sum();
    if total_debit != total_credit {
        return Err(PostingError::Unbalanced);
    }

    for line in lines {
        let acct = accounts
            .get(&line.account_id)
            .ok_or(PostingError::AccountNotFound(line.account_id))?;
        if acct.is_header || !acct.is_detail || acct.status != "active" {
            return Err(PostingError::NonPostableAccount(acct.number.clone()));
        }
        let is_party_account =
            acct.subtype == "accounts_receivable" || acct.subtype == "accounts_payable";
        let has_party = line.party_type.is_some() && line.party_id.is_some();
        if is_party_account && !has_party {
            return Err(PostingError::PartyRequired(acct.number.clone()));
        }
        if !is_party_account && (line.party_type.is_some() || line.party_id.is_some()) {
            return Err(PostingError::PartyNotAllowed(acct.number.clone()));
        }
    }

    if period_closed {
        return Err(PostingError::PeriodClosed);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::gl_posting::PostingLine;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }
    fn acct(id: Uuid) -> PostableAccount {
        PostableAccount {
            id,
            number: "1000".into(),
            name: "Cash".into(),
            account_type: "asset".into(),
            subtype: "cash".into(),
            normal_balance: "debit".into(),
            is_detail: true,
            is_header: false,
            status: "active".into(),
            current_balance: Decimal::ZERO,
        }
    }
    fn line(id: Uuid, debit: &str, credit: &str) -> PostingLine {
        PostingLine {
            account_id: id,
            debit: dec(debit),
            credit: dec(credit),
            party_type: None,
            party_id: None,
            cost_center_id: None,
            project_id: None,
            department_id: None,
            description: None,
        }
    }

    fn validate(lines: &[PostingLine]) -> Result<(), PostingError> {
        let a = acct(lines[0].account_id);
        let accounts = std::iter::once((a.id, a)).collect();
        super::validate(lines, &accounts, false)
    }

    #[test]
    fn rejects_negative_amounts_even_when_balanced() {
        // {debit:-100, credit:-100} balances at -100 == -100 but must be rejected (R1a).
        let id = Uuid::new_v4();
        let err = validate(&[line(id, "-100", "-100"), line(id, "-100", "-100")]).unwrap_err();
        assert_eq!(err.code(), "invalid_line_amount");
    }

    #[test]
    fn rejects_both_sides_and_zero_lines() {
        let id = Uuid::new_v4();
        assert_eq!(validate(&[line(id, "100", "100"), line(id, "100", "100")]).unwrap_err().code(), "invalid_line_amount");
        assert_eq!(validate(&[line(id, "0", "0"), line(id, "0", "0")]).unwrap_err().code(), "invalid_line_amount");
    }

    #[test]
    fn accepts_clean_balanced_lines() {
        let id = Uuid::new_v4();
        assert!(validate(&[line(id, "100", "0"), line(id, "0", "100")]).is_ok());
    }
}
