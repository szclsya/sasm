use anyhow::{bail, Result};
use lazy_static::lazy_static;
use regex::{Captures, Regex};

pub fn fill_variables(rule: &str) -> Result<String> {
    lazy_static! {
        static ref EXPANSION: Regex = Regex::new(r"\{([A-Z_]+)}").unwrap();
    }

    let kernel_version = get_kernel_version()?;
    let mut unintended_variable = None;
    let res = EXPANSION.replace_all(rule, |caps: &Captures| {
        match caps.get(1).unwrap().as_str() {
            "KERNEL_VERSION" => &kernel_version,
            unintended => {
                unintended_variable = Some(unintended.to_owned());
                ""
            }
        }
    });

    if let Some(unintended) = unintended_variable {
        bail!("Unintended variable {}", unintended);
    }

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
