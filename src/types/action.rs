use super::PkgVersion;
use std::fmt::Display;

#[derive(Default)]
pub struct PkgActions {
    /// Vec<(Name, URL, size, ThisVersion, Option<OlderVersion>)
    pub install: Vec<(String, String, u64, PkgVersion, Option<PkgVersion>)>,
    pub remove: Vec<String>,
    pub purge: Vec<String>,
    pub configure: Vec<String>,
}

impl PkgActions {
    pub fn is_empty(&self) -> bool {
        self.install.is_empty()
            && self.remove.is_empty()
            && self.purge.is_empty()
            && self.configure.is_empty()
    }
}
