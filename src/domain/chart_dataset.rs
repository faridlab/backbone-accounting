//! Chart-of-accounts datasets — the data half of the chart install engine.
//!
//! A [`ChartDataset`] is a versioned, parents-first description of a national chart of
//! accounts (e.g. the Indonesian SAK chart): numbers, names, classification, and tree
//! shape. It is DATA, not database state — no template tables exist anywhere. The
//! install engine (`chart_install_service`) turns a dataset into real `accounts` rows
//! for one company; rows carry `chart_code`/`chart_version` provenance so re-installs
//! recognize their own rows and leave manual ones alone.
//!
//! Header/detail flags, level, and path are NOT part of a dataset — they are derived
//! from the tree at install time ("has children ⇒ header"), so a dataset can never
//! drift from the shape it describes.
//!
//! Dataset invariants, enforced by [`validate_dataset`] before any write happens:
//! - numbers and codes are unique and non-empty; a code IS its number with the dots
//!   stripped (identity is hashed from the code while collision matching matches on
//!   the number, so the two must never diverge)
//! - names are non-empty; currencies are 3-letter uppercase ISO codes; sort orders
//!   are unique
//! - every `parent_code` refers to an account appearing EARLIER in the list
//!   (parents-first order makes the tree acyclic by construction)
//! - at least one AR, one AP, one bank, and one cash account exist — the posting and
//!   settlement paths assume those subtypes are present.
//!
//! ## The version contract: account codes are identity, and identity is stable
//!
//! A new dataset version may re-classify, re-name, or re-shape accounts freely,
//! but corrections must keep every account's NUMBER (and therefore its code)
//! stable. The engine's deterministic ids hash the code, so a renumbered account
//! is a NEW identity: the old row is neither updated nor removed — it lingers as
//! a fully live, chart-stamped posting target after the newer version installs
//! (and once the company posts, the postings gate freezes any later cleanup).
//! The same applies to codes dropped from a later version. Until a deprecation
//! sweep exists, removing or renumbering an account is a manual, operator-driven
//! archival step, deliberately outside the engine.

use crate::domain::entity::{AccountSubtype, AccountType, NormalBalance};
use serde::Deserialize;
use std::collections::HashSet;

/// A registered chart of accounts, ready to install into a company.
#[derive(Debug, Clone, Deserialize)]
pub struct ChartDataset {
    /// Stable dataset identifier (e.g. `ID_SAK`). Part of the deterministic account id.
    pub code: String,
    /// Dataset version (e.g. `2026.1`). Corrections ship as a new version.
    pub version: String,
    /// Human-readable chart name.
    pub name: String,
    /// Account definitions in parents-first order (every parent appears before its children).
    pub accounts: Vec<ChartAccountDef>,
}

/// One account definition inside a [`ChartDataset`].
#[derive(Debug, Clone, Deserialize)]
pub struct ChartAccountDef {
    /// Display/lookup number, dots allowed (e.g. `1171.000`).
    pub number: String,
    /// Compact code: the number with dots stripped (e.g. `1171000`). Unique per dataset.
    pub code: String,
    /// Localized name (Indonesian for the SAK chart).
    pub name: String,
    pub account_type: AccountType,
    pub account_subtype: AccountSubtype,
    pub normal_balance: NormalBalance,
    /// Parent's `code`, or `None` for roots. The parent must appear earlier in the dataset.
    pub parent_code: Option<String>,
    /// Requires reconciliation against statements/bank feeds (bank, cash, AR/AP).
    pub is_reconcilable: bool,
    /// ISO currency code (e.g. `IDR`).
    pub currency: String,
    /// Display order within the chart.
    pub sort_order: i32,
}

/// What a dataset must minimally contain for the posting/settlement paths to work.
pub const REQUIRED_SUBTYPES: [AccountSubtype; 4] = [
    AccountSubtype::AccountsReceivable,
    AccountSubtype::AccountsPayable,
    AccountSubtype::Bank,
    AccountSubtype::Cash,
];

/// Why a dataset failed validation. Carries the offending identifiers so the caller
/// can surface them (error Displays must not swallow the detail).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DatasetError {
    #[error("account '{0}' has an empty number or code")]
    EmptyIdentifier(String),
    #[error("account '{0}' has an empty name")]
    EmptyName(String),
    #[error("duplicate account number '{0}'")]
    DuplicateNumber(String),
    #[error("duplicate account code '{0}'")]
    DuplicateCode(String),
    #[error("duplicate sort_order {0} — dataset order must be unambiguous")]
    DuplicateSortOrder(i32),
    #[error("account '{0}' currency '{1}' must be a 3-letter uppercase ISO code")]
    InvalidCurrency(String, String),
    #[error("account '{0}' code '{1}' must be its number '{2}' with dots stripped: identity hashes the code while collision matching matches the number, so the two must not diverge")]
    CodeNumberMismatch(String, String, String),
    #[error("account '{0}' references parent '{1}' which does not appear earlier in the dataset")]
    OrphanParent(String, String),
    #[error("dataset is missing a required '{0}' account")]
    MissingRequired(&'static str),
}

