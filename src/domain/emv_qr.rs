//! EMV(QRCPS) merchant-presented-mode QR payload builder (the QRIS profile).
//!
//! Hand-authored (user-owned; see `metaphor.codegen.yaml`). Pure domain code —
//! no I/O. Builds the TLV payload string a renderer encodes into the QR shown
//! on an invoice:
//!
//! ```text
//! 00  payload format indicator  "01"
//! 01  point of initiation        "11" static (reusable) | "12" dynamic (one-time)
//! 26  merchant account template  26-00 GUI (QRIS: ID.CO.QRIS.WWW)
//!                               26-01 merchant identifier (PAN / national ID / UME)
//! 52  merchant category code     4 digits
//! 53  transaction currency      ISO-4217 numeric (from the supported map)
//! 54  transaction amount        decimal, "." separator, 1..=13 chars (omitted on
//!                               static QRs that let the payer enter the amount)
//! 58  country code              ISO-3166 alpha-2, uppercased
//! 59  merchant name             ≤ 25 chars after accent folding
//! 60  merchant city             ≤ 15 chars after accent folding
//! 62  additional data template  62-01 invoice/reference number, ≤ 25 chars
//! 63  CRC-16/CCITT-FALSE        poly 0x1021, init 0xFFFF, no reflection, no xorout,
//!                               computed over the payload including "6304"
//! ```
//!
//! Refusals are TYPED ([`EmvQrError`]) and every builder arm fails closed —
//! there is no fallback payload: an unsupported currency or overlong merchant
//! datum refuses the render rather than degrading it. This is deliberately the
//! DISPLAY half only; the channel half (notification codecs, charge creation,
//! polling/settlement) is parked pending a named PSP consumer.

use rust_decimal::Decimal;

/// Field-length limits from the EMV QRCPS merchant-presented specification.
pub const MAX_MERCHANT_NAME: usize = 25;
pub const MAX_MERCHANT_CITY: usize = 15;
pub const MAX_REFERENCE: usize = 25;
/// EMV caps the transaction-amount string (including the decimal separator) at 13.
pub const MAX_AMOUNT_LEN: usize = 13;

/// The merchant-presented data the payload is built from. Storage-backed
/// (`emv_qr_configs`); the invoice legs (amount / reference) ride the build
/// call, not the profile.
#[derive(Debug, Clone)]
pub struct EmvMerchantProfile {
    /// Tag 59. Folded (accents stripped) and length-checked at build time.
    pub merchant_name: String,
    /// Tag 60. Folded and length-checked at build time.
    pub merchant_city: String,
    /// Tag 58. ISO-3166 alpha-2.
    pub country: String,
    /// Tag 52. Exactly 4 digits.
    pub mcc: String,
    /// Tag 26-00 — the QRIS global unique identifier.
    pub gui: String,
    /// Tag 26-01 — merchant PAN, national ID, or "UME" (unregistered).
    pub merchant_identifier: String,
    /// Tag 01 — "11" static or "12" dynamic.
    pub initiation: String,
}

/// Typed builder refusal. `code()` is the stable error string asserted by tests.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum EmvQrError {
    /// Merchant name exceeds 25 chars after accent folding.
    MerchantNameTooLong(usize),
    /// Merchant city exceeds 15 chars after accent folding.
    MerchantCityTooLong(usize),
    /// Country code is not 2 ASCII letters.
    InvalidCountry(String),
    /// Merchant category code is not exactly 4 digits.
    InvalidMcc(String),
    /// Point of initiation is not "11" or "12".
    InvalidInitiation(String),
    /// GUI or merchant identifier failed validation (empty or overlong).
    InvalidMerchantAccount(String),
    /// Currency alpha code is not in the supported numeric map.
    CurrencyNotSupported(String),
    /// Amount is zero or negative.
    InvalidAmount,
    /// Amount does not fit the 13-character EMV string limit.
    AmountTooLong,
    /// Reference number exceeds 25 chars after folding.
    ReferenceTooLong(usize),
}

