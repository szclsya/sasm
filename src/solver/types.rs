use super::version::{PackageVersion, VersionRequirement};

#[derive(Clone, Debug)]
pub struct PackageMeta {
    pub name: String,
    pub version: PackageVersion,
    pub depends: Vec<(String, VersionRequirement)>,
    pub breaks: Vec<(String, VersionRequirement)>,
    pub conflicts: Vec<(String, VersionRequirement)>,
    pub filename: String,
}
