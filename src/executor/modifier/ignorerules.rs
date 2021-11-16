use crate::types::{config::IgnoreRules, PkgActionModifier, PkgActions};
use anyhow::Result;
use regex::Regex;

// Apply IgnoreRules so that packages with corresponding names won't be removed
pub struct IgnorePkgs {
    rules: Vec<Regex>,
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
                if rule.is_match(&pkg.0) {
                    return false;
                }
            }
            true
        });
        actions.purge.retain(|pkg| {
            for rule in &self.rules {
                if rule.is_match(&pkg.0) {
                    return false;
                }
            }
            true
        });
    }
}
