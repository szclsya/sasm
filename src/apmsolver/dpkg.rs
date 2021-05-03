struct PackageVersion {
    version: usize,
    epoch: Option<usize>,
    revision: usize,
}

enum VersionRequirement {
    Above(PackageVersion),
    Below(PackageVersion),
}

struct PackageMeta {
    name: String,
    description: PackageVersion,
    section: String,
    version: String,
    arch: String,
    size: usize,
    installed_size: usize,
    filename_in_repo: String,
    sha256: String,
    depends: Vec<(String, VersionRequirement)>,
    breaks: Vec<(String, VersionRequirement)>,
}
