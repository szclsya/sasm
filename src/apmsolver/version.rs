use anyhow::{bail, format_err, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::cmp::Ordering;

lazy_static! {
    static ref DIGIT_TABLE: Vec<char> = "1234567890".chars().collect();
    static ref NON_DIGIT_TABLE: Vec<char> =
        "~|ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz+."
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
        }

        let mut epoch = 0;
        let mut version = Vec::new();
        let mut revision = 0;

        let segments = VER_PARTITION
            .captures(s)
            .ok_or(format_err!("Malformed version string"))?;
        if let Some(e) = segments.name("epoch") {
            epoch = e
                .as_str()
                .parse()
                .map_err(|_| format_err!("Malformed epoch"))?;
        }
        if let Some(v) = segments.name("version") {
            version = parse_version_string(v.as_str())?;
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
                if r_x > &y_nondigit_rank[pos] {
                    return Ordering::Greater;
                } else if r_x < &y_nondigit_rank[pos] {
                    return Ordering::Less;
                }
            }

            // Compare digit part
            let x_digit = x.1.unwrap_or(0);
            let y_digit = y.1.unwrap_or(0);
            if x_digit > y_digit {
                return Ordering::Greater;
            } else if x_digit < y_digit {
                return Ordering::Less;
            }
        }

        // If other still has remaining segments, then other is larger
        if other_vec.len() > 0 {
            return Ordering::Less;
        }

        // Finally, compare revision
        if self.revision > other.revision {
            return Ordering::Greater;
        } else if self.revision < other.revision {
            return Ordering::Less;
        }

        Ordering::Equal
    }
}

impl PartialOrd for PackageVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn parse_version_string(s: &str) -> Result<Vec<(String, Option<u128>)>> {
    if s.len() == 0 {
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
            if in_digit && digit_buffer.len() != 0 {
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
                version: vec![("".to_string(), Some(0)), ("+git".to_string(), Some(20210608))],
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
            ("1.1.1", "1.1.2"),
            ("1b", "1a"),
            ("1~~", "1~~a"),
            ("1~~a", "1~"),
        ];
        let result = vec![false, true, false, false];

        for (pos, e) in source.iter().enumerate() {
            assert_eq!(
                PackageVersion::from(e.0).unwrap() > PackageVersion::from(e.1).unwrap(),
                result[pos]
            );
        }
    }

    #[test]
    fn pkg_ver_eq() {
     let source = vec![
            ("1.1+git2021", "1.1+git2021")
        ];
        for e in &source {
            assert_eq!(
                PackageVersion::from(e.0).unwrap(), PackageVersion::from(e.1).unwrap()
            );
        }
    }
}
