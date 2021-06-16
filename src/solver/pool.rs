use super::types::{PackageExtraMeta, PackageMeta};
use super::version::PackageVersion;

use anyhow::{bail, Result};
use std::collections::HashMap;
use varisat::{
    CnfFormula, ExtendFormula, Var,
    {lit::Lit, solver::Solver},
};

pub struct PackagePool {
    pkgs: Vec<(String, PackageMeta)>,
    // The id of packages for each name
    // The first item is the latest, the rest is not sorted
    name_to_ids: HashMap<String, Vec<usize>>,
}

impl PackagePool {
    pub fn new() -> Self {
        PackagePool {
            pkgs: Vec::new(),
            name_to_ids: HashMap::new(),
        }
    }

    pub fn add(&mut self, meta: PackageMeta) -> usize {
        let name = meta.name.clone();
        let this_version = meta.version.clone();
        self.pkgs.push((name.to_string(), meta));
        let index = self.pkgs.len();

        if self.name_to_ids.contains_key(&name) {
            let ids = self.name_to_ids.get_mut(&name).unwrap();
            if !ids.is_empty() && self.pkgs[ids[0]].1.version < this_version {
                ids.insert(0, index);
            } else {
                ids.push(index);
            }
        } else {
            self.name_to_ids.insert(name, Vec::from([index]));
        }

        index
    }

    pub fn pkg_name_to_ids(&self, name: &str) -> Vec<(usize, PackageVersion)> {
        let mut res: Vec<(usize, PackageVersion)> = Vec::new();
        if let Some(pkgs) = self.name_to_ids.get(name) {
            for pkg in pkgs {
                res.push((*pkg, self.pkgs[*pkg].1.version.clone()))
            }
        }
        res
    }

    pub fn id_to_pkg(&self, id: usize) -> Result<(String, PackageVersion)> {
        if id > self.pkgs.len() {
            bail!("ID does not exist");
        }
        // Since our SAT solver only accepts int > 0 as Literal, we offset pos by 1
        let pos = id - 1;
        let pkg = &self.pkgs[pos];
        Ok((pkg.0.clone(), pkg.1.version.clone()))
    }

    pub fn add_rules_to_solver(&self, solver: &mut Solver) {
        for (pos, pkg) in self.pkgs.iter().enumerate() {
            let formula = self.pkg_to_rule(&pkg.1, pos + 1);
            solver.add_formula(&formula);
        }
    }

    fn pkg_to_rule(&self, pkg: &PackageMeta, pkgid: usize) -> CnfFormula {
        let mut formula = CnfFormula::new();
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

            for dep_pkgid in available {
                let p = &self.pkgs[*dep_pkgid - 1];
                if dep.1.within(&p.1.version) {
                    clause.push(Lit::from_dimacs(*dep_pkgid as isize));
                }
            }

            if clause.len() > 1 {
                formula.add_clause(clause.as_slice());
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

            for dep_pkgid in breakable {
                let p = &self.pkgs[*dep_pkgid - 1];
                if bk.1.within(&p.1.version) {
                    clause.push(!Lit::from_dimacs(*dep_pkgid as isize));
                }
            }
            if clause.len() > 1 {
                formula.add_clause(clause.as_slice());
            }
        }

        formula
    }
}

#[cfg(test)]
mod test {
    use super::super::types::VersionRequirement;
    use super::super::version::PackageVersion;
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
            breaks: Vec::new(),
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

        let mut solver = Solver::new();
        pool.add_rules_to_solver(&mut solver);
        solver.add_clause(&[Lit::from_dimacs(c_id as isize)]);

        solver.solve().unwrap();
        assert_eq!(
            solver.model().unwrap(),
            vec![
                Lit::from_dimacs(a_id as isize),
                Lit::from_dimacs(b_id as isize),
                Lit::from_dimacs(c_id as isize),
            ]
        );
    }
}
