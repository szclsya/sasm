use super::{tools::pkg_to_rule, BasicPkgPool, PkgPool};
use crate::types::{PkgMeta, PkgVersion};
use crate::warn;

use rayon::prelude::*;
use std::collections::HashMap;
use varisat::{lit::Lit, CnfFormula, ExtendFormula};

pub struct InMemoryPool {
    pkgs: Vec<PkgMeta>,
    // The id of packages for each name, sorted by version
    name_to_ids: HashMap<String, Vec<(usize, PkgVersion)>>,
}

impl InMemoryPool {
    pub fn new() -> Self {
        InMemoryPool {
            pkgs: Vec::new(),
            name_to_ids: HashMap::new(),
        }
    }
}

impl BasicPkgPool for InMemoryPool {
    fn add(&mut self, meta: PkgMeta) -> usize {
        let name = meta.name.clone();
        let version = meta.version.clone();
        self.pkgs.push(meta);
        let index = self.pkgs.len();

        if self.name_to_ids.contains_key(&name) {
            let ids = self.name_to_ids.get_mut(&name).unwrap();
            ids.push((index, version));
        } else {
            self.name_to_ids.insert(name, Vec::from([(index, version)]));
        }

        index
    }

    fn finalize(&mut self) {
        // Sort versions
        self.name_to_ids.iter_mut().for_each(|(_, pkgs)| {
            // Sort in descending order
            pkgs.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        });
    }

    fn get_pkg_by_id(&self, id: usize) -> Option<&PkgMeta> {
        if id > self.pkgs.len() {
            return None;
        }
        // Since our SAT solver only accepts int > 0 as Literal, we offset pos by 1
        let pos = id - 1;
        let pkg = &self.pkgs[pos];
        Some(pkg)
    }

    fn get_pkgs_by_name(&self, name: &str) -> Option<Vec<usize>> {
        match self.name_to_ids.get(name) {
            Some(pkgs) => {
                let res: Vec<usize> = pkgs.iter().map(|(pkgid, _)| *pkgid).collect();
                Some(res)
            }
            None => None,
        }
    }

    fn gen_formula(&self, subset: Option<&[usize]>) -> CnfFormula {
        let ids = match subset {
            Some(ids) => ids.to_vec(),
            None => (1..self.pkgs.len()).collect(),
        };
        // Generating rules from pool
        let mut rules: Vec<Vec<Lit>> = ids
            .par_iter()
            .filter_map(|id| match pkg_to_rule(self, *id, Some(&ids)) {
                Ok(rules) => Some(rules),
                Err(e) => {
                    let pkg = self.get_pkg_by_id(*id).unwrap();
                    warn!("Ignoring package {} due to: {}", pkg.name, e);
                    None
                }
            })
            .flatten()
            .collect();

        // Generate conflict for different versions of the same package
        let conflict_rules: Vec<Vec<Lit>> = self
            .name_to_ids
            .par_iter()
            .filter_map(|(_, versions)| {
                let versions: Vec<usize> = versions
                    .iter()
                    .filter(|pkg| ids.contains(&pkg.0))
                    .map(|pkg| pkg.0)
                    .collect();
                if versions.len() > 1 {
                    let clause: Vec<Lit> = versions
                        .into_iter()
                        .map(|pkgid| !Lit::from_dimacs(pkgid as isize))
                        .collect();
                    Some(clause)
                } else {
                    None
                }
            })
            .collect();

        // Combine rule sets
        rules.extend(conflict_rules);

        let mut formula = CnfFormula::new();
        // Add generated rules to formula
        for rule in rules {
            formula.add_clause(&rule);
        }
        formula
    }
}

impl PkgPool for InMemoryPool {}
