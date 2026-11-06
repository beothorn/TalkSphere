//! Independent portable-claim literal and malformed encoding cases.
use super::*;
// Python RFC4648 oracle generated this fixed105-byte layout, independently of the production codec.
#[test]
fn portable_literal() {
    let literal = "cr2se-claim:AQMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQAAAAAAAAAH";
    let claim = Portable::parse(literal).unwrap();
    assert_eq!(claim.issuer(), IdentityId([3; 32]));
    assert_eq!(claim.id(), [4; 32]);
    assert_eq!(claim.amount(), 7);
    assert_eq!(claim.secret.as_ref(), &[5; 32]);
    assert_eq!(claim.export().as_str(), literal);
    for invalid in [
        "",
        &literal[..literal.len() - 1],
        &format!("{literal}="),
        &literal.replace("cr2se-claim:", "CR2SE-CLAIM:"),
    ] {
        assert!(Portable::parse(invalid).is_err());
    }
    let mut bad = literal.as_bytes().to_vec();
    bad[12] = b'!';
    assert!(Portable::parse(std::str::from_utf8(&bad).unwrap()).is_err());
    bad = literal.as_bytes().to_vec();
    bad[12] = b'B';
    assert!(Portable::parse(std::str::from_utf8(&bad).unwrap()).is_err());
    let zero = Portable { amount: 0, ..claim };
    assert!(Portable::parse(&zero.export()).is_err());
}
