use super::types::PackageMeta;
use super::version::PackageVersion;
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

    pub fn add(&mut self, name: &str, meta: PackageMeta) -> usize {
        let this_version = meta.version.clone();
        self.pkgs.push((name.to_string(), meta.clone()));
        let index = self.pkgs.len() - 1;

        if self.name_to_ids.contains_key(name) {
            let ids = self.name_to_ids.get_mut(name).unwrap();
            if !ids.is_empty() && self.pkgs[ids[0]].1.version < this_version {
                ids.insert(0, index);
            } else {
                ids.push(index);
            }
        } else {
            self.name_to_ids
                .insert(name.to_string(), Vec::from([index]));
        }

        index
    }

    pub fn get_ids(&self, name: &str) -> Vec<(usize, PackageMeta)> {
        let mut res: Vec<(usize, PackageMeta)> = Vec::new();

        for (pos, pkg) in self.pkgs.iter().enumerate() {
            if pkg.0 == name {
                res.push((pos, pkg.1.clone()));
            }
        }
        res
    }

    pub fn to_solver(&self) -> Solver {
        let mut solver = Solver::new();
        for (pos, pkg) in self.pkgs.iter().enumerate() {
            let formula = self.pkg_to_rule(&pkg.1, pos);
            solver.add_formula(&formula);
        }
        solver
    }

    fn pkg_to_rule(&self, pkg: &PackageMeta, pkgid: usize) -> CnfFormula {
        let mut formula = CnfFormula::new();
        // Enroll dependencies
        for dep in pkg.depends.iter() {
            let mut clause = Vec::new();
            clause.push(!Lit::from_dimacs(pkgid as isize));

            let available = &self.name_to_ids[&dep.0];
            for dep_pkgid in available {
                let p = &self.pkgs[*dep_pkgid];
                if dep.1.within(&p.1.version) {
                    clause.push(Lit::from_dimacs(*dep_pkgid as isize));
                }
            }
            formula.add_clause(clause.as_slice());
        }

        // Enroll breaks
        for bk in pkg.breaks.iter() {
            let mut clause = Vec::new();
            clause.push(!Lit::from_dimacs(pkgid as isize));

            let breakable = &self.name_to_ids[&bk.0];
            for dep_pkgid in breakable {
                let p = &self.pkgs[*dep_pkgid];
                if bk.1.within(&p.1.version) {
                    clause.push(!Lit::from_dimacs(*dep_pkgid as isize));
                }
            }
            formula.add_clause(clause.as_slice());
        }

        formula
    }
}
