use super::{variables::fill_variables, PkgRequest};
use crate::{
    error,
    types::{parse_version_requirement, VersionRequirement},
};

use anyhow::{bail, Context, Result};
use console::style;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::*,
    error::{Error, ErrorKind, ParseError},
    sequence::*,
    IResult, InputTakeAtPosition,
};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BlueprintLine {
    PkgRequest(PkgRequest),
    Comment(String),
    EmptyLine,
}

pub fn read_blueprint_from_file(path: &Path) -> Result<Vec<BlueprintLine>> {
    // Read lines from blueprint file
    let f = File::open(path).context(format!("Failed to open Blueprint at {}", path.display()))?;
    let reader = BufReader::new(f);
    let mut lines = parse_blueprint_lines(reader)
        .context(format!("Failed to parse {}", style(path.display()).bold()))?;
    for (no, mut line) in lines.iter_mut().enumerate() {
        // Fill variables
        if let BlueprintLine::PkgRequest(req) = &mut line {
            let new_pkgname = fill_variables(&req.name)?;
            if new_pkgname.chars().all(is_pkgname_char) {
                // Only contains valid package names, we are good
                req.name = new_pkgname;
            } else {
                bail!(
                    "Fail to parse {}: invalid package name at line {}",
                    path.display(),
                    no
                );
            }
        }
    }

    Ok(lines)
}

fn parse_blueprint_lines(reader: impl BufRead) -> Result<Vec<BlueprintLine>> {
    let mut res = Vec::new();
    let mut errors = 0;
    for (no, line) in reader.lines().enumerate() {
        let i = line?;
        match alt((empty_line, comment_line, package_line_wrapper))(&i) {
            Ok((_, content)) => {
                res.push(content);
            }
            Err(e) => {
                errors += 1;
                error!(
                    "Failed to parse blueprint at line {}: {}\n",
                    no,
                    e.to_string()
                );
            }
        };
    }

    if errors == 0 {
        Ok(res)
    } else {
        bail!("Failed to parse blueprint due to {} error(s)", errors)
    }
}

fn empty_line(i: &str) -> IResult<&str, BlueprintLine> {
    match nom::sequence::terminated(nom::character::complete::space0, nom::combinator::eof)(i) {
        Ok(_) => Ok(("", BlueprintLine::EmptyLine)),
        Err(e) => Err(e),
    }
}

fn comment_line(i: &str) -> IResult<&str, BlueprintLine> {
    match char('#')(i) {
        Ok((r, _)) => Ok(("", BlueprintLine::Comment(r.to_string()))),
        Err(e) => Err(e),
    }
}

fn is_pkgname_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '.' || c == '+'
}

fn is_pkgname_with_var_char(c: char) -> bool {
    is_pkgname_char(c) || c == '{' || c == '_' || c == '}'
}

fn package_name(i: &str) -> IResult<&str, &str> {
    i.split_at_position1_complete(|item| !is_pkgname_with_var_char(item), ErrorKind::Char)
}

enum PkgOption {
    VersionRequirement(VersionRequirement),
    AddedBy(String),
    Local,
}

fn pkg_option(i: &str) -> IResult<&str, PkgOption> {
    if let Ok((i, _)) = tag::<_, _, Error<&str>>("added_by")(i) {
        let (i, _) = space0(i)?;
        let (i, _) = char('=')(i)?;
        let (i, _) = space0(i)?;
        let (i, pkgname) = package_name(i)?;
        return Ok((i, PkgOption::AddedBy(pkgname.to_owned())));
    }

    if let Ok((i, req)) = parse_version_requirement(i) {
        return Ok((i, PkgOption::VersionRequirement(req)));
    }

    if let Ok((i, _)) = tag::<_, _, Error<&str>>("local")(i) {
        return Ok((i, PkgOption::Local));
    }

    Err(nom::Err::Error(nom::error::Error::from_error_kind(
        i,
        ErrorKind::Alt,
    )))
}

