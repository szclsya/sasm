use super::ExecutionError;
use crate::solver::PackageVersion;

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
    type Error = ExecutionError;
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
                return Err(ExecutionError::StateError("".to_string()));
            }
        };
        Ok(res)
    }
}

/// Status of package on this instance, extracted from dpkg status db
#[derive(Clone)]
pub struct PkgStatus {
    pub name: String,
    pub version: PackageVersion,
    pub state: PkgState,
}

impl TryFrom<&HashMap<&str, String>> for PkgStatus {
    type Error = ExecutionError;

    #[inline]
    fn try_from(f: &HashMap<&str, String>) -> Result<PkgStatus, ExecutionError> {
        let name = f
            .get("Package")
            .ok_or_else(|| {
                ExecutionError::StateError(
                    "Malformed dpkg status db: no Package name for package".to_string(),
                )
            })?
            .to_string();
        let state_line = f.get("Status").ok_or_else(|| {
            ExecutionError::StateError(
                "Malformed dpkg status db: no Status for package".to_string(),
            )
        })?;
        let version = f.get("Version").ok_or_else(|| {
            ExecutionError::StateError(format!(
                "Malformed dpkg status db: no Version for package {}",
                name
            ))
        })?;

        let status: Vec<&str> = state_line.split(' ').collect();
        if status.len() != 3 {
            return Err(ExecutionError::StateError(
                "Malformed dpkg status db".to_string(),
            ));
        }

        let state = PkgState::try_from(*status.get(2).unwrap())?;
        let version = PackageVersion::try_from(version.as_str()).map_err(|err| {
            ExecutionError::StateError(format!(
                "Malformed dpkg status db, cannot parse version: {}",
                err
            ))
        })?;
        let res = PkgStatus {
            name,
            version,
            state,
        };

        Ok(res)
    }
}

/// Single action to modify package state on local instance
#[derive(Debug)]
pub enum PkgAction {
    // Install((PackageName, deb file URL))
    Install(String, String),
    // Upgrade((PackageName, deb file URL))
    // Upgrade is identical for dpkg, differentiate for display purpose
    Upgrade(String, String),
    // Remove(PackageName, Purge?)
    Remove(Vec<String>, bool),
    // Reconfigure(PackageName)
    Reconfigure(String),
}