/// Validate dataset invariants (see module docs). Pure — no I/O.
pub fn validate_dataset(ds: &ChartDataset) -> Result<(), DatasetError> {
    let mut seen_numbers: HashSet<&str> = HashSet::new();
    let mut seen_codes: HashSet<&str> = HashSet::new();
    let mut seen_sort_orders: HashSet<i32> = HashSet::new();

    for a in &ds.accounts {
        if a.number.trim().is_empty() || a.code.trim().is_empty() {
            return Err(DatasetError::EmptyIdentifier(a.name.clone()));
        }
        if a.name.trim().is_empty() {
            return Err(DatasetError::EmptyName(a.number.clone()));
        }
        if a.code != a.number.replace('.', "") {
            return Err(DatasetError::CodeNumberMismatch(
                a.name.clone(),
                a.code.clone(),
                a.number.clone(),
            ));
        }
        if !seen_numbers.insert(a.number.as_str()) {
            return Err(DatasetError::DuplicateNumber(a.number.clone()));
        }
        if !seen_codes.insert(a.code.as_str()) {
            return Err(DatasetError::DuplicateCode(a.code.clone()));
        }
        if !seen_sort_orders.insert(a.sort_order) {
            return Err(DatasetError::DuplicateSortOrder(a.sort_order));
        }
        let currency_ok =
            a.currency.len() == 3 && a.currency.bytes().all(|b| b.is_ascii_uppercase());
        if !currency_ok {
            return Err(DatasetError::InvalidCurrency(
                a.name.clone(),
                a.currency.clone(),
            ));
        }
        if let Some(parent) = &a.parent_code {
            // Parents-first order: the parent must already have been defined. This
            // check subsumes both "parent missing" and "cycle" (a cycle always has a
            // first edge whose parent has not appeared yet).
            if !seen_codes.contains(parent.as_str()) {
                return Err(DatasetError::OrphanParent(a.code.clone(), parent.clone()));
            }
        }
    }

    for required in REQUIRED_SUBTYPES {
        if !ds.accounts.iter().any(|a| a.account_subtype == required) {
            return Err(DatasetError::MissingRequired(match required {
                AccountSubtype::AccountsReceivable => "accounts_receivable",
                AccountSubtype::AccountsPayable => "accounts_payable",
                AccountSubtype::Bank => "bank",
                AccountSubtype::Cash => "cash",
                _ => unreachable!("REQUIRED_SUBTYPES only holds these four"),
            }));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(number: &str, subtype: AccountSubtype) -> ChartAccountDef {
        ChartAccountDef {
            number: number.to_string(),
            code: number.replace('.', ""),
            name: format!("Account {number}"),
            account_type: AccountType::Asset,
            account_subtype: subtype,
            normal_balance: NormalBalance::Debit,
            parent_code: None,
            is_reconcilable: false,
            currency: "IDR".to_string(),
            sort_order: number.replace('.', "").parse().unwrap_or(0),
        }
    }

    fn minimal_dataset() -> ChartDataset {
        ChartDataset {
            code: "TEST".into(),
            version: "1".into(),
            name: "Test chart".into(),
            accounts: vec![
                def("1100", AccountSubtype::Bank),
                def("1200", AccountSubtype::Cash),
                def("1300", AccountSubtype::AccountsReceivable),
                def("2100", AccountSubtype::AccountsPayable),
            ],
        }
    }

    #[test]
    fn minimal_dataset_passes() {
        assert_eq!(validate_dataset(&minimal_dataset()), Ok(()));
    }

    #[test]
    fn rejects_duplicate_numbers() {
        let mut ds = minimal_dataset();
        ds.accounts.push(def("1100", AccountSubtype::Tax));
        assert_eq!(
            validate_dataset(&ds),
            Err(DatasetError::DuplicateNumber("1100".into()))
        );
    }

    #[test]
    fn rejects_orphan_parent() {
        let mut ds = minimal_dataset();
        let mut child = def("1110", AccountSubtype::Bank);
        child.parent_code = Some("9999".into());
        ds.accounts.push(child);
        assert!(matches!(
            validate_dataset(&ds),
            Err(DatasetError::OrphanParent(_, _))
        ));
    }

    #[test]
    fn requires_ar_ap_bank_cash() {
        let mut ds = minimal_dataset();
        ds.accounts.retain(|a| a.account_subtype != AccountSubtype::Cash);
        assert!(matches!(
            validate_dataset(&ds),
            Err(DatasetError::MissingRequired("cash"))
        ));
    }

    #[test]
    fn parent_referenced_later_is_orphan() {
        let mut ds = minimal_dataset();
        let mut child = def("1150", AccountSubtype::Bank);
        child.parent_code = Some("1100".into());
        ds.accounts.insert(0, child); // child BEFORE its parent
        assert!(matches!(
            validate_dataset(&ds),
            Err(DatasetError::OrphanParent(_, _))
        ));
    }
}
