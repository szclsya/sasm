use super::{parse_version, PkgVersion};
use anyhow::{bail, format_err, Result};
use nom::{branch::alt, bytes::complete::tag, character::complete::*, error::context, IResult};
use serde::{Deserialize, Serialize, Serializer};
use std::cmp::Ordering::*;
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

    /// Check if this VersionRequirement accepts arbitary version
    pub fn is_arbitary(&self) -> bool {
        self.lower_bond.is_none() && self.upper_bond.is_none()
    }

    /// Create a new VersionRequirment that satisfies both original requirements
    pub fn combine(&self, other: &VersionRequirement) -> Result<VersionRequirement> {
        let mut new = self.clone();
        if self.lower_bond.is_none() && other.lower_bond.is_some() {
            new.lower_bond = other.lower_bond.clone();
        } else if self.lower_bond.is_some() && other.lower_bond.is_some() {
            let this = self.lower_bond.as_ref().unwrap();
            let other = other.lower_bond.as_ref().unwrap();
            if this.0 < other.0 || (this.0 == other.0 && this.1 && !other.1) {
                // Either other is stricter than this (higher lower-bond),
                // or same bond but other is not inclusive
                new.lower_bond = Some(other.clone());
            }
        }

        if self.upper_bond.is_none() && other.upper_bond.is_some() {
            new.upper_bond = other.upper_bond.clone();
        } else if self.upper_bond.is_some() && other.upper_bond.is_some() {
            let this = self.upper_bond.as_ref().unwrap();
            let other = other.upper_bond.as_ref().unwrap();
            if this.0 > other.0 || (this.0 == other.0 && this.1 && !other.1) {
                // Either other is stricter than this (lower upper-bond),
                // or same bond but other is not inclusive
                new.upper_bond = Some(other.clone());
            }
        }

        if !new.valid() {
            bail!("Cannot merge version requirements {} and {}", self, other);
        }

        Ok(new)
    }

    /// Validate if this VersionRequirment can be satisfied for some PkgVersion
    pub fn valid(&self) -> bool {
        if self.lower_bond.is_some() && self.upper_bond.is_some() {
            let lower = self.lower_bond.as_ref().unwrap();
            let upper = self.upper_bond.as_ref().unwrap();
            match lower.0.cmp(&upper.0) {
                Greater => false,
                Equal => {
                    // must be both inclusive to be valid
                    lower.1 && upper.1
                }
                Less => true,
            }
        } else {
            true
        }
    }

    /// Check if a PkgVersion satisfies this VersionRequirement
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

/// Use `nom` to parse a VersionRequirement string
pub fn parse_version_requirement(i: &str) -> IResult<&str, VersionRequirement> {
    let (i, compare) = context(
        "parsing compare literal",
        alt((tag(">="), tag("<="), tag("="), tag(">"), tag("<"))),
    )(i)?;
    let (i, _) = space0(i)?;
    let (i, ver) = context("parsing version in VersionRequirement", parse_version)(i)?;
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
        if !ver_req.valid() {
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
                write!(f, ", ")?;
            }
            // If inclusive
            if upper.1 {
                write!(f, "<={}", upper.0)?;
            } else {
                write!(f, "<{}", upper.0)?;
            }
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
    #[test]
    fn merge_ver_rq() {
        let tests = vec![
            (
                VersionRequirement::default(),
                VersionRequirement::default(),
                VersionRequirement::default(),
            ),
            (
                VersionRequirement::default(),
                VersionRequirement::try_from(">1").unwrap(),
                VersionRequirement::try_from(">1").unwrap(),
            ),
            (
                VersionRequirement::try_from(">1").unwrap(),
                VersionRequirement::try_from(">=1").unwrap(),
                VersionRequirement::try_from(">1").unwrap(),
            ),
            (
                VersionRequirement::try_from(">1").unwrap(),
                VersionRequirement::try_from(">2").unwrap(),
                VersionRequirement::try_from(">2").unwrap(),
            ),
            (
                VersionRequirement::try_from(">2").unwrap(),
                VersionRequirement::try_from(">1").unwrap(),
                VersionRequirement::try_from(">2").unwrap(),
            ),
            (
                VersionRequirement::try_from(">1").unwrap(),
                VersionRequirement::try_from("<=2").unwrap(),
                VersionRequirement {
                    lower_bond: Some((PkgVersion::try_from("1").unwrap(), false)),
                    upper_bond: Some((PkgVersion::try_from("2").unwrap(), true)),
                },
            ),
        ];

        for t in tests {
            assert_eq!(t.0.combine(&t.1).unwrap(), t.2);
        }
    }

    #[test]
    fn merge_ver_fail() {
        let tests = vec![(
            VersionRequirement::try_from(">1").unwrap(),
            VersionRequirement::try_from("<1").unwrap(),
        )];

        for t in tests {
            assert_eq!(t.0.combine(&t.1).is_ok(), false);
        }
    }
}
