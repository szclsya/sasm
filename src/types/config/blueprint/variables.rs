use anyhow::{bail, Result};
use lazy_static::lazy_static;
use regex::{Captures, Regex};

pub fn fill_variables(rule: &str) -> Result<String> {
    lazy_static! {
        static ref EXPANSION: Regex = Regex::new(r"\{([A-Z_]+)}").unwrap();
    }

    let kernel_version = get_kernel_version()?;
    let mut unknown_variable = Vec::new();
    let res = EXPANSION.replace_all(rule, |caps: &Captures| match caps.get(1).unwrap().as_str() {
        "KERNEL_VERSION" => &kernel_version,
        unintended => {
            unknown_variable.push(unintended.to_owned());
            ""
        }
    });

    if !unknown_variable.is_empty() {
        bail!("Unknown variable: {}.", unknown_variable.join(", "));
    }

    Ok(res.to_string())
}

fn get_kernel_version() -> Result<String> {
    let uname = nix::sys::utsname::uname();
    let version = uname.release();
    let section = version.split('-').next();
    if let Some(section) = section {
        return Ok(section.to_string());
    }
    bail!("Failed to obtain kernel version: malformed kernel local version.");
}