impl EmvQrError {
    pub fn code(&self) -> &'static str {
        match self {
            EmvQrError::MerchantNameTooLong(_) => "merchant_name_too_long",
            EmvQrError::MerchantCityTooLong(_) => "merchant_city_too_long",
            EmvQrError::InvalidCountry(_) => "invalid_country_code",
            EmvQrError::InvalidMcc(_) => "invalid_mcc",
            EmvQrError::InvalidInitiation(_) => "invalid_initiation_method",
            EmvQrError::InvalidMerchantAccount(_) => "invalid_merchant_account",
            EmvQrError::CurrencyNotSupported(_) => "currency_not_supported",
            EmvQrError::InvalidAmount => "invalid_amount",
            EmvQrError::AmountTooLong => "amount_too_long",
            EmvQrError::ReferenceTooLong(_) => "reference_too_long",
        }
    }
}

impl std::fmt::Display for EmvQrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmvQrError::MerchantNameTooLong(n) => write!(
                f,
                "merchant_name_too_long: {n} chars after folding (limit {MAX_MERCHANT_NAME})"
            ),
            EmvQrError::MerchantCityTooLong(n) => write!(
                f,
                "merchant_city_too_long: {n} chars after folding (limit {MAX_MERCHANT_CITY})"
            ),
            EmvQrError::InvalidCountry(c) => {
                write!(f, "invalid_country_code: {c:?} (want 2 ASCII letters)")
            }
            EmvQrError::InvalidMcc(m) => write!(f, "invalid_mcc: {m:?} (want 4 digits)"),
            EmvQrError::InvalidInitiation(i) => {
                write!(f, "invalid_initiation_method: {i:?} (want \"11\" or \"12\")")
            }
            EmvQrError::InvalidMerchantAccount(m) => write!(
                f,
                "invalid_merchant_account: {m} (GUI and merchant identifier must be 1..=32 printable-ASCII chars)"
            ),
            EmvQrError::CurrencyNotSupported(c) => write!(
                f,
                "currency_not_supported: {c} has no EMV numeric code in the supported map"
            ),
            EmvQrError::InvalidAmount => write!(f, "invalid_amount: must be strictly positive"),
            EmvQrError::AmountTooLong => write!(
                f,
                "amount_too_long: exceeds the {MAX_AMOUNT_LEN}-character EMV amount limit"
            ),
            EmvQrError::ReferenceTooLong(n) => write!(
                f,
                "reference_too_long: {n} chars after folding (limit {MAX_REFERENCE})"
            ),
        }
    }
}
impl std::error::Error for EmvQrError {}

/// ISO-4217 alpha → EMV numeric code, for the currencies the merchant-presented
/// QR flow supports. Anything outside the map refuses the build — the QR would
/// otherwise carry a currency no acquirer in the profile settles.
pub fn currency_numeric(alpha: &str) -> Option<&'static str> {
    let code = alpha.trim().to_ascii_uppercase();
    Some(match code.as_str() {
        "AUD" => "036",
        "BGN" => "975",
        "BHD" => "048",
        "BND" => "096",
        "BRL" => "986",
        "CAD" => "124",
        "CHF" => "756",
        "CLP" => "152",
        "CNY" => "156",
        "CZK" => "203",
        "DKK" => "208",
        "EUR" => "978",
        "GBP" => "826",
        "HKD" => "344",
        "HRK" => "191",
        "HUF" => "348",
        "IDR" => "360",
        "ILS" => "376",
        "INR" => "356",
        "ISK" => "352",
        "JPY" => "392",
        "KRW" => "410",
        "KWD" => "414",
        "KZT" => "398",
        "LAK" => "418",
        "LKR" => "144",
        "MAD" => "504",
        "MDL" => "498",
        "MMK" => "104",
        "MOP" => "446",
        "MXN" => "484",
        "MYR" => "458",
        "NOK" => "578",
        "NPR" => "524",
        "NZD" => "554",
        "OMR" => "512",
        "PHP" => "608",
        "PLN" => "985",
        "QAR" => "634",
        "RON" => "946",
        "RUB" => "643",
        "SAR" => "682",
        "SEK" => "752",
        "SGD" => "702",
        "THB" => "764",
        "TND" => "788",
        "TRY" => "949",
        "TWD" => "901",
        "UAH" => "980",
        "USD" => "840",
        "VND" => "704",
        "ZAR" => "710",
        _ => return None,
    })
}

/// Fold a free-text field into the EMV alphanumeric charset: Unicode
/// decomposition, combining marks dropped (accents stripped — "SÜRABAYA" →
/// "SURABAYA"), non-printable-ASCII removed, trimmed. Length is enforced by the
/// caller so a refusal can name the folded length, not the raw one.
pub fn fold_alphanum(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.trim().chars() {
        for decomposed in decompose(ch) {
            if !is_combining_mark(decomposed) && is_printable_ascii(decomposed) {
                out.push(decomposed);
            }
        }
    }
    out.trim().to_string()
}

