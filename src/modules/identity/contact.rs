//! Contact QR text codecs; endpoint hints never establish reachability or identity possession.
use super::{api::Contact, error::IdentityError, text};

/// Format ID-only or IP/port contact payloads with canonical text and percent-escaped colons.
/// This produces text for a QR encoder, not an image and not a network connection.
/// # Errors
/// Rejects port zero and IPv6 scope/flow metadata, which this contact format cannot carry.
pub fn format(contact: Contact) -> Result<String, IdentityError> {
    tracing::trace!("Formatting a public contact QR payload");
    let mut output = text::format(contact.expected_id);
    let Some(endpoint) = contact.endpoint else {
        return Ok(output);
    };
    if endpoint.port() == 0 {
        return Err(IdentityError::InvalidInput);
    }
    if let std::net::SocketAddr::V6(v6) = endpoint
        && (v6.scope_id() != 0 || v6.flowinfo() != 0)
    {
        return Err(IdentityError::InvalidInput);
    }
    output.push_str("?address=");
    output.push_str(&endpoint.ip().to_string().replace(':', "%3A"));
    output.push_str("&port=");
    output.push_str(&endpoint.port().to_string());
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::identity::api::IdentityId;
    // Public QR payloads preserve the expected ID and escape IPv6 colons with no brackets or zones.
    #[test]
    fn exact_contact_payloads_and_endpoint_boundaries() {
        let id = IdentityId([0; 32]);
        let prefix = text::format(id);
        assert_eq!(
            format(Contact {
                expected_id: id,
                endpoint: None
            })
            .unwrap(),
            prefix
        );
        for (address, expected) in [
            ("127.0.0.1:1", "127.0.0.1&port=1"),
            ("[2001:db8::1]:65535", "2001%3Adb8%3A%3A1&port=65535"),
        ] {
            assert_eq!(
                format(Contact {
                    expected_id: id,
                    endpoint: Some(address.parse().unwrap())
                })
                .unwrap(),
                format!("{prefix}?address={expected}")
            );
        }
        for endpoint in [
            "127.0.0.1:0".parse().unwrap(),
            std::net::SocketAddrV6::new(std::net::Ipv6Addr::LOCALHOST, 1, 0, 1).into(),
            std::net::SocketAddrV6::new(std::net::Ipv6Addr::LOCALHOST, 1, 1, 0).into(),
        ] {
            assert_eq!(
                format(Contact {
                    expected_id: id,
                    endpoint: Some(endpoint)
                }),
                Err(IdentityError::InvalidInput)
            );
        }
    }
}
