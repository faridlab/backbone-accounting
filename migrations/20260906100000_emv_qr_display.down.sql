-- Down: drop the EMV QR display configuration surface.

DROP POLICY IF EXISTS emv_qr_configs_company_isolation ON accounting.emv_qr_configs;
DROP TABLE IF EXISTS accounting.emv_qr_configs;
