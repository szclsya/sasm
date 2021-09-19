use crate::types::{PkgActionModifier, PkgActions};

use anyhow::{bail, Result};
use lazy_static::lazy_static;
use regex::{Captures, Regex};

lazy_static! {
    static ref EXPANSION: Regex = Regex::new(r"\{([A-Z_]+)}").unwrap();
}

/// Given a template, prevent removing packages from generated package names
pub struct DontRemove {
    rules: Vec<String>,
    kernel_version: String,
}

impl DontRemove {
    pub fn new(rules: Vec<String>) -> Result<Self> {
        Ok(DontRemove {
            rules,
            kernel_version: get_kernel_version()?,
        })
    }
}

impl PkgActionModifier for DontRemove {
    fn apply(&self, actions: &mut PkgActions) {
        let pkgnames: Vec<String> = self
            .rules
            .iter()
            .map(|rule| {
                EXPANSION
                    .replace_all(rule, |caps: &Captures| {
                        match caps.get(1).unwrap().as_str() {
                            "KERNEL_VERSION" => &self.kernel_version,
                            _ => "",
                        }
                    })
                    .to_string()
            })
            .collect();
        println!("{:?}", pkgnames);

        actions
            .remove
            .retain(|(pkgname, _)| !pkgnames.contains(pkgname));
        actions
            .remove
            .retain(|(pkgname, _)| !pkgnames.contains(pkgname));
    }
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
    fn test_kernel_version_replacement() {
        let result = format!("linux-kernel-{}", get_kernel_version().unwrap());

        let modifier = DontRemove::new(vec!["linux-kernel-{KERNEL_VERSION}".to_string()]).unwrap();
        let mut actions = PkgActions::default();
        actions.remove.push((result.clone(), 0));
        modifier.apply(&mut actions);
        assert_eq!(actions, PkgActions::default());
    }
}
