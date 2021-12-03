mod action;
mod checksum;
pub mod config;
mod version;

pub use action::{PkgActionModifier, PkgActions, PkgInstallAction};
pub use checksum::Checksum;
pub use version::{parse_version, parse_version_requirement, PkgVersion, VersionRequirement};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Default)]
pub struct PkgRequirement {
    pub with_recommends: Option<bool>,
    pub version: Option<VersionRequirement>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PkgMeta {
    pub name: String,
    pub version: PkgVersion,
    pub depends: Vec<(String, VersionRequirement)>,
    pub breaks: Vec<(String, VersionRequirement)>,
    pub conflicts: Vec<(String, VersionRequirement)>,
    pub install_size: u64,
    pub url: String,
    // u64 because reqwest's content length is u64
    pub size: u64,
    pub checksum: Checksum,
    pub recommends: Option<Vec<(String, VersionRequirement)>>,
}
