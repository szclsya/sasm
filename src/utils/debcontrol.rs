use crate::types::VersionRequirement;

use anyhow::{format_err, Result};
use nom::{
    bytes::complete::{take_while, take_while1},
    character::{complete::alphanumeric1, complete::char, is_alphanumeric},
    combinator::{opt, recognize},
    sequence::{delimited, separated_pair},
    sequence::{pair, preceded},
    IResult,
};

// parser combinators
fn parse_package_name(s: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(pair(
        alphanumeric1,
        take_while(|c| is_alphanumeric(c) || c == b'+' || c == b'-' || c == b'.'),
    ))(s)
}

fn parse_version_op(s: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while1(|c| c == b'>' || c == b'<' || c == b'=')(s)
}

fn parse_version(s: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while1(|c| {
        is_alphanumeric(c) || c == b'+' || c == b'-' || c == b'.' || c == b'~' || c == b':'
    })(s)
}

fn parse_version_expr(s: &[u8]) -> IResult<&[u8], &[u8]> {
    recognize(separated_pair(parse_version_op, char(' '), parse_version))(s)
}

fn parse_relation_suffix(s: &[u8]) -> IResult<&[u8], &[u8]> {
    preceded(
        char(' '),
        delimited(char('('), parse_version_expr, char(')')),
    )(s)
}

fn parse_relational(s: &[u8]) -> IResult<&[u8], (&[u8], Option<&[u8]>)> {
    pair(parse_package_name, opt(parse_relation_suffix))(s)
}

pub fn parse_pkg_list(s: &str) -> Result<Vec<(String, VersionRequirement)>> {
    if s.is_empty() {
        return Ok(Vec::new());
    }

    let mut res = Vec::new();
    let pkgs: Vec<&str> = s.split(", ").collect();
    for pkg in pkgs {
        let (_, (name, version)) = parse_relational(pkg.as_bytes())
            .map_err(|_| format_err!("Malformed version in depends/breaks: {}", pkg))?;
        // The regex should ensure name always exist
        let ver_req = match version {
            Some(s) => VersionRequirement::try_from(std::str::from_utf8(s)?)?,
            None => VersionRequirement::new(),
        };
        // Add to result
        res.push((std::str::from_utf8(name)?.to_string(), ver_req));
    }

    Ok(res)
}

#[test]
fn test_parsers() {
    assert_eq!(parse_version_op(&b">>"[..]), Ok((&b""[..], &b">>"[..])));
    assert_eq!(parse_version_op(&b"<="[..]), Ok((&b""[..], &b"<="[..])));
    assert_eq!(
        parse_version_expr(&b">= 2:1.1.0~rc.1"[..]),
        Ok((&b""[..], &b">= 2:1.1.0~rc.1"[..]))
    );
    assert_eq!(
        parse_package_name(&b"sqlite-ass"[..]),
        Ok((&b""[..], &b"sqlite-ass"[..]))
    );
    assert_eq!(
        parse_package_name(&b"sqlite_ass"[..]),
        Ok((&b"_ass"[..], &b"sqlite"[..]))
    );
    assert_eq!(
        parse_relational(&b"libpcap (>= 1.9.1)"[..]),
        Ok((&b""[..], (&b"libpcap"[..], Some(&b">= 1.9.1"[..]))))
    );
    assert_eq!(
        parse_relational(&b"libpcap"[..]),
        Ok((&b""[..], (&b"libpcap"[..], None)))
    );
    assert!(parse_relational(&b"libpcap_invalid (>= 1.9.1)"[..]).is_err());
}
