use super::version::PackageVersion;

#[derive(PartialEq, Eq)]
pub enum VersionRequirement {
    Above(PackageVersion),
    Below(PackageVersion),
    Any,
}

pub struct Request {
    pub install: Vec<(String, VersionRequirement)>,
}

pub struct PackageMeta {
    pub version: PackageVersion,
    pub depends: Vec<(String, VersionRequirement)>,
    pub breaks: Vec<(String, VersionRequirement)>,
}

struct PackageExtraMeta {
    description: String,
    section: String,
    arch: String,
    size: usize,
    installed_size: usize,
    filename_in_repo: String,
    sha256: String,
}