fn package_line(i: &str) -> IResult<&str, PkgRequest> {
    let (i, name) = package_name(i)?;
    let (i, _) = nom::character::complete::space0(i)?;
    // Construct basic result
    let mut res = PkgRequest {
        name: name.to_string(),
        version: VersionRequirement::default(),
        added_by: None,
        local: false,
    };

    let i = if let Ok((i, opts)) = nom::sequence::delimited(
        tuple((space0, char('('), space0)),
        nom::multi::separated_list1(tuple((space0, char(','), space0)), pkg_option),
        tuple((space0, char(')'), space0)),
    )(i)
    {
        // Enroll optional requests
        for opt in opts {
            match opt {
                PkgOption::AddedBy(pkgname) => res.added_by = Some(pkgname),
                PkgOption::VersionRequirement(request) => {
                    res.version = res.version.combine(&request).unwrap();
                }
                PkgOption::Local => {
                    res.local = true;
                }
            }
        }
        i
    } else {
        i
    };

    let (i, _) = nom::combinator::eof(i)?;

    Ok((i, res))
}

fn package_line_wrapper(i: &str) -> IResult<&str, BlueprintLine> {
    let (i, res) = package_line(i)?;
    Ok((i, BlueprintLine::PkgRequest(res)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PkgVersion;
    use nom::{error::Error, IResult};
    use std::convert::TryFrom;
    #[test]
    fn test_empty_line() {
        let t: Vec<(&str, IResult<&str, BlueprintLine>)> = vec![
            ("", Ok(("", BlueprintLine::EmptyLine))),
            ("   ", Ok(("", BlueprintLine::EmptyLine))),
            (
                "blah",
                Err(nom::Err::Error(Error::new(
                    "blah",
                    nom::error::ErrorKind::Eof,
                ))),
            ),
            (
                "   nope",
                Err(nom::Err::Error(Error::new(
                    "nope",
                    nom::error::ErrorKind::Eof,
                ))),
            ),
        ];

        for test in t {
            assert_eq!(empty_line(test.0), test.1);
        }
    }

    #[test]
    fn test_comment_line() {
        let t: Vec<(&str, IResult<&str, BlueprintLine>)> = vec![
            ("#", Ok(("", BlueprintLine::Comment("".to_string())))),
            ("#   ", Ok(("", BlueprintLine::Comment("   ".to_string())))),
            (
                "# This is a comment",
                Ok(("", BlueprintLine::Comment(" This is a comment".to_string()))),
            ),
            (
                "blah",
                Err(nom::Err::Error(Error::new(
                    "blah",
                    nom::error::ErrorKind::Char,
                ))),
            ),
            (
                "   nope",
                Err(nom::Err::Error(Error::new(
                    "   nope",
                    nom::error::ErrorKind::Char,
                ))),
            ),
        ];

        for test in t {
            assert_eq!(comment_line(test.0), test.1);
        }
    }

    #[test]
    fn test_pkgname() {
        let t: Vec<(&str, IResult<&str, &str>)> = vec![
            ("a1-v2", Ok(("", "a1-v2"))),
            ("a.+b", Ok(("", "a.+b"))),
            ("a~b", Ok(("~b", "a"))), // The letters after ~ will not be consumed
        ];

        for test in t {
            assert_eq!(package_name(test.0), test.1);
        }
    }

    #[test]
    fn test_package_line() {
        let tests = vec![(
            "abc (added_by = wow, >>1)",
            PkgRequest {
                name: "abc".to_string(),
                version: VersionRequirement {
                    lower_bond: Some((PkgVersion::try_from("1").unwrap(), false)),
                    upper_bond: None,
                },
                added_by: Some("wow".to_string()),
                local: false,
            },
            (
                "pkgname (>>1, local, <<2)",
                PkgRequest {
                    name: "abc".to_string(),
                    version: VersionRequirement {
                        lower_bond: Some((PkgVersion::try_from("1").unwrap(), false)),
                        upper_bond: Some((PkgVersion::try_from("2").unwrap(), true)),
                    },
                    added_by: None,
                    local: true,
                },
            ),
        )];

        for t in tests {
            assert_eq!(package_line(t.0).unwrap().1, t.1);
        }
    }
}
