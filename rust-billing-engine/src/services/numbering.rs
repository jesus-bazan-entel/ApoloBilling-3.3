// src/services/numbering.rs
//
// Number normalisation for CDRs. `caller_number` / `called_number` keep what
// was exchanged on the wire (percent-decoded) so they can be reconciled with
// the carrier's own CDRs; the routed destination is also stored in E.164 in
// `cdrs.destination_prefix` (the API's `destination`), and the customer that
// owns either end is resolved from `sip_devices.sip_username`.

/// Number the call was actually delivered to: FreeSWITCH's
/// `transformed_number` (set by the inbound route's strip/prefix in
/// from-kamailio.xml) or, when absent, the dialed number. Integratel DIDs
/// arrive as `31` + 2-digit routing code + 8-digit national number
/// (`312217473046`); the "DID 3122" route strips those 4 digits, and the same
/// rule is applied here for CDRs that never went through that dialplan
/// (Kamailio direct routing).
pub fn routed_number(callee: &str, transformed: Option<&str>) -> String {
    if let Some(t) = transformed.map(str::trim).filter(|t| !t.is_empty()) {
        return t.to_string();
    }
    let digits_only = callee.bytes().all(|b| b.is_ascii_digit());
    if digits_only && callee.len() == 12 && callee.starts_with("31") {
        return callee[4..].to_string();
    }
    callee.to_string()
}

/// E.164 form of a Peruvian (or already international) number, or None when
/// the value is not a recognisable phone number (extensions, short codes,
/// garbage such as `DD30D984541863`).
///
/// National plan (Peru, +51): mobiles are 9 digits starting with 9; fixed
/// lines are 8 digits, Lima `1` + 7 digits and provinces a 2-digit area code
/// (4x-8x) + 6 digits; `0` is the national trunk prefix and `00` the
/// international one.
pub fn to_e164(number: &str) -> Option<String> {
    let n = number.trim();
    let (plus, digits) = match n.strip_prefix('+') {
        Some(rest) => (true, rest),
        None => (false, n),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let international = |d: &str| (8..=15).contains(&d.len()).then(|| format!("+{}", d));

    if plus {
        return international(digits);
    }
    if let Some(rest) = digits.strip_prefix("00") {
        return international(rest);
    }
    // Country code without '+'. An 8-digit Puno number (area 51) leaves only
    // 6 digits after it, so it falls through to the national rule below.
    if let Some(national) = digits.strip_prefix("51").filter(|r| is_pe_national(r)) {
        return Some(format!("+51{}", national));
    }
    if let Some(rest) = digits.strip_prefix('0').filter(|r| is_pe_fixed(r)) {
        return Some(format!("+51{}", rest));
    }
    if is_pe_national(digits) {
        return Some(format!("+51{}", digits));
    }
    None
}

/// National significant number of a Peruvian E.164 (`+5117473046` →
/// `17473046`), the form `sip_devices.sip_username` uses.
pub fn pe_national(e164: &str) -> Option<&str> {
    e164.strip_prefix("+51").filter(|n| is_pe_national(n))
}

/// `sip_devices.sip_username` values that identify the SBC customer of a
/// call: the routed destination on inbound calls, the caller on outbound ones
/// (raw and national forms, since devices register with the national number).
pub fn customer_keys(caller: &str, routed: &str, direction: &str) -> Vec<String> {
    let party = if direction.eq_ignore_ascii_case("outbound") {
        caller
    } else {
        routed
    };
    let mut keys = vec![party.to_string()];
    if let Some(national) = to_e164(party).as_deref().and_then(pe_national) {
        if national != party {
            keys.push(national.to_string());
        }
    }
    keys
}

fn is_pe_mobile(d: &str) -> bool {
    d.len() == 9 && d.starts_with('9')
}

fn is_pe_fixed(d: &str) -> bool {
    d.len() == 8 && matches!(d.as_bytes()[0], b'1' | b'4'..=b'8')
}

fn is_pe_national(d: &str) -> bool {
    d.bytes().all(|b| b.is_ascii_digit()) && (is_pe_mobile(d) || is_pe_fixed(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routed_number_prefers_transformed() {
        assert_eq!(routed_number("312217473046", Some("17473046")), "17473046");
        assert_eq!(routed_number("312217473046", Some("")), "17473046");
        assert_eq!(routed_number("315717473046", None), "17473046");
        assert_eq!(routed_number("993123456", None), "993123456");
        assert_eq!(routed_number("1001", None), "1001");
    }

    #[test]
    fn e164_peru() {
        assert_eq!(to_e164("17473046").as_deref(), Some("+5117473046"));
        assert_eq!(to_e164("993123456").as_deref(), Some("+51993123456"));
        assert_eq!(to_e164("44123456").as_deref(), Some("+5144123456"));
        assert_eq!(to_e164("51234567").as_deref(), Some("+5151234567"));
        assert_eq!(to_e164("017473046").as_deref(), Some("+5117473046"));
        assert_eq!(to_e164("51993123456").as_deref(), Some("+51993123456"));
        assert_eq!(to_e164("5117473046").as_deref(), Some("+5117473046"));
        assert_eq!(to_e164("+18505551234").as_deref(), Some("+18505551234"));
        assert_eq!(to_e164("0018505551234").as_deref(), Some("+18505551234"));
    }

    #[test]
    fn e164_rejects_non_numbers() {
        assert_eq!(to_e164("DD30D984541863"), None);
        assert_eq!(to_e164("1001"), None);
        assert_eq!(to_e164("123"), None);
        assert_eq!(to_e164("312217473046"), None);
        assert_eq!(to_e164("23123456"), None);
        assert_eq!(to_e164(""), None);
        assert_eq!(to_e164("+12"), None);
    }

    #[test]
    fn customer_keys_by_direction() {
        assert_eq!(customer_keys("+12035551234", "17473046", "inbound"), ["17473046"]);
        assert_eq!(customer_keys("017473046", "993123456", "outbound"), ["017473046", "17473046"]);
        assert_eq!(customer_keys("1001", "993123456", "OUTBOUND"), ["1001"]);
    }

    #[test]
    fn national_of_e164() {
        assert_eq!(pe_national("+5117473046"), Some("17473046"));
        assert_eq!(pe_national("+51993123456"), Some("993123456"));
        assert_eq!(pe_national("+18505551234"), None);
    }
}
