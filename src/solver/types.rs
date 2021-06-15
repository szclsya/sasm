use super::version::PackageVersion;

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct VersionRequirement {
    // The bool represents if the restriction is inclusive
    pub lower_bond: Option<(PackageVersion, bool)>,
    pub upper_bond: Option<(PackageVersion, bool)>,
}

impl VersionRequirement {
    pub fn within(&self, ver: &PackageVersion) -> bool {
        if let Some(lower) = &self.lower_bond {
            // If inclusive
            if lower.1 {
                if ver <= &lower.0 {
                    return false;
                }
            } else {
                if ver < &lower.0 {
                    return false;
                }
            }
        }

        if let Some(upper) = &self.upper_bond {
            // If inclusive
            if upper.1 {
                if ver >= &upper.0 {
                    return false;
                }
            } else {
                if ver > &upper.0 {
                    return false;
                }
            }
        }

        true
    }
}

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
