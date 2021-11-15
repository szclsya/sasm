use crate::types::{PkgActionModifier, PkgActions, config::IgnoreRules};
use regex::Regex;
use anyhow::Result;

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
                    return true;
                }
            }
            false
        });
        actions.purge.retain(|pkg| {
            for rule in &self.rules {
                if rule.is_match(&pkg.0) {
                    return true;
                }
            }
            false
        });
    }
}
