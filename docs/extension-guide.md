# Extension Guide — Accounting (`backbone-accounting`)

> Reader: **developer-consumer** (billing / selling / payments / banking engineer).
> How to drive and extend the GL core **without modifying or breaking it**. Governed by
> `docs/erp/extension-contract.md`. This module is authored once; many verticals enhance it via its
> public surface — never by forking it.

## 1. The one rule

**Accounting imports no producer, and no producer imports accounting's internals.** The only things
crossing the boundary are: the **`AccountingPost` inbound contract** (you emit it), the **exported
events** (you subscribe to them), and the **exported DTOs** (you read them). Everything else —
domain entities, repositories, private services, generated code, `// <<< CUSTOM` blocks — is
internal and may change any release.

## 2. Public surface, by stability tier

### Tier A — Public & stable (versioned; break = major bump)

| Surface | Where | What you may depend on |
|---------|-------|------------------------|
| **Inbound contract `AccountingPost`** | `POST /accounting/posts` (see FSD §8) | The documented request shape — `company_id`, `source_type`, `source_id`, `posting_type`, `lines[]`. Emit it to record any financial effect. |
| **Exported events** | `src/exports/events.rs` | `AccountingPostPosted { post_id, source_type, source_id, journal_id, status }`, `AccountingPostFailed { post_id, source_type, source_id, error_code, error_message }`. Subscribe to reconcile your own document state. |
| **Exported DTOs** | `src/exports/types.rs` | Posting result/status DTO; Account-lookup DTO (resolve `account_code`→`account_id`/`account_subtype`); trial-balance / statement projection DTOs. |
| **Logical FKs** | `party_id`, `project_id`, `department_id`, `branch_id`, `company_id` | Target entity `id`s are stable references; you supply them on posting lines. |
| **`user_owned:` globs** | your app's `metaphor.codegen.yaml` | App-specific wiring/composition the generator never overwrites. |

### Tier B — Supported but coupled (extend *your own* copy)

| Surface | Where | Use |
|---------|-------|-----|
| **`*_custom.rs` siblings** | e.g. your `billing_posting_custom.rs` | Hand-authored logic inside a module **you own**. Survives regeneration. Not a way to reach into accounting. |

### Tier C — Internal, NOT a contract (never depend on)

- Accounting's domain entities (`Account`, `Journal`, `JournalLine`, `Ledger`, `FiscalPeriod`,
  `Reconciliation`, `AccountingPost` as a struct), repositories, private services (`src/domain`, `src/infrastructure`).
- `// <<< CUSTOM … // END CUSTOM` blocks inside accounting's generated files.
- The 12 CRUD endpoints on accounting entities — for accounting's own admin use, not your write path. **Never** create a `Journal`/`Ledger` directly; always go through `AccountingPost`.

## 3. How a consumer drives accounting (the golden path)

A producer (e.g. `backbone-billing` posting a sales invoice) does exactly this:

1. **Compute your lines.** Assemble balanced `lines[]` (Σdebit = Σcredit). Ask `backbone-tax-id` for
   any PPN/PPh lines and include them — accounting does not compute tax.
2. **Resolve accounts** by code via the exported **Account-lookup DTO** (`AR`, `REVENUE`,
   `PPN_OUTPUT` → `account_id`). Never hardcode UUIDs.
3. **Attach the party** on receivable/payable lines (`party_type = customer`, `party_id`) — required
   iff `account_subtype ∈ {accounts_receivable, accounts_payable}` (R3), else the post is rejected.
4. **Emit the `AccountingPost`** to `POST /accounting/posts` with your `source_type`/`source_id`
   (your document id — opaque to accounting) and `posting_type = original`.
5. **Reconcile via event.** Subscribe to `AccountingPostPosted` (mark your invoice `posted`, store
   `journal_id`) and `AccountingPostFailed` (surface `error_code`, e.g. balance/party/period). Do
   **not** block on a synchronous return — treat posting as eventually consistent.

