use anyhow::{bail, format_err, Result};
use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;

lazy_static! {
    static ref DIGIT_TABLE: Vec<char> = "1234567890".chars().collect();
    static ref NON_DIGIT_TABLE: Vec<char> =
        "~|ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz+-."
            .chars()
            .collect();
}

/// dpkg style version comparison.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct PackageVersion {
    epoch: usize,
    version: Vec<(String, Option<u128>)>,
    revision: usize,
}

impl PackageVersion {
    pub fn from(s: &str) -> Result<Self> {
        lazy_static! {
            static ref VER_PARTITION: Regex = Regex::new(
                r"^((?P<epoch>[0-9]+):)?(?P<version>[A-Za-z0-9.+~]+)(\-(?P<revision>[0-9]+))?$"
            )
            .unwrap();
            static ref ALT_VER_PARTITION: Regex =
                Regex::new(r"^((?P<epoch>[0-9]+):)?(?P<version>[A-Za-z0-9.+-~]+)$").unwrap();
        }

        let mut epoch = 0;
        let version;
        let mut revision = 0;

        let segments = match VER_PARTITION.captures(s) {
            Some(c) => c,
            None => ALT_VER_PARTITION
                .captures(s)
                .ok_or_else(|| format_err!("Malformed version string: {}", s))?,
        };
        if let Some(e) = segments.name("epoch") {
            epoch = e
                .as_str()
                .parse()
                .map_err(|_| format_err!("Malformed epoch"))?;
        }
        if let Some(v) = segments.name("version") {
            version = parse_version_string(v.as_str())?;
        } else {
            bail!("Version segment is required")
        }
        if let Some(r) = segments.name("revision") {
            revision = r
                .as_str()
                .parse()
                .map_err(|_| format_err!("Malformed revision"))?
        }

        Ok(PackageVersion {
            epoch,
            version,
            revision,
        })
    }
}

impl fmt::Display for PackageVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.epoch != 0 {
            write!(f, "{}:", self.epoch)?;
        }
        for segment in self.version.iter() {
            write!(f, "{}", &segment.0)?;
            if let Some(num) = segment.1 {
                write!(f, "{}", num)?;
            }
        }
        if self.revision != 0 {
            write!(f, "-{}", self.revision)?;
        }
        Ok(())
    }
}

impl Ord for PackageVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.epoch > other.epoch {
            return Ordering::Greater;
        }

        if self.epoch < other.epoch {
            return Ordering::Less;
        }

        let mut self_vec = self.version.clone();
        let mut other_vec = other.version.clone();
        // Add | to the back to make sure end of string is more significant than '~'
        self_vec.push(("|".to_string(), None));
        other_vec.push(("|".to_string(), None));
        // Reverse them so that we can pop them
        self_vec.reverse();
        other_vec.reverse();
        while !self_vec.is_empty() {
            // Match non digit
            let mut x = self_vec.pop().unwrap();
            let mut y = match other_vec.pop() {
                Some(y) => y,
                None => {
                    return Ordering::Greater;
                }
            };

            // Magic! To make sure end of string have the correct rank
            x.0.push('|');
            y.0.push('|');
            let x_nondigit_rank = str_to_ranks(&x.0);
            let y_nondigit_rank = str_to_ranks(&y.0);
            for (pos, r_x) in x_nondigit_rank.iter().enumerate() {
                match r_x.cmp(&y_nondigit_rank[pos]) {
                    Ordering::Greater => {
                        return Ordering::Greater;
                    }
                    Ordering::Less => {
                        return Ordering::Less;
                    }
                    Ordering::Equal => (),
                }
            }

            // Compare digit part
            let x_digit = x.1.unwrap_or(0);
            let y_digit = y.1.unwrap_or(0);
            match x_digit.cmp(&y_digit) {
                Ordering::Greater => {
                    return Ordering::Greater;
                }
                Ordering::Less => {
                    return Ordering::Less;
                }
                Ordering::Equal => (),
            }
        }

        // If other still has remaining segments
        if !other_vec.is_empty() {
            return Ordering::Greater;
        }

        // Finally, compare revision
        match self.revision.cmp(&other.revision) {
            Ordering::Greater => {
                return Ordering::Greater;
            }
            Ordering::Less => {
                return Ordering::Less;
            }
            Ordering::Equal => (),
        }

        Ordering::Equal
    }
}

impl PartialOrd for PackageVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(PartialEq, Eq, Clone, Debug, Deserialize)]
#[serde(try_from = "&str")]
pub struct VersionRequirement {
    // The bool represents if the restriction is inclusive
    pub lower_bond: Option<(PackageVersion, bool)>,
    pub upper_bond: Option<(PackageVersion, bool)>,
}

