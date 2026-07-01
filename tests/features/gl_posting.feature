# GL Posting acceptance oracle — backbone-accounting
# Flow map:    docs/business-flows/gl-posting.md
# Golden cases: docs/business-flows/golden-cases.md
# Declarative, business-level. One When per scenario. Money is exact IDR decimal(18,2).
# Tax lines (PPN/PPh) are GIVEN inputs — accounting records them, it does not compute tax.

Feature: Post financial effects to the General Ledger
  In order to keep one balanced ledger of record for every business transaction
  As a producing module (billing, payments, purchasing)
  I want to emit a balanced posting request and have accounting record it immutably

  Background:
    Given a company "ACME" with an open fiscal period covering the posting date
    And the chart of accounts:
      | code | name             | account_type | account_subtype       | normal_balance |
      | 1100 | Bank BCA         | asset        | bank                  | debit          |
      | 1200 | Piutang Usaha    | asset        | accounts_receivable   | debit          |
      | 1210 | PPN Masukan      | asset        | tax                   | debit          |
      | 2100 | Utang Usaha      | liability    | accounts_payable      | credit         |
      | 2200 | PPN Keluaran     | liability    | tax                   | credit         |
      | 2300 | Utang PPh 23     | liability    | tax                   | credit         |
      | 4000 | Pendapatan       | revenue      | operating_revenue     | credit         |
      | 5000 | Beban Operasional| expense      | operating_expense     | debit          |
    And a customer party "CUST-1"
    And a supplier party "SUPP-1"

  # ---------------------------------------------------------------------------
  # Flow 1 — happy path: post a balanced entry (GC-1)
  # ---------------------------------------------------------------------------
  @happy-path @module:accounting @gc-1
  Scenario: Sales invoice with PPN Output is posted to the ledger
    Given a posting request from source "order" with lines:
      | account | debit        | credit       | party         |
      | 1200    | 1,110,000.00 | 0.00         | customer CUST-1 |
      | 4000    | 0.00         | 1,000,000.00 |               |
      | 2200    | 0.00         | 110,000.00   |               |
    When the producer submits the posting request
    Then the posting is accepted with status "posted"
    And a journal is written with 3 ledger entries
    And the ledger entries are:
      | account | debit        | credit       |
      | 1200    | 1,110,000.00 | 0.00         |
      | 4000    | 0.00         | 1,000,000.00 |
      | 2200    | 0.00         | 110,000.00   |
    And total debit equals total credit at 1,110,000.00
    And the accounts receivable balance for "CUST-1" is 1,110,000.00
    And an "AccountingPostPosted" event is emitted once

  @happy-path @module:accounting @gc-3
  Scenario: Purchase invoice with PPN Input and PPh 23 withholding is posted
    Given a posting request from source "expense" with lines:
      | account | debit      | credit     | party           |
      | 5000    | 500,000.00 | 0.00       |                 |
      | 1210    | 55,000.00  | 0.00       |                 |
      | 2100    | 0.00       | 545,000.00 | supplier SUPP-1 |
      | 2300    | 0.00       | 10,000.00  |                 |
    When the producer submits the posting request
    Then the posting is accepted with status "posted"
    And a journal is written with 4 ledger entries
    And total debit equals total credit at 555,000.00
    And the accounts payable balance for "SUPP-1" is 545,000.00

  # ---------------------------------------------------------------------------
  # Flow 7 — AR/AP subledger aging: payment settles the receivable (GC-2)
  # ---------------------------------------------------------------------------
  @happy-path @module:accounting @gc-2
  Scenario: Payment received settles the customer receivable to zero
    Given the sales invoice for "CUST-1" from golden case GC-1 has been posted
    And a posting request from source "payment" with lines:
      | account | debit        | credit       | party           |
      | 1100    | 1,110,000.00 | 0.00         |                 |
      | 1200    | 0.00         | 1,110,000.00 | customer CUST-1 |
    When the producer submits the posting request
    Then the posting is accepted with status "posted"
    And the accounts receivable balance for "CUST-1" is 0.00
    And the ledger balance for account "1100" is 1,110,000.00

  # ---------------------------------------------------------------------------
  # Flow 6 — reversal: swapped debit/credit, net effect zero (GC-4)
  # ---------------------------------------------------------------------------
  @happy-path @module:accounting @gc-4
  Scenario: Reversing the sales invoice nets the general ledger to zero
    Given the sales invoice for "CUST-1" from golden case GC-1 has been posted as the original
    And a reversal posting request referencing the original
    When the producer submits the posting request
    Then the posting is accepted with status "posted"
    And the reversing ledger entries are:
      | account | debit        | credit       |
      | 1200    | 0.00         | 1,110,000.00 |
      | 4000    | 1,000,000.00 | 0.00         |
      | 2200    | 110,000.00   | 0.00         |
    And the net ledger balance for every account across the original and reversal is 0.00
    And the original post links to the reversing post and back
    And the accounts receivable balance for "CUST-1" is 0.00

  # ---------------------------------------------------------------------------
  # Flow 5 — idempotency (GC-8)
  # ---------------------------------------------------------------------------
  @edge @module:accounting @gc-8
  Scenario: Re-submitting a post with the same idempotency key does not double-write
    Given the sales invoice for "CUST-1" from golden case GC-1 has been posted with idempotency key "K1"
    And a posting request identical to GC-1 with idempotency key "K1"
    When the producer submits the posting request
    Then the original journal is returned
    And exactly 1 journal and 3 ledger entries exist in total
    And the accounts receivable balance for "CUST-1" is 1,110,000.00

  # ---------------------------------------------------------------------------
  # Flow 2 — reject unbalanced (GC-5)
  # ---------------------------------------------------------------------------
  @edge @module:accounting @gc-5
  Scenario: Unbalanced posting is rejected and writes nothing
    Given a posting request from source "manual" with lines:
      | account | debit  | credit |
      | 5000    | 100.00 | 0.00   |
      | 1100    | 0.00   | 90.00  |
    When the producer submits the posting request
    Then the posting is rejected with error code "unbalanced"
    And no journal, journal line, or ledger entry is written
    And an "AccountingPostFailed" event is emitted

  # ---------------------------------------------------------------------------
  # Flow 4 — party rule (GC-6, GC-7)
  # ---------------------------------------------------------------------------
  @edge @module:accounting @gc-6
  Scenario: Receivable line without a party is rejected
    Given a posting request from source "order" with lines:
      | account | debit        | credit       | party |
      | 1200    | 1,110,000.00 | 0.00         |       |
      | 4000    | 0.00         | 1,000,000.00 |       |
      | 2200    | 0.00         | 110,000.00   |       |
    When the producer submits the posting request
    Then the posting is rejected with error code "party_required"
    And no journal, journal line, or ledger entry is written

  @edge @module:accounting @gc-7
  Scenario: Party on a non-receivable/payable line is rejected
    Given a posting request from source "order" with lines:
      | account | debit        | credit       | party           |
      | 1200    | 1,110,000.00 | 0.00         | customer CUST-1 |
      | 4000    | 0.00         | 1,000,000.00 | customer CUST-1 |
      | 2200    | 0.00         | 110,000.00   |                 |
    When the producer submits the posting request
    Then the posting is rejected with error code "party_not_allowed"
    And no journal, journal line, or ledger entry is written

  # ---------------------------------------------------------------------------
  # Flow 3 — structural / account / period rejects (GC-9, GC-10, GC-11)
  # ---------------------------------------------------------------------------
  @edge @module:accounting @gc-9
  Scenario: A posting with fewer than two lines is rejected
    Given a posting request from source "manual" with lines:
      | account | debit  | credit |
      | 1100    | 100.00 | 0.00   |
    When the producer submits the posting request
    Then the posting is rejected with error code "too_few_lines"
    And no journal, journal line, or ledger entry is written

  @edge @module:accounting @gc-10
  Scenario: A posting into a closed fiscal period is rejected
    Given the fiscal period covering the posting date is closed
    And a balanced posting request from source "order" with lines:
      | account | debit        | credit       | party           |
      | 1200    | 1,110,000.00 | 0.00         | customer CUST-1 |
      | 4000    | 0.00         | 1,000,000.00 |                 |
      | 2200    | 0.00         | 110,000.00   |                 |
    When the producer submits the posting request
    Then the posting is rejected with error code "period_closed"
    And no journal, journal line, or ledger entry is written

  @edge @module:accounting @gc-11
  Scenario: A posting against a header account is rejected
    Given a header account "1000" that cannot be posted to directly
    And a balanced posting request from source "manual" with lines:
      | account | debit  | credit |
      | 1000    | 100.00 | 0.00   |
      | 1100    | 0.00   | 100.00 |
    When the producer submits the posting request
    Then the posting is rejected with error code "non_postable_account"
    And no journal, journal line, or ledger entry is written

  # ---------------------------------------------------------------------------
  # Balance rule across boundary values (R1)
  # ---------------------------------------------------------------------------
  @edge @module:accounting
  Scenario Outline: Balance rule accepts equal sides and rejects unequal sides
    Given a posting request from source "manual" with lines:
      | account | debit    | credit    |
      | 5000    | <debit>  | 0.00      |
      | 1100    | 0.00     | <credit>  |
    When the producer submits the posting request
    Then the posting <result>

    Examples:
      | debit      | credit     | result                                    |
      | 100.00     | 100.00     | is accepted with status "posted"          |
      | 100.00     | 100.01     | is rejected with error code "unbalanced"  |
      | 100.01     | 100.00     | is rejected with error code "unbalanced"  |
      | 999,999.99 | 999,999.99 | is accepted with status "posted"          |
</content>
