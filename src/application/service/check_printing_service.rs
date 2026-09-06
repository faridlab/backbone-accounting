//! Check printing — the per-bank-journal check-number sequence and the
//! printed-check registry.
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). The Odoo
//! `account_check_printing` shape ported onto this estate: a "bank journal" is
//! a company's bank account (`banking.bank_accounts` owns the row; carried
//! here as a logical reference — no cross-schema foreign key by contract).
//!
//! Numbering modes:
//! - `auto`    — the verb allocates the next number from
//!   `bank_check_sequences.next_number`. The allocation is a single
//!   `UPDATE ... RETURNING` inside the verb's own transaction: the row lock IS
//!   the serialization point, so two concurrent allocations block each other
//!   and always receive distinct numbers — there is no read-then-write gap.
//! - `manual`  — the officer supplies the number (prenumbered stock); the
//!   registry's unique `(company, bank journal, number)` constraint refuses a
//!   reuse with a typed error.
//!
//! Numbers are capped at 2147483647 (MAX_INT32): sequence columns upstream are
//! 32-bit and printed numbers must stay portable. An allocation that would
//! cross the cap refuses with `check_number_overflow` and the whole
//! transaction rolls back — no number is consumed.

use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// Upstream's 32-bit sequence ceiling.
pub const MAX_CHECK_NUMBER: i64 = 2_147_483_647;

#[derive(Debug, Clone)]
pub struct RegisterSequence {
    pub company_id: Uuid,
    pub bank_account_id: Uuid,
    /// "auto" or "manual".
    pub numbering_mode: String,
    /// First number the sequence will hand out (auto mode).
    pub next_number: i64,
}

#[derive(Debug, Clone)]
pub struct RecordCheck {
    pub company_id: Uuid,
    pub bank_account_id: Uuid,
    pub payment_id: Uuid,
    pub payment_number: Option<String>,
    pub amount: Decimal,
    pub payee_name: Option<String>,
    /// Required under `manual` numbering; must be absent under `auto`.
    pub check_number: Option<String>,
    /// Officer recording the print.
    pub actor: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SequenceAck {
    pub sequence_id: Uuid,
    pub company_id: Uuid,
    pub bank_account_id: Uuid,
    pub numbering_mode: String,
    pub next_number: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AllocationAck {
    pub first: i64,
    pub last: i64,
    pub numbers: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrintedCheckAck {
    pub printed_check_id: Uuid,
    pub company_id: Uuid,
    pub bank_account_id: Uuid,
    pub payment_id: Uuid,
    pub check_number: String,
    pub status: String,
}

#[derive(Debug)]
pub enum CheckPrintingError {
    /// No sequence registered for (company, bank journal).
    SequenceNotRegistered(Uuid),
    /// A number was supplied to an auto sequence, or missing under manual.
    NumberingModeConflict { mode: String, detail: String },
    /// A manual number is malformed (empty, non-digit, over length, over cap).
    InvalidCheckNumber(String),
    /// The number is already in the registry for this bank journal.
    DuplicateCheckNumber(String),
    /// The next allocation would cross MAX_INT32.
    CheckNumberOverflow,
    /// Registry row not found for the company.
    CheckNotFound(Uuid),
    /// Voiding a check that is already voided.
    AlreadyVoided(Uuid),
    /// Check amounts must be strictly positive.
    InvalidAmount,
    /// Storage failure.
    Internal(String),
}

impl CheckPrintingError {
    pub fn code(&self) -> &'static str {
        match self {
            CheckPrintingError::SequenceNotRegistered(_) => "sequence_not_registered",
            CheckPrintingError::NumberingModeConflict { .. } => "numbering_mode_conflict",
            CheckPrintingError::InvalidCheckNumber(_) => "invalid_check_number",
            CheckPrintingError::DuplicateCheckNumber(_) => "duplicate_check_number",
            CheckPrintingError::CheckNumberOverflow => "check_number_overflow",
            CheckPrintingError::CheckNotFound(_) => "check_not_found",
            CheckPrintingError::AlreadyVoided(_) => "check_already_voided",
            CheckPrintingError::InvalidAmount => "invalid_amount",
            CheckPrintingError::Internal(_) => "internal_error",
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            CheckPrintingError::SequenceNotRegistered(_) => 404,
            CheckPrintingError::CheckNotFound(_) => 404,
            CheckPrintingError::Internal(_) => 500,
            _ => 422,
        }
    }
}

impl std::fmt::Display for CheckPrintingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckPrintingError::SequenceNotRegistered(bank) => write!(
                f,
                "sequence_not_registered: no check sequence for bank account {bank}"
            ),
            CheckPrintingError::NumberingModeConflict { mode, detail } => write!(
                f,
                "numbering_mode_conflict: sequence is {mode}; {detail}"
            ),
            CheckPrintingError::InvalidCheckNumber(n) => write!(
                f,
                "invalid_check_number: {n:?} (want 1..=10 digits, value <= {MAX_CHECK_NUMBER})"
            ),
            CheckPrintingError::DuplicateCheckNumber(n) => write!(
                f,
                "duplicate_check_number: {n} is already registered for this bank journal"
            ),
            CheckPrintingError::CheckNumberOverflow => write!(
                f,
                "check_number_overflow: allocation would cross {MAX_CHECK_NUMBER}"
            ),
            CheckPrintingError::CheckNotFound(id) => {
                write!(f, "check_not_found: printed check {id}")
            }
            CheckPrintingError::AlreadyVoided(id) => {
                write!(f, "check_already_voided: printed check {id}")
            }
            CheckPrintingError::InvalidAmount => {
                write!(f, "invalid_amount: check amount must be strictly positive")
            }
            CheckPrintingError::Internal(e) => write!(f, "internal_error: {e}"),
        }
    }
}
impl std::error::Error for CheckPrintingError {}

