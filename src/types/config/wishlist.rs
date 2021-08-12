use crate::error;
use crate::types::{parse_version_requirement, VersionRequirement};

use anyhow::{bail, Result};
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

#[derive(Default)]
pub struct Wishlist {
    lines: Vec<WishlistLine>,
}

impl Wishlist {
    pub fn from_file(path: &Path) -> Result<Self> {
        let mut res = Wishlist::default();
        let f = File::open(path)?;
        let reader = BufReader::new(f);

        for line in parse_wishlist_lines(reader)? {
            if let WishlistLine::PkgRequest(req) = &line {
                if res.contains(&req.name) {
                    // Already contains this package, no good!
                    bail!("Duplicate package {} in wishlist", req.name)
                }
            }
            res.lines.push(line);
        }

        Ok(res)
    }

    pub fn get_pkg_requests(&self) -> Vec<&PkgRequest> {
        self.lines
            .iter()
            .filter_map(|line| match line {
                WishlistLine::PkgRequest(req) => Some(req),
                _ => None,
            })
            .collect()
    }

    pub fn contains(&self, pkgname: &str) -> bool {
        for line in &self.lines {
            if let WishlistLine::PkgRequest(req) = line {
                if req.name == pkgname {
                    return true;
                }
            }
        }
        false
    }

    pub fn add(&mut self, pkgname: &str) -> Result<()> {
        if self.contains(pkgname) {
            bail!("Package {} already exists in wishlist", pkgname);
        }

        let pkgreq = PkgRequest {
            name: pkgname.to_string(),
            version: VersionRequirement::default(),
            install_recomm: None,
        };
        self.lines.push(WishlistLine::PkgRequest(pkgreq));
        Ok(())
    }

    pub fn remove(&mut self, pkgname: &str) -> Result<()> {
        let mut i = 0;
        while i < self.lines.len() {
            if let WishlistLine::PkgRequest(req) = &self.lines[i] {
                if req.name == pkgname {
                    self.lines.remove(i);
                    return Ok(());
                }
            }
            i += 1;
        }

        bail!("Package with name {} not found in wishlist", pkgname)
    }

    pub fn export(&self) -> String {
        let mut res = String::new();
        for l in &self.lines {
            match l {
                WishlistLine::Comment(content) => res.push_str(&format!("#{}\n", content)),
                WishlistLine::EmptyLine => res.push('\n'),
                WishlistLine::PkgRequest(req) => res.push_str(&format!("{}\n", req.to_string())),
            }
        }
        res
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum WishlistLine {
    PkgRequest(PkgRequest),
    Comment(String),
    EmptyLine,
}

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub struct PkgRequest {
    pub name: String,
    pub version: VersionRequirement,
    pub install_recomm: Option<bool>,
}

impl std::fmt::Display for PkgRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.name)?;
        let ver_req_str = self.version.to_string();
        let recomm_str = match self.install_recomm {
            Some(recomm) => {
                if recomm {
                    "recomm".to_string()
                } else {
                    "no_recomm".to_string()
                }
            }
            None => String::new(),
        };

        if !ver_req_str.is_empty() || !recomm_str.is_empty() {
            write!(f, "({}", ver_req_str)?;
            if !recomm_str.is_empty() {
                write!(f, ", {}", recomm_str)?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

fn parse_wishlist_lines(reader: impl BufRead) -> Result<Vec<WishlistLine>> {
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
                    "Failed to parse wishlist at line {}: {}\n",
                    no,
                    e.to_string()
                );
            }
        };
    }

    if errors == 0 {
        Ok(res)
    } else {
        bail!("Failed to parse wishlist due to {} error(s)", errors)
    }
}

fn empty_line(i: &str) -> IResult<&str, WishlistLine> {
    match nom::sequence::terminated(nom::character::complete::space0, nom::combinator::eof)(i) {
        Ok(_) => Ok(("", WishlistLine::EmptyLine)),
        Err(e) => Err(e),
    }
}

fn comment_line(i: &str) -> IResult<&str, WishlistLine> {
    match char('#')(i) {
        Ok((r, _)) => Ok(("", WishlistLine::Comment(r.to_string()))),
        Err(e) => Err(e),
    }
}

fn is_pkgname_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '.' || c == '+'
}

fn package_name(i: &str) -> IResult<&str, &str> {
    i.split_at_position1_complete(|item| !is_pkgname_char(item), ErrorKind::Char)
}

enum PkgOption {
    InstallRecomm,
    NoRecomm,
    VersionRequirement(VersionRequirement),
}

fn pkg_option(i: &str) -> IResult<&str, PkgOption> {
    if let Ok((i, _)) = tag::<_, _, Error<&str>>("recomm")(i) {
        return Ok((i, PkgOption::InstallRecomm));
    }

    if let Ok((i, _)) = tag::<_, _, Error<&str>>("no_recomm")(i) {
        return Ok((i, PkgOption::NoRecomm));
    }

    if let Ok((i, req)) = parse_version_requirement(i) {
        return Ok((i, PkgOption::VersionRequirement(req)));
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
        install_recomm: None,
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
                PkgOption::InstallRecomm => res.install_recomm = Some(true),
                PkgOption::NoRecomm => res.install_recomm = Some(true),
                PkgOption::VersionRequirement(req) => {
                    res.version = res.version.combine(&req).unwrap();
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

fn package_line_wrapper(i: &str) -> IResult<&str, WishlistLine> {
    let (i, res) = package_line(i)?;
    Ok((i, WishlistLine::PkgRequest(res)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PkgVersion;
    use nom::{error::Error, IResult};
    use std::convert::TryFrom;
    #[test]
    fn test_empty_line() {
        let t: Vec<(&str, IResult<&str, WishlistLine>)> = vec![
            ("", Ok(("", WishlistLine::EmptyLine))),
            ("   ", Ok(("", WishlistLine::EmptyLine))),
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
        let t: Vec<(&str, IResult<&str, WishlistLine>)> = vec![
            ("#", Ok(("", WishlistLine::Comment("".to_string())))),
            ("#   ", Ok(("", WishlistLine::Comment("   ".to_string())))),
            (
                "# This is a comment",
                Ok(("", WishlistLine::Comment(" This is a comment".to_string()))),
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
            "abc (no_recomm, >1)",
            PkgRequest {
                name: "abc".to_string(),
                version: VersionRequirement {
                    lower_bond: Some((PkgVersion::try_from("1").unwrap(), false)),
                    upper_bond: None,
                },
                install_recomm: Some(true),
            },
            (
                "pkgname (>1, <2)",
                PkgRequest {
                    name: "abc".to_string(),
                    version: VersionRequirement {
                        lower_bond: Some((PkgVersion::try_from("1").unwrap(), false)),
                        upper_bond: Some((PkgVersion::try_from("2").unwrap(), true)),
                    },
                    install_recomm: None,
                },
            ),
        )];

        for t in tests {
            assert_eq!(package_line(t.0).unwrap().1, t.1);
        }
    }
}
