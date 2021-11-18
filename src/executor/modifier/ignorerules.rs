use crate::types::{config::IgnoreRules, PkgActionModifier, PkgActions};
use anyhow::Result;

// Apply IgnoreRules so that packages with corresponding names won't be removed
pub struct IgnorePkgs {
    rules: Vec<String>,
}

impl IgnorePkgs {
    pub fn new(ignorerules: &IgnoreRules) -> Result<Self> {
        let res = IgnorePkgs {
            rules: ignorerules.gen_rules()?,
        };
        Ok(res)
    }
}

impl PkgActionModifier for IgnorePkgs {
    fn apply(&self, actions: &mut PkgActions) {
        actions.remove.retain(|pkg| {
            for rule in &self.rules {
                let pkgname = &pkg.0;
                if rule == pkgname {
                    return false;
                }
            }
            true
        });
        actions.purge.retain(|pkg| {
            for rule in &self.rules {
                let pkgname = &pkg.0;
                if rule == pkgname {
                    return false;
                }
            }
            true
        });
    }
}
