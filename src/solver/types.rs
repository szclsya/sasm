use super::version::{PackageVersion, VersionRequirement};

pub struct Request {
    pub install: Vec<(String, VersionRequirement)>,
}

#[derive(Clone, Debug)]
pub struct PackageMeta {
    pub name: String,
    pub version: PackageVersion,
    pub depends: Vec<(String, VersionRequirement)>,
    pub breaks: Vec<(String, VersionRequirement)>,
}

pub struct PackageExtraMeta {
    description: String,
    section: String,
    arch: String,
    size: usize,
    installed_size: usize,
    filename_in_repo: String,
    sha256: String,
}
