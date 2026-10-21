//! Shared helpers for writing dSIPRouter's `dr_gateways` table from Kamailio syncs.

/// `dr_gateways.type` of an external carrier/SIP provider, matches `FLT_CARRIER` in kamailio.cfg
pub const GW_TYPE_CARRIER: i32 = 8;

/// `dr_gateways.type` of an internal PBX endpoint, matches `FLT_PBX` in kamailio.cfg
pub const GW_TYPE_PBX: i32 = 9;

/// Map Apolo's `transport` column ("udp"/"tcp"/"tls") to the signalling value understood by
/// `SET_DST_SIGNALLING` in kamailio.cfg. Anything else, NULL included, falls back to "proxy",
/// which leaves the destination URI built by drouting untouched.
pub fn signalling_from_transport(transport: Option<&str>) -> &'static str {
    match transport.unwrap_or("").to_ascii_lowercase().as_str() {
        "udp" => "sip_udp",
        "tcp" => "sip_tcp",
        "tls" => "sips_tls",
        _ => "proxy",
    }
}

/// Build the `dr_gateways.address` column, which must stay scheme-less because `do_routing`
/// prepends "sip:" itself: storing "sip:host:port" yields an R-URI of "sip:number@sip:host:port"
/// that Kamailio cannot parse, leaving `$rz`/`$rd`/`$rp` null in `OUTBOUND_DROUTING` so the
/// outbound To/From headers are built as "<null>:number@<null>:<null>" and the INVITE is dropped.
/// Only the `dispatcher` table takes a scheme, and Apolo never writes it.
pub fn gateway_address(host: &str, port: Option<i32>) -> String {
    format!("{}:{}", host, port.unwrap_or(5060))
}

/// Build the positional `attrs` column of a `dr_gateways` row, which drouting publishes as
/// `$avp(dr_attrs)` and `SET_CALLDST_INFO` in kamailio.cfg reads as
/// "gwid,gwtype,msteams_domain,signalling,media". A shorter value makes Kamailio log
/// "invalid signalling/media configuration" on every call to that gateway. Empty msteams_domain
/// and "proxy" media are the dSIPRouter defaults, the latter meaning rtpengine stays in the path.
pub fn gateway_attrs(gwid: i32, gw_type: i32, signalling: &str) -> String {
    format!("{},{},,{},proxy", gwid, gw_type, signalling)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attrs_matches_dsiprouter_build_attrs() {
        // dSIPRouter: ','.join([gwid, type, msteams_domain, signalling, media])
        assert_eq!(
            gateway_attrs(60, GW_TYPE_CARRIER, "sip_udp"),
            "60,8,,sip_udp,proxy"
        );
        assert_eq!(
            gateway_attrs(103, GW_TYPE_PBX, "proxy"),
            "103,9,,proxy,proxy"
        );
        // SET_CALLDST_INFO reads signalling at index 3 and media at index 4
        assert_eq!(
            gateway_attrs(1, GW_TYPE_CARRIER, "sip_tcp")
                .split(',')
                .count(),
            5
        );
    }

    #[test]
    fn address_omits_the_scheme_drouting_adds() {
        assert_eq!(
            gateway_address("10.35.13.100", Some(5060)),
            "10.35.13.100:5060"
        );
        assert_eq!(gateway_address("pbx.local", None), "pbx.local:5060");
    }

    #[test]
    fn transport_falls_back_to_proxy() {
        assert_eq!(signalling_from_transport(Some("udp")), "sip_udp");
        assert_eq!(signalling_from_transport(Some("TCP")), "sip_tcp");
        assert_eq!(signalling_from_transport(Some("tls")), "sips_tls");
        assert_eq!(signalling_from_transport(Some("sctp")), "proxy");
        assert_eq!(signalling_from_transport(None), "proxy");
    }
}
