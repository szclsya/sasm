mod ord;
mod parse;
mod requirement;
mod test;
pub use parse::parse_version;
pub use requirement::{parse_version_requirement, VersionRequirement};

use serde::{Deserialize, Serialize, Serializer};
use std::fmt;

#[derive(PartialEq, Eq, Clone, Debug, Deserialize)]
pub enum PkgVersionSegment {
    Number(u64),
    Alphabetic(String),
    Separater(String),
}

/// RPM style package version comparison
#[derive(PartialEq, Eq, Clone, Debug, Deserialize)]
pub struct PkgVersion {
    pub epoch: u64,
    pub version: Vec<PkgVersionSegment>,
    pub revision: Option<u64>,
}

impl fmt::Display for PkgVersionSegment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Number(x) => write!(f, "{x}")?,
            Self::Alphabetic(x) => write!(f, "{x}")?,
            Self::Separater(x) => write!(f, "{x}")?,
        }
        Ok(())
    }
}

impl fmt::Display for PkgVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.epoch != 0 {
            write!(f, "{}:", self.epoch)?;
        }
        for segment in &self.version {
            write!(f, "{}", &segment)?;
        }
        if let Some(rev) = self.revision {
            write!(f, "-{}", rev)?;
        }
        Ok(())
    }
}

impl Serialize for PkgVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let res = self.to_string();
        serializer.serialize_str(&res)
    }
}