fn decompose(ch: char) -> Vec<char> {
    // Canonical decomposition without pulling in a unicode-normalization crate:
    // cover the accent-fold table, pass everything else through.
    let folded: &[(&str, &str)] = &[
        ("À", "A"), ("Á", "A"), ("Â", "A"), ("Ã", "A"), ("Ä", "A"), ("Å", "A"),
        ("à", "a"), ("á", "a"), ("â", "a"), ("ã", "a"), ("ä", "a"), ("å", "a"),
        ("È", "E"), ("É", "E"), ("Ê", "E"), ("Ë", "E"),
        ("è", "e"), ("é", "e"), ("ê", "e"), ("ë", "e"),
        ("Ì", "I"), ("Í", "I"), ("Î", "I"), ("Ï", "I"),
        ("ì", "i"), ("í", "i"), ("î", "i"), ("ï", "i"),
        ("Ò", "O"), ("Ó", "O"), ("Ô", "O"), ("Õ", "O"), ("Ö", "O"),
        ("ò", "o"), ("ó", "o"), ("ô", "o"), ("õ", "o"), ("ö", "o"),
        ("Ù", "U"), ("Ú", "U"), ("Û", "U"), ("Ü", "U"),
        ("ù", "u"), ("ú", "u"), ("û", "u"), ("ü", "u"),
        ("Ý", "Y"), ("ý", "y"), ("ÿ", "y"),
        ("Ñ", "N"), ("ñ", "n"), ("Ç", "C"), ("ç", "c"),
        ("Đ", "D"), ("đ", "d"),
    ];
    let s = ch.to_string();
    for (marked, plain) in folded {
        if s == *marked {
            return plain.chars().collect();
        }
    }
    vec![ch]
}

fn is_combining_mark(ch: char) -> bool {
    // Unicode combining diacritical marks block (U+0300..U+036F).
    (0x300..=0x36F).contains(&(ch as u32))
}

fn is_printable_ascii(ch: char) -> bool {
    (0x20..=0x7E).contains(&(ch as u32))
}

/// One EMV TLV object: two-digit id, two-digit length, value.
fn tlv(id: &str, value: &str) -> String {
    format!("{id}{:02}{}", value.chars().count(), value)
}

