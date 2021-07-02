use super::types::PackageMeta;
use super::version::PackageVersion;

use anyhow::{bail, format_err, Result};
use rayon::prelude::*;
use std::collections::HashMap;
use varisat::{lit::Lit, CnfFormula, ExtendFormula};

pub struct PackagePool {
    pkgs: Vec<PackageMeta>,
    // The id of packages for each name, sorted by version
    name_to_ids: HashMap<String, Vec<(usize, PackageVersion)>>,
}

impl PackagePool {
    pub fn new() -> Self {
        PackagePool {
            pkgs: Vec::new(),
            name_to_ids: HashMap::new(),
        }
    }

    #[inline]
    pub fn add(&mut self, meta: PackageMeta) -> usize {
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
        self.name_to_ids.par_iter_mut().for_each(|(_, pkgs)| {
            pkgs.sort_by(|a, b| a.1.cmp(&b.1));
        });
    }

    #[inline]
    pub fn pkg_name_to_ids(&self, name: &str) -> Option<Vec<(usize, PackageVersion)>> {
        self.name_to_ids.get(name).cloned()
    }

    #[inline]
    pub fn id_to_pkg(&self, id: usize) -> Option<(String, PackageVersion)> {
        if id > self.pkgs.len() {
            return None;
        }
        // Since our SAT solver only accepts int > 0 as Literal, we offset pos by 1
        let pos = id - 1;
        let pkg = &self.pkgs[pos];
        Some((pkg.name.clone(), pkg.version.clone()))
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
            let rules = self.pkg_to_rule(pos + 1);
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

    fn pkg_to_rule(&self, pkgid: usize) -> Vec<Vec<Lit>> {
        let pkg = self.pkgs.get(pkgid - 1).unwrap();
        let mut res = Vec::new();
        // Enroll dependencies
        for dep in pkg.depends.iter() {
            let available = match self.name_to_ids.get(&dep.0) {
                Some(d) => d,
                None => {
                    println!("Warning: Cannot find dependency {} for {}", dep.0, pkg.name);
                    continue;
                }
            };

            let mut clause = vec![!Lit::from_dimacs(pkgid as isize)];

            for (dep_pkgid, _) in available {
                let p = &self.pkgs[*dep_pkgid - 1];
                if dep.1.within(&p.version) {
                    clause.push(Lit::from_dimacs(*dep_pkgid as isize));
                }
            }

            if clause.len() > 1 {
                res.push(clause);
            } else {
                println!(
                    "Warning: dependency {} can't be fulfilled for pkg {}",
                    &dep.0, pkg.name
                );
            }
        }

        // Enroll breaks
        for bk in pkg.breaks.iter() {
            let breakable = match self.name_to_ids.get(&bk.0) {
                Some(b) => b,
                None => {
                    continue;
                }
            };

            let mut clause = vec![!Lit::from_dimacs(pkgid as isize)];

            for (bk_pkgid, _) in breakable {
                let p = &self.pkgs[*bk_pkgid - 1];
                if bk.1.within(&p.version) {
                    clause.push(!Lit::from_dimacs(*bk_pkgid as isize));
                }
            }
            if clause.len() > 1 {
                res.push(clause);
            }
        }

        res
    }
}

#[cfg(test)]
mod test {
    use super::super::version::{PackageVersion, VersionRequirement};
    use super::*;

    #[test]
    fn trivial_pool() {
        let mut pool = PackagePool::new();
        let a_id = pool.add(PackageMeta {
            name: "a".to_string(),
            version: PackageVersion::from("1").unwrap(),
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
        });
        let b_id = pool.add(PackageMeta {
            name: "b".to_string(),
            version: PackageVersion::from("1").unwrap(),
            depends: vec![(
                "a".to_string(),
                VersionRequirement {
                    lower_bond: None,
                    upper_bond: None,
                },
            )],
            breaks: Vec::new(),
        });
        let c_id = pool.add(PackageMeta {
            name: "c".to_string(),
            version: PackageVersion::from("1").unwrap(),
            depends: vec![(
                "b".to_string(),
                VersionRequirement {
                    lower_bond: None,
                    upper_bond: None,
                },
            )],
            breaks: Vec::new(),
        });
        let e_id = pool.add(PackageMeta {
            name: "e".to_string(),
            version: PackageVersion::from("1").unwrap(),
            depends: vec![(
                "b".to_string(),
                VersionRequirement {
                    lower_bond: None,
                    upper_bond: None,
                },
            )],
            breaks: Vec::new(),
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
