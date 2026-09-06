-- EMV(QRCPS) merchant-presented QR display configuration (one row per company,
-- optionally per bank account). This is the QRIS display half of the pair whose
-- channel-codec half (payment-gateway QRIS/VA notification codecs) stays parked:
-- the row carries only merchant-presented static data. The payload itself is
-- never stored — it is rebuilt from this row plus the invoice parameters at
-- display time, so a config correction immediately changes every rendered QR.
--
-- Hand-authored migration (no generator header): the table is verb-backed
-- (EmvQrService upsert/read); it deliberately mounts no CRUD surface.

CREATE SCHEMA IF NOT EXISTS accounting;

CREATE TABLE IF NOT EXISTS accounting.emv_qr_configs (
    id UUID NOT NULL DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL,
    -- Merchant-presented data (EMV QRCPS Merchant-Presented Mode, QRIS profile).
    merchant_name TEXT NOT NULL,
    merchant_city TEXT NOT NULL,
    country_code TEXT NOT NULL DEFAULT 'ID',
    -- QRIS merchant category code, 4 digits.
    mcc TEXT NOT NULL,
    -- QRIS global unique identifier (the tag-26 GUI; Bank Indonesia's is
    -- ID.CO.QRIS.WWW). Kept configurable so other EMV profiles stay possible.
    gui TEXT NOT NULL DEFAULT 'ID.CO.QRIS.WWW',
    -- QRIS merchant identifier: merchant PAN, national ID, or 'UME' when the
    -- acquirer has not issued one.
    merchant_identifier TEXT NOT NULL,
    -- Company default currency for the payload's tag 53 (ISO-4217 alpha; the
    -- numeric translation lives in the builder's currency table).
    currency TEXT NOT NULL DEFAULT 'IDR',
    -- EMV tag 01: '11' static (reusable) or '12' dynamic (single use).
    initiation_method TEXT NOT NULL DEFAULT '11',
    -- Optional anchor to the bank account whose statement collects QRIS
    -- proceeds. Logical reference only (banking.bank_accounts owns the row);
    -- no cross-schema foreign key by contract.
    bank_account_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    PRIMARY KEY (id),
    CONSTRAINT emv_qr_configs_initiation_method_check
        CHECK (initiation_method IN ('11', '12'))
);

-- One config per (company, bank account) slot. NULL bank_account_id is the
-- company-wide default; COALESCE keeps that slot unique too (Postgres treats
-- NULLs as distinct under a plain unique constraint).
CREATE UNIQUE INDEX IF NOT EXISTS uq_emv_qr_configs_company_bank_slot
    ON accounting.emv_qr_configs (
        company_id,
        COALESCE(bank_account_id, '00000000-0000-0000-0000-000000000000'::uuid)
    );
CREATE INDEX IF NOT EXISTS idx_emv_qr_configs_company_id
    ON accounting.emv_qr_configs (company_id);

-- Company fence (ADR-0014: strict) — same posture as every company-scoped table
-- in this module: a session sees only rows whose company_id equals the
-- request-scoped company (set_config('app.company_id', <uuid>, true)); an unset
-- var sees zero rows (fail-closed). Requires the app to connect as a
-- non-superuser role; migrations/seeders run as the owner and bypass.
ALTER TABLE accounting.emv_qr_configs ENABLE ROW LEVEL SECURITY;
ALTER TABLE accounting.emv_qr_configs FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS emv_qr_configs_company_isolation ON accounting.emv_qr_configs;
CREATE POLICY emv_qr_configs_company_isolation ON accounting.emv_qr_configs
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
