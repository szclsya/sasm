use anyhow::{bail, format_err, Result};
use lazy_static::lazy_static;
use nom::{character::complete::*, error::ErrorKind, IResult, InputTakeAtPosition};
use regex::Regex;
use serde::{Deserialize, Serialize, Serializer};
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
#[derive(PartialEq, Eq, Clone, Debug, Deserialize)]
pub struct PkgVersion {
    pub epoch: usize,
    pub version: Vec<(String, Option<u128>)>,
    pub revision: usize,
}

fn is_upstream_version_char(c: char) -> bool {
    c.is_alphanumeric() || c == '.' || c == '+' || c == '~'
}

fn upstream_version(i: &str) -> IResult<&str, &str> {
    i.split_at_position1_complete(|item| !is_upstream_version_char(item), ErrorKind::Char)
}

fn standard_parse_version(i: &str) -> IResult<&str, PkgVersion> {
    let (i, epoch) =
        match nom::sequence::pair::<_, _, _, nom::error::Error<&str>, _, _>(digit1, char(':'))(i) {
            Ok((i, (epoch, _))) => (i, epoch.parse().unwrap()),
            Err(_) => (i, 0),
        };
    let (i, upstream_version) = upstream_version(i)?;
    let (i, revision) = match i.len() {
        0 => (i, 0),
        _ => {
            let (i, (_, revision)) = nom::sequence::pair(char('-'), digit1)(i)?;
            let revision = revision.parse().unwrap();
            (i, revision)
        }
    };
    // Ensure nothing's left
    nom::combinator::eof(i)?;

    let res = PkgVersion {
        epoch,
        version: parse_version_string(upstream_version).unwrap(),
        revision,
    };

    Ok((i, res))
}

fn alt_is_upstream_version_char(c: char) -> bool {
    is_upstream_version_char(c) || c == '-'
}

fn alt_upstream_version(i: &str) -> IResult<&str, &str> {
    i.split_at_position1_complete(|item| !alt_is_upstream_version_char(item), ErrorKind::Char)
}

fn alt_parse_version(i: &str) -> IResult<&str, PkgVersion> {
    let (i, epoch) =
        match nom::sequence::pair::<_, _, _, nom::error::Error<&str>, _, _>(digit1, char(':'))(i) {
            Ok((i, (epoch, _))) => (i, epoch.parse().unwrap()),
            Err(_) => (i, 0),
        };
    let (i, upstream_version) = alt_upstream_version(i)?;
    // Ensure nothing left
    nom::combinator::eof(i)?;

    let res = PkgVersion {
        epoch,
        version: parse_version_string(upstream_version).unwrap(),
        revision: 0,
    };

    Ok((i, res))
}

pub fn parse_version(i: &str) -> IResult<&str, PkgVersion> {
        let (i, res) = match standard_parse_version(i) {
            Ok(res) => res,
            Err(_) => {
                use crate::warn;
                warn!("{}", i);
                alt_parse_version(i)?
            }
        };
        Ok((i, res ))
}

impl TryFrom<&str> for PkgVersion {
    type Error = anyhow::Error;
    fn try_from(s: &str) -> Result<Self> {
        let (_, res ) = parse_version(s).map_err(|e| format_err!("Malformed version: {}", e))?;
        Ok(res)
    }
}

impl fmt::Display for PkgVersion {
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

impl Serialize for PkgVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let res = self.to_string();
        serializer.serialize_str(&res)
    }
}

impl Ord for PkgVersion {
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

impl PartialOrd for PkgVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
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

    #[test]
    fn pkg_ver_from_str() {
        let source = vec!["1.1.1.", "999:0+git20210608-1"];
        let result = vec![
            PkgVersion {
                epoch: 0,
                version: vec![
                    ("".to_string(), Some(1)),
                    (".".to_string(), Some(1)),
                    (".".to_string(), Some(1)),
                    (".".to_string(), None),
                ],
                revision: 0,
            },
            PkgVersion {
                epoch: 999,
                version: vec![
                    ("".to_string(), Some(0)),
                    ("+git".to_string(), Some(20210608)),
                ],
                revision: 1,
            },
        ];

        for (pos, e) in source.iter().enumerate() {
            assert_eq!(PkgVersion::try_from(*e).unwrap(), result[pos]);
        }
    }
}