**Example (sales invoice, IDR, PPN 11% — golden case G1):**
```
AccountingPost {
  company_id, source_type: order, source_id: <invoice_id>, source_reference: "INV-2026-00042",
  posting_type: original, currency: "IDR", posting_date: 2026-07-01,
  lines: [
    { account_id: <AR>,          debit: 1_110_000, credit: 0, party_type: customer, party_id: <cust> },
    { account_id: <REVENUE>,     debit: 0, credit: 1_000_000 },
    { account_id: <PPN_OUTPUT>,  debit: 0, credit:   110_000, is_tax_line: true, tax_rate: 11.0 },
  ],
}
```
Accounting validates, writes one `Journal` + three `JournalLine`s + three `Ledger` rows, and emits
`AccountingPostPosted { journal_id, status: posted }`.

**To undo:** never edit the posted GL. Emit a **reversal** — same `source_type`/`source_id`,
`posting_type = reversal`, swapped debit/credit. Accounting mirrors the journal, links the posts,
and lands the reversal in the current open period (R7/R8, golden case G4).

## 4. How a consumer *extends* accounting without modifying it

Follow the decision rule (`extension-contract.md §3`):

1. **React to / add behavior on a post** → subscribe to `AccountingPostPosted` (Tier A). e.g. a
   loyalty vertical awards points when a sales post lands; a dunning vertical schedules a reminder.
   Fully decoupled, eventually consistent. **Default choice.**
2. **Drive accounting** → emit `AccountingPost` (Tier A). Your only write path into the GL.
3. **Read accounting's data** → consume exported DTOs, or maintain your own projection synced from
   the events (Tier A). Never query accounting's tables directly.
4. **App-specific composition/wiring** → put it in a `user_owned:` file (Tier A) — never overwritten.
5. **Logic inside a module you own** → your own `*_custom.rs` (Tier B).
6. **Never** edit accounting's generated code, depend on its `// <<< CUSTOM` blocks, or import its
   domain entities (Tier C).

**Worked extension:** a retail vertical wants "post-and-notify". It subscribes to
`AccountingPostPosted` in its own `retail_posting_hook_custom.rs`, reads the `journal_id` from the
event, and fires a notification. Accounting is untouched; the vertical's logic lives entirely in a
`*_custom.rs` it owns + an event subscription. This is the acceptance round-trip of
`extension-contract.md §5` — it must survive regeneration of both modules.

## 5. What is stable vs internal (quick reference)

| Stable (depend freely) | Internal (never depend) |
|------------------------|-------------------------|
| `AccountingPost` request shape | `AccountingPost` Rust struct, its repository |
| `AccountingPostPosted` / `AccountingPostFailed` events | `Journal` / `JournalLine` / `Ledger` entities |
| Exported DTOs (result, account-lookup, statement projection) | Posting-validation service internals, `*_custom.rs` of accounting |
| `party_id`/`project_id`/`department_id`/`branch_id`/`company_id` logical-FK ids | Ledger running-balance algorithm, sequence numbering |
| `PostingSourceType` / `PostingType` enum variants | `// <<< CUSTOM` blocks; the 12 CRUD routes as a write path |

## 6. Versioning & compatibility

- Events and exported DTOs are **versioned** (`extension-contract.md §4`). Additive changes (new
  optional field, new event) are minor; removing/renaming a field or changing semantics is **major**
  — the old version ships alongside the new with `@deprecated` and ≥1 migration cycle.
- **Logical FKs are part of the contract:** the target entity's `id` is stable; a change is announced
  via the same deprecation path.
- Modules carry independent semver; a per-release compatibility matrix says which versions compose.
- **Open item (pre-1.0):** `idempotency_key` is not yet a field on `AccountingPost` (see FSD §10) —
  today re-emitting the same `(source_type, source_id, posting_type)` is deduped by the composite
  unique index. Set your `source_id` stably so retries are idempotent.
