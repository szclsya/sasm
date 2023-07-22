use crate::types::PkgVersion;

use anyhow::{bail, format_err, Context, Error, Result};
use std::collections::HashMap;

/// Status of package on this instance, extracted from pacman local state db
/// Usually located at /var/lib/pacman/local
#[derive(Clone)]
pub struct PkgStatus {
    pub name: String,
    pub version: PkgVersion,
    pub install_size: u64,
}

impl TryFrom<HashMap<&str, String>> for PkgStatus {
    type Error = Error;

    #[inline]
    fn try_from(mut f: HashMap<&str, String>) -> Result<PkgStatus, Self::Error> {
        let name = f
            .remove("Package")
            .ok_or_else(|| format_err!("Malformed dpkg status database: package without name."))?;
        let state_line = f.remove("Status").ok_or_else(|| {
            format_err!("Malformed dpkg status database: no Status field for package {}.", name)
        })?;
        let version = f.remove("Version").ok_or_else(|| {
            format_err!("Malformed dpkg status database: no Version field for package {}.", name)
        })?;
        let version = PkgVersion::try_from(version.as_str()).context(format!(
            "Malformed dpkg status database: cannot parse version for {}.",
            name
        ))?;
        // Installed-Size is in kilobytes, multiply by 1024 to convert it to bytes
        let install_size: u64 = f
            .remove("Installed-Size")
            .ok_or_else(|| {
                format_err!("Malformed dpkg status database: no Version field for package {}", name)
            })?
            .parse()
            .map(|kb: u64| 1024 * kb)?;
        let essential = if let Some(word) = f.remove("Essential") {
            match word.as_str() {
                "yes" => true,
                "no" => false,
                invalid => {
                    bail!("Malformed dpkg status database: expected \"yes\"/\"no\" for Essential field, got {}.", invalid);
                }
            }
        } else {
            false
        };
        let status: Vec<&str> = state_line.split(' ').collect();
        if status.len() != 3 {
            bail!("Malformed dpkg status database.");
        }

        let res = PkgStatus { name, version, install_size };

        Ok(res)
    }
}
