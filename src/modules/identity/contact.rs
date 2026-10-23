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
/// Parse one bounded contact hint without opening sockets or performing discovery.
/// # Errors
/// Rejects invalid ID/query/IP/port spelling and excessive text before decoding.
pub fn parse(input: &str, maximum_bytes: usize) -> Result<Contact, IdentityError> {
    tracing::trace!("Parsing a bounded contact hint");
    if input.len() > maximum_bytes {
        return Err(IdentityError::ResourceLimit);
    }
    if input.contains('#') {
        return Err(IdentityError::InvalidInput);
    }
    let (id, query) = match input.split_once('?') {
        Some((id, query)) => (id, Some(query)),
        None => (input, None),
    };
    let expected_id = text::parse(id)?;
    let Some(query) = query else {
        return Ok(Contact {
            expected_id,
            endpoint: None,
        });
    };
    let mut address = None;
    let mut port = None;
    for field in query.split('&') {
        let (name, value) = field.split_once('=').ok_or(IdentityError::InvalidInput)?;
        match name {
            "address" if address.is_none() => {
                address = Some(decode_address(value)?);
            }
            "port" if port.is_none() => {
                if value.is_empty()
                    || value.starts_with('0')
                    || !value.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err(IdentityError::InvalidInput);
                }
                port = Some(
                    value
                        .parse::<u16>()
                        .map_err(|_| IdentityError::InvalidInput)?,
                );
            }
            _ => return Err(IdentityError::InvalidInput),
        }
    }
    let endpoint = std::net::SocketAddr::new(
        address.ok_or(IdentityError::InvalidInput)?,
        port.ok_or(IdentityError::InvalidInput)?,
    );
    Ok(Contact {
        expected_id,
        endpoint: Some(endpoint),
    })
}
// Decode once, requiring reserved characters to be percent escaped, then validate an IP literal.
fn decode_address(input: &str) -> Result<std::net::IpAddr, IdentityError> {
    tracing::trace!("Decoding a contact IP literal once");
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'%' {
            let pair = bytes
                .get(index + 1..index + 3)
                .ok_or(IdentityError::InvalidInput)?;
            let hi = hex_digit(pair[0])?;
            let lo = hex_digit(pair[1])?;
            decoded.push((hi << 4) | lo);
            index += 3;
        } else {
            if !byte.is_ascii_alphanumeric() && !b"-._~".contains(&byte) {
                return Err(IdentityError::InvalidInput);
            }
            decoded.push(byte);
            index += 1;
        }
    }
    std::str::from_utf8(&decoded)
        .map_err(|_| IdentityError::InvalidInput)?
        .parse()
        .map_err(|_| IdentityError::InvalidInput)
}
// Interpret ASCII hexadecimal only; malformed percent escape bytes are rejected.
fn hex_digit(byte: u8) -> Result<u8, IdentityError> {
    tracing::trace!("Validating a percent-escape digit");
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(IdentityError::InvalidInput),
    }
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
    // Contact inputs preserve their expected ID and reject invalid fields before returning a hint.
    #[test]
    fn query_validation_and_one_time_decoding() {
        let id = IdentityId([7; 32]);
        let prefix = text::format(id);
        assert_eq!(
            parse(&prefix, 58).unwrap(),
            Contact {
                expected_id: id,
                endpoint: None
            }
        );
        for (query, endpoint) in [
            ("address=127.0.0.1&port=1", "127.0.0.1:1"),
            (
                "port=65535&address=2001%3Adb8%3A%3A1",
                "[2001:db8::1]:65535",
            ),
            ("address=%3a%3a1&port=2", "[::1]:2"),
        ] {
            let contact = parse(&format!("{prefix}?{query}"), 256).unwrap();
            assert_eq!(contact.expected_id, id);
            assert_eq!(
                contact.endpoint.unwrap(),
                endpoint.parse::<std::net::SocketAddr>().unwrap()
            );
            assert_eq!(parse(&format(contact).unwrap(), 256).unwrap(), contact);
        }
        for query in [
            "",
            "address=127.0.0.1",
            "port=1",
            "address=127.0.0.1&port=0",
            "address=127.0.0.1&port=65536",
            "address=127.0.0.1&port=+1",
            "address=127.0.0.1&port=01",
            "address=127.0.0.1&port=",
            "address=127.0.0.1&port=1&port=2",
            "address=127.0.0.1&address=127.0.0.2&port=1",
            "address=127.0.0.1&port=1&other=x",
            "address=example.com&port=1",
            "address=[::1]&port=1",
            "address=::1&port=1",
            "address=fe80%3A%3A1%25eth0&port=1",
            "address=%253A%253A1&port=1",
            "address=%&port=1",
            "address=%GG&port=1",
            "address=%FF&port=1",
            "address=%3A%3A1&port=1#fragment",
            "address=127.0.0.1&port=1&",
        ] {
            assert!(parse(&format!("{prefix}?{query}"), 256).is_err(), "{query}");
        }
        assert_eq!(parse(&prefix, 57), Err(IdentityError::ResourceLimit));
        assert!(parse("bad", 256).is_err());
    }
}
