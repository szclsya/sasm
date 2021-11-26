use crate::error;
use crate::types::{parse_version_requirement, VersionRequirement};

use anyhow::{bail, Context, Result};
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::*,
    error::{Error, ErrorKind, ParseError},
    sequence::*,
    IResult, InputTakeAtPosition,
};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader},
    os::unix::fs::FileExt,
    path::{Path, PathBuf},
};

/// A collection of
pub struct Blueprints {
    user_blueprint_path: PathBuf,
    // If we need to export the blueprint back to disk
    user_blueprint_modified: bool,
    user: Vec<BlueprintLine>,
    vendor: Vec<Vec<BlueprintLine>>,
}

impl Blueprints {
    pub fn from_files(user: PathBuf, vendor: &[PathBuf]) -> Result<Self> {
        let user_blueprint = read_blueprint_from_file(&user)?;
        let mut vendor_blueprints = Vec::with_capacity(vendor.len());
        for path in vendor {
            vendor_blueprints.push(read_blueprint_from_file(path)?);
        }

        Ok(Blueprints {
            user_blueprint_path: user,
            user_blueprint_modified: false,
            user: user_blueprint,
            vendor: vendor_blueprints,
        })
    }

    pub fn get_pkg_requests(&self) -> Vec<&PkgRequest> {
        // Add user blueprint first
        let mut res: Vec<&PkgRequest> = self
            .user
            .iter()
            .filter_map(|line| match line {
                BlueprintLine::PkgRequest(req) => Some(req),
                _ => None,
            })
            .collect();

        // Then add vendor blueprint
        for vendor in &self.vendor {
            for line in vendor {
                if let BlueprintLine::PkgRequest(req) = line {
                    res.push(req);
                }
            }
        }

        // Duplicates are allowed, so we shall dedup here
        res.dedup();
        res
    }

    pub fn add(&mut self, pkgname: &str) -> Result<()> {
        if self.user_list_contains(pkgname) {
            bail!("Package {} already exists in user blueprint", pkgname);
        }

        let pkgreq = PkgRequest {
            name: pkgname.to_string(),
            version: VersionRequirement::default(),
            install_recomm: None,
        };
        self.user.push(BlueprintLine::PkgRequest(pkgreq));
        self.user_blueprint_modified = true;
        Ok(())
    }

    pub fn remove(&mut self, pkgname: &str) -> Result<()> {
        if !self.user_list_contains(pkgname) {
            bail!("Package with name {} not found in user blueprint", pkgname)
        } else {
            self.user.retain(|line| match line {
                BlueprintLine::PkgRequest(req) => req.name != pkgname,
                _ => true,
            });
            self.user_blueprint_modified = true;
            Ok(())
        }
    }

    // Write back user blueprint
    pub fn export(&self) -> Result<bool> {
        if !self.user_blueprint_modified {
            // If not modified, nothing to do here.
            return Ok(false);
        }

        let mut res = String::new();
        for l in &self.user {
            match l {
                BlueprintLine::Comment(content) => res.push_str(&format!("#{}\n", content)),
                BlueprintLine::EmptyLine => res.push('\n'),
                BlueprintLine::PkgRequest(req) => res.push_str(&format!("{}\n", req.to_string())),
            }
        }

        // Open user blueprint
        let blueprint_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.user_blueprint_path)?;
        blueprint_file.set_len(0)?;
        blueprint_file
            .write_all_at(&res.into_bytes(), 0)
            .context(format!(
                "Failed to write to blueprint file at {}",
                self.user_blueprint_path.display()
            ))?;

        Ok(true)
    }

    fn user_list_contains(&self, pkgname: &str) -> bool {
        for line in &self.user {
            if let BlueprintLine::PkgRequest(req) = line {
                if req.name == pkgname {
                    return true;
                }
            }
        }
        false
    }
}

fn read_blueprint_from_file(path: &Path) -> Result<Vec<BlueprintLine>> {
    // Read lines from blueprint file
    let mut lines = Vec::new();
    let f = File::open(path).context(format!("Failed to open Blueprint at {}", path.display()))?;
    let reader = BufReader::new(f);
    for line in parse_blueprint_lines(reader)? {
        lines.push(line);
    }

    Ok(lines)
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum BlueprintLine {
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
