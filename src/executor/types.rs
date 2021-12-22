use crate::types::PkgVersion;

use anyhow::{bail, format_err, Context, Error, Result};
use std::collections::HashMap;
use std::convert::TryFrom;

/// dpkg package state
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PkgState {
    // Not installed
    NotInstalled,
    // Previously installed, now not installed, config files remains
    ConfigFiles,
    // Installation uncompleted
    HalfInstalled,
    Unpacked,
    HalfConfigured,
    TriggerAwaited,
    TriggerPending,
    Installed,
}

impl std::convert::TryFrom<&str> for PkgState {
    type Error = Error;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let res = match s {
            "not-installed" => Self::NotInstalled,
            "config-files" => Self::ConfigFiles,
            "half-installed" => Self::HalfInstalled,
            "unpacked" => Self::Unpacked,
            "half-configured" => Self::HalfConfigured,
            "triggers-awaited" => Self::TriggerAwaited,
            "triggers-pending" => Self::TriggerPending,
            "installed" => Self::Installed,
            _ => {
                bail!("Malformed package state")
            }
        };
        Ok(res)
    }
}

/// Status of package on this instance, extracted from dpkg status db
#[derive(Clone)]
pub struct PkgStatus {
    pub name: String,
    pub version: PkgVersion,
    pub install_size: u64,
    pub essential: bool,
    pub state: PkgState,
}

impl TryFrom<HashMap<&str, String>> for PkgStatus {
    type Error = Error;

    #[inline]
    fn try_from(mut f: HashMap<&str, String>) -> Result<PkgStatus, Self::Error> {
        let name = f
            .remove("Package")
            .ok_or_else(|| format_err!("Malformed dpkg status db: no Package name for package"))?;
        let state_line = f
            .remove("Status")
            .ok_or_else(|| format_err!("Malformed dpkg status db: no Status for package"))?;
        let version = f.remove("Version").ok_or_else(|| {
            format_err!("Malformed dpkg status db: no Version for package {}", name)
        })?;
        let version = PkgVersion::try_from(version.as_str())
            .context("Malformed dpkg status db, cannot parse version")?;
        let install_size: u64 = f
            .remove("Installed-Size")
            .ok_or_else(|| {
                format_err!("Malformed dpkg status db: no Version for package {}", name)
            })?
            .parse()?;
        let essential = if let Some(word) = f.remove("Essential") {
            match word.as_str() {
                "yes" => true,
                "no" => false,
                invalid => {
                    bail!("Malformed dpkg status db: expect \"yes\"/\"no\" for Essential field, got {}", invalid);
                }
            }
        } else {
            false
        };
        let status: Vec<&str> = state_line.split(' ').collect();
        if status.len() != 3 {
            bail!("Malformed dpkg status db");
        }

        let state = PkgState::try_from(*status.get(2).unwrap())?;

        let res = PkgStatus {
            name,
            version,
            install_size,
            essential,
            state,
        };

        Ok(res)
    }
}
