use anyhow::{bail, Context, Result};
use lazy_static::lazy_static;
use regex::{Captures, Regex };
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

pub struct IgnoreRules {
    rules: Vec<Regex>,
}

impl IgnoreRules {
    pub fn from_file(path: &Path) -> Result<Self> {
        // Read lines from ignorerules file
        let mut rules = Vec::new();
        let f = File::open(path)?;
        let reader = BufReader::new(f);
        let mut line_no = 0;
        for line in reader.lines() {
            let line = line?;
            line_no += 1;
            let rule = parse_ignorerule_line(line)
                .context(format!("Invalid rule in {} at line {}", path.display(), line_no))?;
            let regex = Regex::new(&rule)
                .context(format!("Invalid rule in {} at line {}", path.display(), line_no))?;
            rules.push(regex);
        }

        Ok(IgnoreRules { rules })
    }
}

fn parse_ignorerule_line(mut line: String) -> Result<String> {
    // Check input sanity
    sanitize_ignore_rule(&line)?;
    // Fill variables
    fill_variables(&mut line)?;
    // Generate Regex
    let rule = format!("^{}$", line);
    Ok(rule)
}

fn sanitize_ignore_rule(rule: &str) -> Result<()> {
    lazy_static! {
        static ref IGNORE_RULE: Regex = Regex::new("^[a-z0-9-.+{A-Z_}]+$").unwrap();
    }
    if IGNORE_RULE.is_match(rule) {
        Ok(())
    } else {
        bail!("Invalid ignore rule {}", rule);
    }
}

fn fill_variables(rule: &mut String) -> Result<()> {
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

    *rule = res.to_string();
    Ok(())
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
        let tests = vec![("linux-kernel-{KERNEL_VERSION}",
                          format!("^linux-kernel-{}$", get_kernel_version().unwrap()))];

        for (input, output) in tests {
            assert_eq!(parse_ignorerule_line(input.to_string()).unwrap(), output);
        }
    }
}
