use crate::types::{PkgMeta, PkgVersion};

use crate::warn;
use anyhow::{bail, format_err, Result};
use std::collections::HashMap;
use varisat::{lit::Lit, CnfFormula, ExtendFormula};

pub struct PackagePool {
    pkgs: Vec<PkgMeta>,
    // The id of packages for each name, sorted by version
    name_to_ids: HashMap<String, Vec<(usize, PkgVersion)>>,
}

impl PackagePool {
    pub fn new() -> Self {
        PackagePool {
            pkgs: Vec::new(),
            name_to_ids: HashMap::new(),
        }
    }

    #[inline]
    pub fn add(&mut self, meta: PkgMeta) -> usize {
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

    #[inline]
    pub fn finalize(&mut self) {
        // Sort versions
        self.name_to_ids.iter_mut().for_each(|(_, pkgs)| {
            // Sort in descending order
            pkgs.sort_by(|a, b| b.1.cmp(&a.1));
        });
    }

    #[inline]
    pub fn get_pkgs_by_name(&self, name: &str) -> Option<Vec<(usize, PkgVersion)>> {
        self.name_to_ids.get(name).cloned()
    }

    #[inline]
    pub fn get_pkg_by_id(&self, id: usize) -> Option<&PkgMeta> {
        if id > self.pkgs.len() {
            return None;
        }
        // Since our SAT solver only accepts int > 0 as Literal, we offset pos by 1
        let pos = id - 1;
        let pkg = &self.pkgs[pos];
        Some(pkg)
    }

    pub fn get_deps(&self, pkgid: usize) -> Result<Vec<Vec<usize>>> {
        let pkg = self
            .pkgs
            .get(pkgid - 1)
            .ok_or_else(|| format_err!("Package with ID {} not found", pkgid))?;
        let mut res = Vec::new();
        for dep in pkg.depends.iter() {
            let mut deps_id = Vec::new();
            let available = match self.name_to_ids.get(&dep.0) {
                Some(d) => d,
                None => {
                    bail!("Warning: Cannot find dependency {} for {}", dep.0, pkg.name);
                }
            };
            for (dep_pkgid, _) in available {
                let p = &self.pkgs[*dep_pkgid - 1];
                if dep.1.within(&p.version) {
                    deps_id.push(*dep_pkgid);
                }
            }
            if deps_id.is_empty() {
                bail!(
                    "Warning: dependency {} can't be fulfilled for pkg {}",
                    &dep.0,
                    pkg.name
                );
            } else {
                res.push(deps_id);
            }
        }
        Ok(res)
    }

    pub fn gen_formula(&self) -> CnfFormula {
        let mut formula = CnfFormula::new();
        // Generate rules for each individual package
        for (pos, _) in self.pkgs.iter().enumerate() {
            let rules = self.pkg_to_rule(pos + 1, None);
            for rule in rules {
                formula.add_clause(&rule);
            }
        }
        // Generate conflict for different versions of the same package
        for versions in self.name_to_ids.values() {
            if versions.len() > 1 {
                let mut clause = Vec::new();
                for pkg in versions {
                    clause.push(!Lit::from_dimacs(pkg.0 as isize));
                }
                formula.add_clause(&clause);
            }
        }
        formula
    }

    pub fn gen_subset_formula(&self, ids: &[usize]) -> CnfFormula {
        let mut formula = CnfFormula::new();
        for id in ids {
            let rules = self.pkg_to_rule(*id, Some(ids));
            for rule in rules {
                formula.add_clause(&rule);
            }
        }
        // Generate conflict for different versions of the same package
        for versions in self.name_to_ids.values() {
            let versions: Vec<usize> = versions
                .iter()
                .filter(|pkg| ids.contains(&pkg.0))
                .map(|pkg| pkg.0)
                .collect();
            if versions.len() > 1 {
                let mut clause = Vec::new();
                for pkgid in versions {
                    clause.push(!Lit::from_dimacs(pkgid as isize));
                }
                formula.add_clause(&clause);
            }
        }
        formula
    }

    fn pkg_to_rule(&self, pkgid: usize, subset: Option<&[usize]>) -> Vec<Vec<Lit>> {
        let pkg = self.pkgs.get(pkgid - 1).unwrap();
        let mut res = Vec::new();
        // Enroll dependencies
        for dep in pkg.depends.iter() {
            let available = match self.name_to_ids.get(&dep.0) {
                Some(pkgs) => match subset {
                    Some(ids) => {
                        let pkgs: Vec<usize> = pkgs
                            .iter()
                            .filter(|(id, _)| ids.contains(id))
                            .map(|pkg| pkg.0)
                            .collect();
                        pkgs
                    }
                    None => pkgs.iter().map(|pkg| pkg.0).collect(),
                },
                None => {
                    warn!("Cannot find dependency {} for {}", dep.0, pkg.name);
                    continue;
                }
            };

            let mut clause = vec![!Lit::from_dimacs(pkgid as isize)];

            for dep_pkgid in available {
                let p = &self.pkgs[dep_pkgid - 1];
                if dep.1.within(&p.version) {
                    clause.push(Lit::from_dimacs(dep_pkgid as isize));
                }
            }

            if clause.len() > 1 {
                res.push(clause);
            } else {
                warn!(
                    "Dependency {} can't be fulfilled for pkg {}",
                    &dep.0, pkg.name
                );
            }
        }

        // Enroll breaks
        for bk in pkg.breaks.iter() {
            let breakable = match self.name_to_ids.get(&bk.0) {
                Some(pkgs) => match subset {
                    Some(ids) => {
                        let pkgs: Vec<usize> = pkgs
                            .iter()
                            .filter(|(id, _)| ids.contains(id))
                            .map(|pkg| pkg.0)
                            .collect();
                        pkgs
                    }
                    None => pkgs.iter().map(|pkg| pkg.0).collect(),
                },
                None => {
                    continue;
                }
            };

            for bk_pkgid in breakable {
                let p = &self.pkgs[bk_pkgid - 1];
                if bk.1.within(&p.version) {
                    let clause = vec![
                        !Lit::from_dimacs(pkgid as isize),
                        !Lit::from_dimacs(bk_pkgid as isize),
                    ];
                    res.push(clause);
                }
            }
        }

        // Enroll conflicts
        for conflict in pkg.conflicts.iter() {
            let conflicable = match self.name_to_ids.get(&conflict.0) {
                Some(pkgs) => match subset {
                    Some(ids) => {
                        let pkgs: Vec<usize> = pkgs
                            .iter()
                            .filter(|(id, _)| ids.contains(id))
                            .map(|pkg| pkg.0)
                            .collect();
                        pkgs
                    }
                    None => pkgs.iter().map(|pkg| pkg.0).collect(),
                },
                None => {
                    continue;
                }
            };

            for conflict_pkgid in conflicable {
                let p = &self.pkgs[conflict_pkgid - 1];
                if conflict.1.within(&p.version) {
                    let clause = vec![
                        !Lit::from_dimacs(pkgid as isize),
                        !Lit::from_dimacs(conflict_pkgid as isize),
                    ];
                    res.push(clause);
                }
            }
        }

        res
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::types::{Checksum, PkgMeta, PkgVersion, VersionRequirement};
    use std::convert::TryFrom;

    #[test]
    fn trivial_pool() {
        let mut pool = PackagePool::new();
        let a_id = pool.add(PkgMeta {
            name: "a".to_string(),
            version: PkgVersion::try_from("1").unwrap(),
            depends: vec![(
                "c".to_string(),
                VersionRequirement {
                    lower_bond: None,
                    upper_bond: None,
                },
            )],
            breaks: vec![(
                "d".to_string(),
                VersionRequirement {
                    lower_bond: None,
                    upper_bond: None,
                },
            )],
            conflicts: Vec::new(),
            install_size: 0,
            url: String::new(),
            size: 0,
            checksum: Checksum::from_sha256_str(&str::repeat("a", 64)).unwrap(),
        });
        let b_id = pool.add(PkgMeta {
            name: "b".to_string(),
            version: PkgVersion::try_from("1").unwrap(),
            depends: vec![(
                "a".to_string(),
                VersionRequirement {
                    lower_bond: None,
                    upper_bond: None,
                },
            )],
            breaks: Vec::new(),
            conflicts: Vec::new(),
            install_size: 0,
            url: String::new(),
            size: 0,
            checksum: Checksum::from_sha256_str(&str::repeat("a", 64)).unwrap(),
        });
        let c_id = pool.add(PkgMeta {
            name: "c".to_string(),
            version: PkgVersion::try_from("1").unwrap(),
            depends: vec![(
                "b".to_string(),
                VersionRequirement {
                    lower_bond: None,
                    upper_bond: None,
                },
            )],
            breaks: Vec::new(),
            conflicts: Vec::new(),
            install_size: 0,
            url: String::new(),
            size: 0,
            checksum: Checksum::from_sha256_str(&str::repeat("a", 64)).unwrap(),
        });
        let e_id = pool.add(PkgMeta {
            name: "e".to_string(),
            version: PkgVersion::try_from("1").unwrap(),
            depends: vec![(
                "b".to_string(),
                VersionRequirement {
                    lower_bond: None,
                    upper_bond: None,
                },
            )],
            breaks: Vec::new(),
            conflicts: Vec::new(),
            install_size: 0,
            url: String::new(),
            size: 0,
            checksum: Checksum::from_sha256_str(&str::repeat("a", 64)).unwrap(),
        });
        pool.finalize();

        let mut solver = varisat::Solver::new();
        let formula = pool.gen_formula();
        solver.add_formula(&formula);
        solver.add_clause(&[Lit::from_dimacs(c_id as isize)]);

        solver.solve().unwrap();
        assert_eq!(
            solver.model().unwrap(),
            vec![
                Lit::from_dimacs(a_id as isize),
                Lit::from_dimacs(b_id as isize),
                Lit::from_dimacs(c_id as isize),
                !Lit::from_dimacs(e_id as isize),
            ]
        );
    }
}
