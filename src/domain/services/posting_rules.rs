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
