use super::{parse_version, PkgVersion};
use anyhow::{bail, format_err, Result};
use nom::{branch::alt, bytes::complete::tag, IResult, character::complete::*};
use serde::{Deserialize, Serialize, Serializer};
use std::convert::TryFrom;
use std::fmt;

#[derive(PartialEq, Eq, Clone, Debug, Deserialize, Default)]
#[serde(try_from = "&str")]
pub struct VersionRequirement {
    // The bool represents if the restriction is inclusive
    pub lower_bond: Option<(PkgVersion, bool)>,
    pub upper_bond: Option<(PkgVersion, bool)>,
}

impl VersionRequirement {
    pub fn new() -> Self {
        VersionRequirement {
            lower_bond: None,
            upper_bond: None,
        }
    }

    pub fn within(&self, ver: &PkgVersion) -> bool {
        if let Some(lower) = &self.lower_bond {
            // If inclusive
            if lower.1 {
                if ver < &lower.0 {
                    return false;
                }
            } else if ver <= &lower.0 {
                return false;
            }
        }

        if let Some(upper) = &self.upper_bond {
            // If inclusive
            if upper.1 {
                if ver > &upper.0 {
                    return false;
                }
            } else if ver >= &upper.0 {
                return false;
            }
        }

        true
    }
}

pub fn parse_version_requirement(i: &str) -> IResult<&str, VersionRequirement> {
    let (i, compare) = alt((tag(">="), tag("<="), tag("="), tag(">"), tag("<")))(i)?;
    let (i, _) = space0(i)?;
    let (i, ver) = parse_version(i)?;
    let mut res = VersionRequirement::default();
    match compare {
        ">" => {
            res.lower_bond = Some((ver, false));
        }
        ">=" => {
            res.lower_bond = Some((ver, true));
        }
        "=" => {
            res.lower_bond = Some((ver.clone(), true));
            res.upper_bond = Some((ver, true));
        }
        "<" => {
            res.upper_bond = Some((ver, false));
        }
        "<=" => {
            res.upper_bond = Some((ver, true));
        }
        _ => panic!(),
    }
    nom::combinator::eof(i)?;

    Ok((i, res))
}

impl TryFrom<&str> for VersionRequirement {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self> {
        if s == "any" {
            return Ok(VersionRequirement::default());
        }
        let (_, ver_req) =
            parse_version_requirement(s).map_err(|e| format_err!("Malformed version: {}", e))?;
        if ver_req.lower_bond.is_some()
            && ver_req.upper_bond.is_some()
            && ver_req.lower_bond > ver_req.upper_bond
        {
            bail!("Failed to parse version requirement: lower bond is greater than upper bond")
        }
        Ok(ver_req)
    }
}

impl fmt::Display for VersionRequirement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut written = false;
        if let Some(lower) = &self.lower_bond {
            // If inclusive
            if lower.1 {
                write!(f, ">={}", lower.0)?;
            } else {
                write!(f, ">{}", lower.0)?;
            }
            written = true;
        }
        if let Some(upper) = &self.upper_bond {
            // Add comma
            if written {
                write!(f, ",")?;
            } else {
                written = true;
            }
            // If inclusive
            if upper.1 {
                write!(f, "<={}", upper.0)?;
            } else {
                write!(f, "<{}", upper.0)?;
            }
        }
        // No version requirement
        if !written {
            write!(f, "any")?;
        }
        Ok(())
    }
}

impl Serialize for VersionRequirement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let res = self.to_string();
        serializer.serialize_str(&res)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::cmp::Ordering::*;

    #[test]
    fn pkg_ver_ord() {
        let source = vec![
            ("1.1.1", Less, "1.1.2"),
            ("1b", Greater, "1a"),
            ("1~~", Less, "1~~a"),
            ("1~~a", Less, "1~"),
            ("1", Less, "1.1"),
            ("1.0", Less, "1.1"),
            ("1.2", Less, "1.11"),
            ("1.0-1", Less, "1.1"),
            ("1.0-1", Less, "1.0-12"),
            // make them different for sorting
            ("1:1.0-0", Equal, "1:1.0"),
            ("1.0", Equal, "1.0"),
            ("1.0-1", Equal, "1.0-1"),
            ("1:1.0-1", Equal, "1:1.0-1"),
            ("1:1.0", Equal, "1:1.0"),
            ("1.0-1", Less, "1.0-2"),
            //("1.0final-5sarge1", Greater, "1.0final-5"),
            ("1.0final-5", Greater, "1.0a7-2"),
            ("0.9.2-5", Less, "0.9.2+cvs.1.0.dev.2004.07.28-1"),
            ("1:500", Less, "1:5000"),
            ("100:500", Greater, "11:5000"),
            ("1.0.4-2", Greater, "1.0pre7-2"),
            ("1.5~rc1", Less, "1.5"),
            ("1.5~rc1", Less, "1.5+1"),
            ("1.5~rc1", Less, "1.5~rc2"),
            ("1.5~rc1", Greater, "1.5~dev0"),
        ];

        for e in source {
            println!("Comparing {} vs {}", e.0, e.2);
            println!(
                "{:#?} vs {:#?}",
                PkgVersion::try_from(e.0).unwrap(),
                PkgVersion::try_from(e.2).unwrap()
            );
            assert_eq!(
                PkgVersion::try_from(e.0)
                    .unwrap()
                    .cmp(&PkgVersion::try_from(e.2).unwrap()),
                e.1
            );
        }
    }

    #[test]
    fn pkg_ver_eq() {
        let source = vec![("1.1+git2021", "1.1+git2021")];
        for e in &source {
            assert_eq!(
                PkgVersion::try_from(e.0).unwrap(),
                PkgVersion::try_from(e.1).unwrap()
            );
        }
    }
}
