use super::types::PkgStatus;
use crate::{debug, error, types::PkgVersion, utils::pacparse};
use anyhow::{anyhow, bail, Context, Result};
use std::{collections::HashMap, path::Path};
use tokio::fs;

const SUPPORTED_ALPM_DB_VERSION: usize = 9;
pub async fn read_alpm_local_db(root: &Path) -> Result<HashMap<String, PkgStatus>> {
    let mut state: HashMap<String, PkgStatus> = HashMap::new();
    // First check ALPM_DB_VERSION
    let alpm_db_ver_path = root.join("ALPM_DB_VERSION");
    let alpm_db_ver: usize =
        if let Some(v) = fs::read_to_string(alpm_db_ver_path).await?.lines().next() {
            v.parse()?
        } else {
            bail!("malformed ALPM DB (no version file)")
        };
    if alpm_db_ver != SUPPORTED_ALPM_DB_VERSION {
        bail!(
            "bad ALPM local database version: expected {}, found {}",
            SUPPORTED_ALPM_DB_VERSION,
            alpm_db_ver
        );
    }

    // Start reading
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry?;
        if entry.path().ends_with("desc") {
            // Parse it
            let content = fs::read_to_string(entry.path()).await?;
            let mut result = pacparse::parse_str(&content)?;
            let name = result.remove("NAME").ok_or_else(|| {
                anyhow!("bad ALPM local db: NAME missing from {}", entry.path().display())
            })?;
            let version: PkgVersion = result
                .remove("VERSION")
                .ok_or_else(|| {
                    anyhow!("bad ALPM local db: NAME missing from {}", entry.path().display())
                })?
                .as_str()
                .try_into()?;
            // SIZE is not mandatory because metapackages
            let install_size: u64 = if let Some(size_str) = result.remove("SIZE") {
                size_str.parse().context(format!(
                    "bad ALPM local db: invalid character in SIZE for {}",
                    entry.path().display()
                ))?
            } else {
                0
            };
            state.insert(name.clone(), PkgStatus { name, version, install_size });
        }
    }
    Ok(state)
}