fn internal(e: impl std::fmt::Display) -> CheckPrintingError {
    CheckPrintingError::Internal(e.to_string())
}

fn validate_manual_number(raw: &str) -> Result<String, CheckPrintingError> {
    let n = raw.trim();
    if n.is_empty()
        || n.chars().count() > 10
        || !n.chars().all(|c| c.is_ascii_digit())
    {
        return Err(CheckPrintingError::InvalidCheckNumber(raw.to_string()));
    }
    // All-digits ≤10 chars always parses; treat any parse failure as invalid.
    let value = n.parse::<i64>().map_err(|_| CheckPrintingError::InvalidCheckNumber(raw.to_string()))?;
    if value > MAX_CHECK_NUMBER {
        return Err(CheckPrintingError::InvalidCheckNumber(raw.to_string()));
    }
    Ok(n.to_string())
}

#[derive(Clone)]
pub struct CheckPrintingService {
    pool: PgPool,
}

#[derive(sqlx::FromRow)]
struct SequenceRow {
    id: Uuid,
    numbering_mode: String,
    next_number: i64,
}

impl CheckPrintingService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Register (or replace) the check sequence for one bank journal.
    pub async fn register_sequence(
        &self,
        req: RegisterSequence,
    ) -> Result<SequenceAck, CheckPrintingError> {
        if req.numbering_mode != "auto" && req.numbering_mode != "manual" {
            return Err(CheckPrintingError::NumberingModeConflict {
                mode: req.numbering_mode.clone(),
                detail: "numbering_mode must be \"auto\" or \"manual\"".into(),
            });
        }
        if req.next_number < 1 || req.next_number > MAX_CHECK_NUMBER {
            return Err(CheckPrintingError::InvalidCheckNumber(
                req.next_number.to_string(),
            ));
        }

        let mut tx = self.pool.begin().await.map_err(|e| internal(e))?;
        backbone_orm::company_scope::bind_company_on(&mut tx, req.company_id)
            .await
            .map_err(|e| internal(e))?;

        let row = sqlx::query_as::<_, SequenceRow>(
            r#"INSERT INTO accounting.bank_check_sequences
                 (company_id, bank_account_id, numbering_mode, next_number, updated_at)
               VALUES ($1,$2,$3,$4,NOW())
               ON CONFLICT (company_id, bank_account_id) DO UPDATE SET
                 numbering_mode = EXCLUDED.numbering_mode,
                 next_number   = EXCLUDED.next_number,
                 updated_at    = NOW()
               RETURNING id, numbering_mode, next_number"#,
        )
        .bind(req.company_id)
        .bind(req.bank_account_id)
        .bind(&req.numbering_mode)
        .bind(req.next_number)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| internal(e))?;

        tx.commit().await.map_err(|e| internal(e))?;
        Ok(SequenceAck {
            sequence_id: row.id,
            company_id: req.company_id,
            bank_account_id: req.bank_account_id,
            numbering_mode: row.numbering_mode,
            next_number: row.next_number,
        })
    }

    /// Allocate `count` consecutive numbers from an AUTO sequence.
    ///
    /// Single-statement atomic allocation: the UPDATE takes the row lock and
    /// returns the post-bump cursor in one step, so a concurrent allocator
    /// blocks on the lock and continues from the bumped value — there is no
    /// window in which two callers read the same cursor. An overflow refuses
    /// and the transaction rolls back, leaving the cursor untouched.
    pub async fn allocate_check_numbers(
        &self,
        company_id: Uuid,
        bank_account_id: Uuid,
        count: i64,
    ) -> Result<AllocationAck, CheckPrintingError> {
        if count < 1 {
            return Err(CheckPrintingError::InvalidCheckNumber(
                "count must be >= 1".into(),
            ));
        }
        let mut tx = self.pool.begin().await.map_err(|e| internal(e))?;
        let ack = self
            .allocate_check_numbers_on(&mut tx, company_id, bank_account_id, count)
            .await?;
        tx.commit().await.map_err(|e| internal(e))?;
        Ok(ack)
    }

    /// In-transaction allocation core, shared with the record verb so a check
    /// number is allocated and its registry row written atomically.
    async fn allocate_check_numbers_on(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        company_id: Uuid,
        bank_account_id: Uuid,
        count: i64,
    ) -> Result<AllocationAck, CheckPrintingError> {
        backbone_orm::company_scope::bind_company_on(tx, company_id)
            .await
            .map_err(|e| internal(e))?;

        let bumped: Option<i64> = sqlx::query_scalar(
            r#"UPDATE accounting.bank_check_sequences
                  SET next_number = next_number + $3, updated_at = NOW()
                WHERE company_id = $1 AND bank_account_id = $2
                  AND numbering_mode = 'auto'
                RETURNING next_number"#,
        )
        .bind(company_id)
        .bind(bank_account_id)
        .bind(count)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| internal(e))?;

        let Some(new_cursor) = bumped else {
            // Distinguish "no row" from "wrong mode" for the refusal message.
            let mode: Option<String> = sqlx::query_scalar(
                r#"SELECT numbering_mode FROM accounting.bank_check_sequences
                    WHERE company_id=$1 AND bank_account_id=$2"#,
            )
            .bind(company_id)
            .bind(bank_account_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| internal(e))?;
            return Err(match mode {
                None => CheckPrintingError::SequenceNotRegistered(bank_account_id),
                Some(m) => CheckPrintingError::NumberingModeConflict {
                    mode: m,
                    detail: "auto allocation requires numbering_mode = \"auto\"".into(),
                },
            });
        };

        let first = new_cursor - count;
        if new_cursor - 1 > MAX_CHECK_NUMBER {
            // Refuse and roll the whole transaction back — the bump above dies
            // with it, so no number is consumed.
            return Err(CheckPrintingError::CheckNumberOverflow);
        }
        Ok(AllocationAck {
            first,
            last: new_cursor - 1,
            numbers: (first..new_cursor).collect(),
        })
    }

    /// Record one printed check: allocate (auto) or validate (manual) the
    /// number and write the registry row in the SAME transaction. The unique
    /// `(company, bank journal, number)` constraint is the cross-payment
    /// uniqueness guard — a violation maps to `duplicate_check_number`.
    pub async fn record_printed_check(
        &self,
        req: RecordCheck,
    ) -> Result<PrintedCheckAck, CheckPrintingError> {
        if req.amount <= Decimal::ZERO {
            return Err(CheckPrintingError::InvalidAmount);
        }

        let mut tx = self.pool.begin().await.map_err(|e| internal(e))?;
        backbone_orm::company_scope::bind_company_on(&mut tx, req.company_id)
            .await
            .map_err(|e| internal(e))?;

        let seq = sqlx::query_as::<_, SequenceRow>(
            r#"SELECT id, numbering_mode, next_number
                 FROM accounting.bank_check_sequences
                WHERE company_id=$1 AND bank_account_id=$2
                FOR UPDATE"#,
        )
        .bind(req.company_id)
        .bind(req.bank_account_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| internal(e))?;

        let check_number = match seq {
            None => {
                return Err(CheckPrintingError::SequenceNotRegistered(
                    req.bank_account_id,
                ))
            }
            Some(row) => match row.numbering_mode.as_str() {
                "auto" => {
                    if req.check_number.is_some() {
                        return Err(CheckPrintingError::NumberingModeConflict {
                            mode: "auto".into(),
                            detail: "auto sequences allocate; do not supply a check number"
                                .into(),
                        });
                    }
                    // One-number allocation through the shared atomic core —
                    // same transaction as the registry insert.
                    self.allocate_check_numbers_on(
                        &mut tx,
                        req.company_id,
                        req.bank_account_id,
                        1,
                    )
                    .await?
                    .first
                    .to_string()
                }
                "manual" => validate_manual_number(
                    req.check_number
                        .as_deref()
                        .ok_or(CheckPrintingError::NumberingModeConflict {
                            mode: "manual".into(),
                            detail: "manual sequences require the officer's check number"
                                .into(),
                        })?,
                )?,
                other => {
                    return Err(CheckPrintingError::NumberingModeConflict {
                        mode: other.to_string(),
                        detail: "unknown numbering mode".into(),
                    })
                }
            },
        };

        let id: Uuid = match sqlx::query_scalar(
            r#"INSERT INTO accounting.printed_checks
                 (company_id, bank_account_id, payment_id, payment_number,
                  check_number, amount, payee_name, printed_by, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NOW())
               RETURNING id"#,
        )
        .bind(req.company_id)
        .bind(req.bank_account_id)
        .bind(req.payment_id)
        .bind(&req.payment_number)
        .bind(&check_number)
        .bind(req.amount)
        .bind(&req.payee_name)
        .bind(req.actor)
        .fetch_one(&mut *tx)
        .await
        {
            Ok(id) => id,
            Err(sqlx::Error::Database(db)) if db.is_unique_violation() => {
                return Err(CheckPrintingError::DuplicateCheckNumber(check_number))
            }
            Err(e) => return Err(internal(e)),
        };

        tx.commit().await.map_err(|e| internal(e))?;
        Ok(PrintedCheckAck {
            printed_check_id: id,
            company_id: req.company_id,
            bank_account_id: req.bank_account_id,
            payment_id: req.payment_id,
            check_number,
            status: "printed".into(),
        })
    }

    /// Void a printed check. The number stays consumed (the registry row and
    /// its uniqueness survive) — reversing the payment is the GL-side action
    /// and never frees a check number for reuse.
    pub async fn void_printed_check(
        &self,
        company_id: Uuid,
        printed_check_id: Uuid,
        actor: Option<Uuid>,
    ) -> Result<PrintedCheckAck, CheckPrintingError> {
        let mut tx = self.pool.begin().await.map_err(|e| internal(e))?;
        backbone_orm::company_scope::bind_company_on(&mut tx, company_id)
            .await
            .map_err(|e| internal(e))?;

        let voided = sqlx::query_as::<_, VoidRow>(
            r#"UPDATE accounting.printed_checks
                  SET status='voided', updated_at=NOW(),
                      metadata = metadata
                          || jsonb_build_object(
                               'voided_by', $3,
                               'voided_at', to_jsonb(NOW()))
                WHERE id=$2 AND company_id=$1 AND status='printed'
                RETURNING id, company_id, bank_account_id, payment_id,
                          check_number, status"#,
        )
        .bind(company_id)
        .bind(printed_check_id)
        .bind(actor.map(|a| a.to_string()))
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| internal(e))?;

        let row = match voided {
            Some(r) => r,
            None => {
                let exists: Option<String> = sqlx::query_scalar(
                    "SELECT status FROM accounting.printed_checks WHERE id=$2 AND company_id=$1",
                )
                .bind(company_id)
                .bind(printed_check_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| internal(e))?;
                return Err(match exists.as_deref() {
                    Some("voided") => CheckPrintingError::AlreadyVoided(printed_check_id),
                    _ => CheckPrintingError::CheckNotFound(printed_check_id),
                });
            }
        };

        tx.commit().await.map_err(|e| internal(e))?;
        Ok(PrintedCheckAck {
            printed_check_id: row.id,
            company_id: row.company_id,
            bank_account_id: row.bank_account_id,
            payment_id: row.payment_id,
            check_number: row.check_number,
            status: row.status,
        })
    }
}

#[derive(sqlx::FromRow)]
struct VoidRow {
    id: Uuid,
    company_id: Uuid,
    bank_account_id: Uuid,
    payment_id: Uuid,
    check_number: String,
    status: String,
}
