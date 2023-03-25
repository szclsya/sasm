use crate::debug;
use anyhow::{bail, Result};
/// Parse pacman style package database files
use nom::{
    bytes::complete::{take_till, take_until1, take_while1},
    character::complete::anychar,
    character::complete::{alphanumeric1, char, space0},
    combinator::eof,
    multi::many1,
    IResult,
};
use std::collections::HashMap;

use crate::types::{parse_version_requirement, VersionRequirement};

/// Parse the key part of a paragraph, like `%NAME%`
fn parse_key(i: &str) -> IResult<&str, &str> {
    let (i, _) = char('%')(i)?;
    let (i, key) = alphanumeric1(i)?;
    let (i, _) = char('%')(i)?;
    // There should be a newline after the key line
    let (i, _) = char('\n')(i)?;

    Ok((i, key))
}

/// Parse the value part of a paragraph that ends with an empty line
fn parse_value_with_empty_line(i: &str) -> IResult<&str, &str> {
    // It should be multiple lines until an empty line
    let (i, content) = take_until1("\n\n")(i)?;
    // Eat the new lines
    let (i, _) = char('\n')(i)?;
    let (i, _) = char('\n')(i)?;

    Ok((i, content))
}

/// Parse the value part of a paragraph that ends with EOF
fn parse_value(mut i: &str) -> IResult<&str, Vec<String>> {
    let mut lines = Vec::new();
    loop {
        let (x, content) = take_till(|c| c == '\n')(i)?;
        let (x, _) = char('\n')(x)?;
        i = x;
        if content.is_empty() {
            break;
        }
        lines.push(content.to_owned());
        if x.is_empty() {
            break;
        }
    }
    Ok((i, lines))
}

/// Parse a key-value pair in pacman's package description syntax
fn parse_pair(i: &str) -> IResult<&str, (String, Vec<String>)> {
    let (i, key) = parse_key(i)?;
    let (i, lines) = parse_value(i)?;

    Ok((i, (key.to_owned(), lines)))
}

pub fn parse_str(mut i: &str) -> anyhow::Result<HashMap<String, Vec<String>>> {
    let mut res = HashMap::new();
    let mut counter = 0;
    while !i.is_empty() {
        match parse_pair(i) {
            Ok((x, pair)) => {
                res.insert(pair.0, pair.1);
                counter += 1;
                i = x;
            }
            Err(e) => {
                bail!("bad pacman database on paragraph {counter}: {e}");
            }
        }
    }
    Ok(res)
}

fn is_package_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '@' || c == '.' || c == '+' || c == '-' || c == '_'
}

fn is_version_requirement(i: &str) -> bool {
    i.starts_with(">") || i.starts_with("<") || i.starts_with("=")
}

pub fn parse_package_requirement_line(
    i: &str,
) -> IResult<&str, (&str, VersionRequirement, Option<String>)> {
    // First parse the package name
    let (i, name) = take_while1(is_package_name_char)(i)?;
    // Then the version requirement
    if is_version_requirement(i) {
        let (i, ver_req) = parse_version_requirement(i)?;
        if i.is_empty() {
            return Ok((i, (name, ver_req, None)));
        }
        let (i, desc) = parse_requirement_description(i)?;
        let (i, _) = eof(i)?;
        Ok((i, (name, ver_req, Some(desc))))
    } else if i.starts_with(":") {
        let (i, desc) = parse_requirement_description(i)?;
        let (i, _) = eof(i)?;
        Ok((i, (name, VersionRequirement::new(), Some(desc))))
    } else {
        let (i, _) = eof(i)?;
        Ok((i, (name, VersionRequirement::new(), None)))
    }
}

fn parse_requirement_description(i: &str) -> IResult<&str, String> {
    let (i, _) = char(':')(i)?;
    let (i, _) = space0(i)?;
    let (i, desc) = many1(anychar)(i)?;
    let (i, _) = eof(i)?;

    let desc_str: String = desc.into_iter().collect();
    Ok((i, desc_str))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn try_parse() {
        assert_eq!(("", "BRUH"), parse_key("%BRUH%\n").unwrap());
        assert_eq!(
            (
                "something else",
                (
                    "NAME".to_string(),
                    vec![
                        "A multiple".to_string(),
                        "line".to_string(),
                        "paragraph.".to_string()
                    ]
                )
            ),
            parse_pair(
                "%NAME%
A multiple
line
paragraph.

something else"
            )
            .unwrap()
        );
        assert_eq!(
            (
                "",
                (
                    "NAME".to_string(),
                    vec![
                        "A multiple".to_string(),
                        "line".to_string(),
                        "paragraph.".to_string()
                    ],
                )
            ),
            parse_pair(
                "%NAME%
A multiple
line
paragraph.
"
            )
            .unwrap()
        );
    }
}
