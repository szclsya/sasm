use anyhow::{bail, Context, Result};
use lazy_static::lazy_static;
use regex::{Captures, Regex};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader},
    os::unix::fs::FileExt,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct IgnoreRules {
    user_ignorerules_path: PathBuf,
    user_ignorerules_modified: bool,
    user: Vec<IgnoreRuleLine>,
    vendor: Vec<Vec<IgnoreRuleLine>>,
}

#[derive(Debug)]
enum IgnoreRuleLine {
    Rule(String),
    EmptyLine,
    Comment(String),
}

impl IgnoreRules {
    pub fn from_files(user: PathBuf, vendor: &[PathBuf]) -> Result<Self> {
        let user_rules = read_ignorerules_from_file(&user)?;
        let mut vendor_rules = Vec::new();
        for path in vendor {
            vendor_rules.push(read_ignorerules_from_file(path)?);
        }

        Ok(IgnoreRules {
            user_ignorerules_path: user,
            user_ignorerules_modified: false,
            user: user_rules,
            vendor: vendor_rules,
        })
    }

    pub fn gen_rules(&self) -> Result<Vec<String>> {
        let mut res = Vec::new();
        for line in &self.user {
            if let IgnoreRuleLine::Rule(rule) = line {
                res.push(fill_variables(rule)?);
            }
        }

        for ruleset in &self.vendor {
            for line in ruleset {
                if let IgnoreRuleLine::Rule(rule) = line {
                    res.push(fill_variables(rule)?);
                }
            }
        }

        Ok(res)
    }

    pub fn add(&mut self, rule: String) -> Result<()> {
        self.user.push(parse_ignorerule_line(rule)?);
        self.user_ignorerules_modified = true;
        Ok(())
    }

    pub fn remove(&mut self, rule: &str) -> Result<()> {
        let old_len = self.user.len();
        self.user.retain(|line| {
            if let IgnoreRuleLine::Rule(r) = line {
                r == rule
            } else {
                false
            }
        });

        if self.user.len() < old_len {
            self.user_ignorerules_modified = true;
            Ok(())
        } else {
            bail!("Rule not found in user IgnoreRules");
        }
    }

    pub fn export(&self) -> Result<bool> {
        if !self.user_ignorerules_modified {
            return Ok(false);
        }
        let new_lines: Vec<String> = self
            .user
            .iter()
            .map(|line| match line {
                IgnoreRuleLine::EmptyLine => "".to_string(),
                IgnoreRuleLine::Comment(comment) => comment.to_owned(),
                IgnoreRuleLine::Rule(rule) => rule.to_owned(),
            })
            .collect();

        let new_user_ignorerules = new_lines.join("\n");

        // Open user ignorerules
        let ignoreruels_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.user_ignorerules_path)?;
        ignoreruels_file.set_len(0)?;
        ignoreruels_file
            .write_all_at(&new_user_ignorerules.into_bytes(), 0)
            .context(format!(
                "Failed to write to IgnoreRules file at {}",
                self.user_ignorerules_path.display()
            ))?;

        Ok(true)
    }
}

fn read_ignorerules_from_file(path: &Path) -> Result<Vec<IgnoreRuleLine>> {
    let mut lines = Vec::new();
    let f =
        File::open(path).context(format!("Failed to open IgnoreRules at {}", path.display()))?;
    let reader = BufReader::new(f);
    let mut line_no = 0;
    for line in reader.lines() {
        let line = line?;
        line_no += 1;
        let line = parse_ignorerule_line(line).context(format!(
            "Invalid rule in {} at line {}",
            path.display(),
            line_no
        ))?;
        lines.push(line);
    }
    Ok(lines)
}

fn parse_ignorerule_line(line: String) -> Result<IgnoreRuleLine> {
    lazy_static! {
        static ref COMMENT_LINE: Regex = Regex::new(r"^#.*$").unwrap();
        static ref EMPTY_LINE: Regex = Regex::new(r"^ *$").unwrap();
    }

    if COMMENT_LINE.is_match(&line) {
        Ok(IgnoreRuleLine::Comment(line))
    } else if EMPTY_LINE.is_match(&line) {
        Ok(IgnoreRuleLine::EmptyLine)
    } else {
        // Check input sanity
        sanitize_ignore_rule(&line)?;
        // Generate Regex
        Ok(IgnoreRuleLine::Rule(line))
    }
}

fn sanitize_ignore_rule(rule: &str) -> Result<()> {
    lazy_static! {
        static ref IGNORE_RULE: Regex = Regex::new("^[a-z0-9-{A-Z_}]+$").unwrap();
    }
    if IGNORE_RULE.is_match(rule) {
        Ok(())
    } else {
        bail!("Invalid ignore rule {}", rule);
    }
}

fn fill_variables(rule: &str) -> Result<String> {
    lazy_static! {
        static ref EXPANSION: Regex = Regex::new(r"\{([A-Z_]+)}").unwrap();
    }

    let kernel_version = get_kernel_version()?;
    let res = EXPANSION.replace_all(rule, |caps: &Captures| {
        match caps.get(1).unwrap().as_str() {
            "KERNEL_VERSION" => &kernel_version,
            _ => "",
        }
    });

    Ok(res.to_string())
}

fn get_kernel_version() -> Result<String> {
    let uname = nix::sys::utsname::uname();
    let version = uname.release();
    let sections: Vec<&str> = version.split('-').collect();
    if sections.is_empty() {
        bail!("Cannot get kernel version: Malformed kernel release");
    }
    Ok(sections.get(0).unwrap().to_string())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_ignorerule_parse() {
        let tests = vec![(
            "linux-kernel-{KERNEL_VERSION}",
            format!("^linux-kernel-{}$", get_kernel_version().unwrap()),
        )];

        for (input, output) in tests {
            //assert_eq!(parse_ignorerule_line(input.to_string()).unwrap(), output);
        }
    }
}