impl VersionRequirement {
    pub fn new() -> Self {
        VersionRequirement {
            lower_bond: None,
            upper_bond: None,
        }
    }

    pub fn within(&self, ver: &PackageVersion) -> bool {
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

impl TryFrom<&str> for VersionRequirement {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self> {
        lazy_static! {
            static ref VER_REQ: Regex =
                Regex::new(r"^(?P<req_type>[<>=]+) ?(?P<req_ver>[A-Za-z0-9.\-:+~]+)$").unwrap();
        }
        let mut ver_req = VersionRequirement {
            upper_bond: None,
            lower_bond: None,
        };

        if s == "any" {
            return Ok(ver_req);
        }

        let segments = VER_REQ
            .captures(s)
            .ok_or_else(|| format_err!("Malformed version requirement"))?;
        let req_type = segments.name("req_type").unwrap().as_str();
        let ver = PackageVersion::from(segments.name("req_ver").unwrap().as_str())?;
        match req_type {
            "=" => {
                ver_req.upper_bond = Some((ver.clone(), true));
                ver_req.lower_bond = Some((ver, true));
            }
            ">" => {
                ver_req.lower_bond = Some((ver, false));
            }
            ">=" => {
                ver_req.lower_bond = Some((ver, true));
            }
            "<" => {
                ver_req.upper_bond = Some((ver, false));
            }
            "<=" => {
                ver_req.upper_bond = Some((ver, true));
            }
            _ => {}
        }
        Ok(ver_req)
    }
}

fn parse_version_string(s: &str) -> Result<Vec<(String, Option<u128>)>> {
    if s.is_empty() {
        bail!("Empty version string")
    }

    let check_first_digit = Regex::new("^[0-9]").unwrap();
    if !check_first_digit.is_match(s) {
        bail!("Version string must start with digit")
    }

    let mut in_digit = true;
    let mut nondigit_buffer = String::new();
    let mut digit_buffer = String::new();
    let mut result = Vec::new();
    for c in s.chars() {
        if NON_DIGIT_TABLE.contains(&c) {
            if in_digit && !digit_buffer.is_empty() {
                // Previously in digit sequence
                // Try to parse digit segment
                let num: u128 = digit_buffer.parse()?;
                result.push((nondigit_buffer.clone(), Some(num)));
                nondigit_buffer.clear();
                digit_buffer.clear();
            }
            nondigit_buffer.push(c);
            in_digit = false;
        } else if DIGIT_TABLE.contains(&c) {
            digit_buffer.push(c);
            in_digit = true;
        } else {
            // This should not happen, we should have sanitized input
            bail!("Invalid character in version")
        }
    }

    // Commit last segment
    if digit_buffer.is_empty() {
        result.push((nondigit_buffer, None));
    } else {
        result.push((nondigit_buffer, Some(digit_buffer.parse::<u128>()?)))
    }
    Ok(result)
}

fn str_to_ranks(s: &str) -> Vec<usize> {
    let res: Vec<usize> = s
        .chars()
        .map(|c| {
            // Input should already be sanitized with the input regex
            NON_DIGIT_TABLE.iter().position(|&i| c == i).unwrap()
        })
        .collect();

    res
}

#[cfg(test)]
mod test {
    use super::*;
    use std::cmp::Ordering::*;

    #[test]
    fn pkg_ver_from_str() {
        let source = vec!["1.1.1.", "999:0+git20210608-1"];
        let result = vec![
            PackageVersion {
                epoch: 0,
                version: vec![
                    ("".to_string(), Some(1)),
                    (".".to_string(), Some(1)),
                    (".".to_string(), Some(1)),
                    (".".to_string(), None),
                ],
                revision: 0,
            },
            PackageVersion {
                epoch: 999,
                version: vec![
                    ("".to_string(), Some(0)),
                    ("+git".to_string(), Some(20210608)),
                ],
                revision: 1,
            },
        ];

        for (pos, e) in source.iter().enumerate() {
            assert_eq!(PackageVersion::from(e).unwrap(), result[pos]);
        }
    }

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
                PackageVersion::from(e.0).unwrap(),
                PackageVersion::from(e.2).unwrap()
            );
            assert_eq!(
                PackageVersion::from(e.0)
                    .unwrap()
                    .cmp(&PackageVersion::from(e.2).unwrap()),
                e.1
            );
        }
    }

    #[test]
    fn pkg_ver_eq() {
        let source = vec![("1.1+git2021", "1.1+git2021")];
        for e in &source {
            assert_eq!(
                PackageVersion::from(e.0).unwrap(),
                PackageVersion::from(e.1).unwrap()
            );
        }
    }
}