/// CRC-16/CCITT-FALSE — poly 0x1021, init 0xFFFF, no input/output reflection,
/// no final xor. The QRIS checksum arm: `crc16(b"123456789") == 0x29B1` (the
/// published catalog check value for this parameter set) is pinned by test.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for byte in data {
        crc ^= (*byte as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// Format the transaction amount (tag 54): decimal with a "." separator, two
/// fractional digits, no grouping. (QRIS uses two decimals even for zero-decimal
/// currencies; the amount is advisory to the payer's app, not authoritative.)
fn format_amount(amount: Decimal) -> Result<String, EmvQrError> {
    if amount <= Decimal::ZERO {
        return Err(EmvQrError::InvalidAmount);
    }
    let s = format!("{:.2}", amount.normalize());
    if s.chars().count() > MAX_AMOUNT_LEN {
        return Err(EmvQrError::AmountTooLong);
    }
    Ok(s)
}

/// Build the merchant-presented QR payload.
///
/// `amount` of `None` renders a static QR without tag 54 (the payer's app asks
/// for the amount); `reference` (tag 62-01) carries the invoice number so the
/// settlement reconciliation can tie the scan to the document.
pub fn build_emv_merchant_payload(
    profile: &EmvMerchantProfile,
    currency_alpha: &str,
    amount: Option<Decimal>,
    reference: Option<&str>,
) -> Result<String, EmvQrError> {
    let name = fold_alphanum(&profile.merchant_name);
    if name.is_empty() || name.chars().count() > MAX_MERCHANT_NAME {
        return Err(EmvQrError::MerchantNameTooLong(name.chars().count()));
    }
    let city = fold_alphanum(&profile.merchant_city);
    if city.is_empty() || city.chars().count() > MAX_MERCHANT_CITY {
        return Err(EmvQrError::MerchantCityTooLong(city.chars().count()));
    }

    let country = profile.country.trim().to_ascii_uppercase();
    if country.chars().count() != 2 || !country.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(EmvQrError::InvalidCountry(profile.country.clone()));
    }

    let mcc = profile.mcc.trim().to_string();
    if mcc.chars().count() != 4 || !mcc.chars().all(|c| c.is_ascii_digit()) {
        return Err(EmvQrError::InvalidMcc(profile.mcc.clone()));
    }

    let initiation = profile.initiation.trim().to_string();
    if initiation != "11" && initiation != "12" {
        return Err(EmvQrError::InvalidInitiation(profile.initiation.clone()));
    }

    let gui = profile.gui.trim().to_string();
    let identifier = profile.merchant_identifier.trim().to_string();
    // Printable ASCII only: EMV TLV lengths are BYTE counts and this builder
    // emits `chars().count()`, so the two must coincide — a multibyte GUI or
    // merchant identifier would understate every subsequent length field. The
    // EMV alphanumeric charset excludes such data anyway; refusing is the
    // fail-closed arm.
    if gui.is_empty()
        || gui.chars().count() > 32
        || identifier.is_empty()
        || identifier.chars().count() > 32
        || !gui.chars().all(|c| (0x20..=0x7E).contains(&(c as u32)))
        || !identifier.chars().all(|c| (0x20..=0x7E).contains(&(c as u32)))
    {
        return Err(EmvQrError::InvalidMerchantAccount(
            "gui and merchant_identifier must each be 1..=32 printable-ASCII chars".into(),
        ));
    }

    let currency = currency_numeric(currency_alpha)
        .ok_or_else(|| EmvQrError::CurrencyNotSupported(currency_alpha.to_string()))?;

    let amount_str = match amount {
        Some(a) => Some(format_amount(a)?),
        None => None,
    };

    let reference_folded = reference.map(fold_alphanum).unwrap_or_default();
    if !reference_folded.is_empty() && reference_folded.chars().count() > MAX_REFERENCE {
        return Err(EmvQrError::ReferenceTooLong(reference_folded.chars().count()));
    }

    let mut payload = String::new();
    payload.push_str(&tlv("00", "01"));
    payload.push_str(&tlv("01", &initiation));
    payload.push_str(&tlv(
        "26",
        &format!("{}{}", tlv("00", &gui), tlv("01", &identifier)),
    ));
    payload.push_str(&tlv("52", &mcc));
    payload.push_str(&tlv("53", currency));
    if let Some(a) = &amount_str {
        payload.push_str(&tlv("54", a));
    }
    payload.push_str(&tlv("58", &country));
    payload.push_str(&tlv("59", &name));
    payload.push_str(&tlv("60", &city));
    if !reference_folded.is_empty() {
        payload.push_str(&tlv("62", &tlv("01", &reference_folded)));
    }

    // CRC arm: computed over the payload INCLUDING the "6304" tag-length header.
    payload.push_str("6304");
    let crc = crc16(payload.as_bytes());
    payload.push_str(&format!("{crc:04X}"));
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> EmvMerchantProfile {
        EmvMerchantProfile {
            merchant_name: "LAOPAY STORE".into(),
            merchant_city: "JAKARTA".into(),
            country: "ID".into(),
            mcc: "6012".into(),
            gui: "ID.CO.QRIS.WWW".into(),
            merchant_identifier: "9360012345678901".into(),
            initiation: "11".into(),
        }
    }

    fn dec(s: &str) -> Decimal {
        Decimal::from_str_exact(s).unwrap()
    }

    /// CRC catalog check value for CRC-16/CCITT-FALSE.
    #[test]
    fn crc16_catalog_check() {
        assert_eq!(crc16(b"123456789"), 0x29B1);
    }

    /// Full-payload vectors generated by an independent implementation (a
    /// throwaway CRC/TLV script run at authoring time), pinned here so the
    /// builder cannot drift from the wire format silently.
    #[test]
    fn payload_vectors() {
        // static, no amount, with invoice reference
        let v1 = build_emv_merchant_payload(
            &profile(),
            "IDR",
            None,
            Some("INV/2026/0001"),
        )
        .unwrap();
        assert_eq!(
            v1,
            "00020101021126380014ID.CO.QRIS.WWW011693600123456789015204601253033605802\
             ID5912LAOPAY STORE6007JAKARTA62170113INV/2026/00016304F7D4"
        );

        // UME merchant, with amount, no reference (tag 62 omitted entirely)
        let p2 = EmvMerchantProfile {
            merchant_name: "TOKO MAJU".into(),
            merchant_city: "BANDUNG".into(),
            mcc: "5999".into(),
            merchant_identifier: "UME".into(),
            ..profile()
        };
        let v2 = build_emv_merchant_payload(&p2, "IDR", Some(dec("150000.00")), None).unwrap();
        assert_eq!(
            v2,
            "00020101021126250014ID.CO.QRIS.WWW0103UME5204599953033605409150000.005802\
             ID5909TOKO MAJU6007BANDUNG630472EC"
        );

        // dynamic initiation, folded accents, larger amount
        let p3 = EmvMerchantProfile {
            merchant_name: "CV Séntosa Åbadi".into(),
            merchant_city: "SÜRABAYA".into(),
            mcc: "6010".into(),
            merchant_identifier: "1234567890123456".into(),
            initiation: "12".into(),
            ..profile()
        };
        let v3 = build_emv_merchant_payload(
            &p3,
            "IDR",
            Some(dec("9876543.21")),
            Some("REF-99881"),
        )
        .unwrap();
        assert_eq!(
            v3,
            "00020101021226380014ID.CO.QRIS.WWW011612345678901234565204601053033605410\
             9876543.215802ID5916CV Sentosa Abadi6008SURABAYA62130109REF-99881630455E8"
        );
    }

    #[test]
    fn typed_refusals() {
        let p = profile();

        assert_eq!(
            build_emv_merchant_payload(
                &EmvMerchantProfile {
                    merchant_name: "A".repeat(26),
                    ..p.clone()
                },
                "IDR", None, None
            )
            .unwrap_err()
            .code(),
            "merchant_name_too_long"
        );

        assert_eq!(
            build_emv_merchant_payload(
                &EmvMerchantProfile {
                    merchant_city: "A".repeat(16),
                    ..p.clone()
                },
                "IDR", None, None
            )
            .unwrap_err()
            .code(),
            "merchant_city_too_long"
        );

        assert_eq!(
            build_emv_merchant_payload(
                &EmvMerchantProfile { country: "IDN".into(), ..p.clone() },
                "IDR", None, None
            )
            .unwrap_err()
            .code(),
            "invalid_country_code"
        );

        assert_eq!(
            build_emv_merchant_payload(
                &EmvMerchantProfile { mcc: "601".into(), ..p.clone() },
                "IDR", None, None
            )
            .unwrap_err()
            .code(),
            "invalid_mcc"
        );

        assert_eq!(
            build_emv_merchant_payload(
                &EmvMerchantProfile { initiation: "13".into(), ..p.clone() },
                "IDR", None, None
            )
            .unwrap_err()
            .code(),
            "invalid_initiation_method"
        );

        // Multibyte merchant data refuses: EMV TLV lengths are byte counts and
        // the builder counts characters, so the charset is closed to printable
        // ASCII rather than silently emitting skewed lengths.
        assert_eq!(
            build_emv_merchant_payload(
                &EmvMerchantProfile {
                    merchant_identifier: "9360012345٦78901".into(),
                    ..p.clone()
                },
                "IDR", None, None
            )
            .unwrap_err()
            .code(),
            "invalid_merchant_account"
        );
        assert_eq!(
            build_emv_merchant_payload(
                &EmvMerchantProfile { gui: "ID.CO.QRIS.WWWÜ".into(), ..p.clone() },
                "IDR", None, None
            )
            .unwrap_err()
            .code(),
            "invalid_merchant_account"
        );

        assert_eq!(
            build_emv_merchant_payload(&p, "XYZ", None, None)
                .unwrap_err()
                .code(),
            "currency_not_supported"
        );

        assert_eq!(
            build_emv_merchant_payload(&p, "IDR", Some(Decimal::ZERO), None)
                .unwrap_err()
                .code(),
            "invalid_amount"
        );

        assert_eq!(
            build_emv_merchant_payload(&p, "IDR", Some(dec("123456789012.00")), None)
                .unwrap_err()
                .code(),
            "amount_too_long"
        );

        assert_eq!(
            build_emv_merchant_payload(&p, "IDR", None, Some(&"R".repeat(26)))
                .unwrap_err()
                .code(),
            "reference_too_long"
        );
    }

    #[test]
    fn folds_accents_and_trims() {
        assert_eq!(fold_alphanum("  SÜRABAYA "), "SURABAYA");
        assert_eq!(fold_alphanum("Jôgyakarta"), "Jogyakarta");
    }
}
