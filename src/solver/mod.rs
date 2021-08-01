pub mod deb;
mod improve;
mod incompatible;
mod pool;
mod sort;

use crate::types::{PkgMeta, PkgVersion, VersionRequirement};
use crate::warn;
use anyhow::{bail, format_err, Context, Result};
use pool::PackagePool;
use std::collections::HashMap;
use varisat::{lit::Lit, ExtendFormula};

pub struct Solver {
    pub pool: PackagePool,
}

impl Solver {
    pub fn new() -> Self {
        Solver {
            pool: PackagePool::new(),
        }
    }

    pub fn finalize(&mut self) {
        self.pool.finalize();
    }

    pub fn install(
        &self,
        to_install: HashMap<String, VersionRequirement>,
    ) -> Result<Vec<&PkgMeta>> {
        let mut formula = self.pool.gen_formula();
        // Add requested packages to formula
        let mut ids = Vec::new();
        for (name, ver_req) in to_install {
            let choices: Vec<(usize, PkgVersion)> = match self.pool.pkg_name_to_ids(&name) {
                Some(pkgs) => pkgs
                    .iter()
                    .cloned()
                    .filter(|(_, ver)| ver_req.within(ver))
                    .collect(),
                None => {
                    bail!("Package {} not found", &name);
                }
            };
            let id = choices
                .get(0)
                .ok_or_else(|| format_err!("No suitable version for {}", &name))?;
            formula.add_clause(&[Lit::from_dimacs(id.0 as isize)]);
            ids.push(id.0);
        }
        // Add rules to solver
        let mut solver = varisat::Solver::new();
        solver.add_formula(&formula);

        // Initial solve
        let mut res = match solve(&mut solver) {
            Ok(r) => r,
            Err(_) => {
                return Err(format_err!(incompatible::find_incompatible_friendly(
                    &self.pool, &ids
                )))
                .context("Cannot satisfy package requirements")
            }
        };

        // Improve the result to remove redundant packages
        // and select best possible packages
        improve::improve(&self.pool, &mut res, &mut solver)?;
        improve::reduced_upgrade(&self.pool, &mut res, &ids)?;
        // Sort result
        sort::sort_pkgs(&self.pool, &mut res).unwrap();

        // Generate result
        let pkgs: Vec<&PkgMeta> = res
            .into_iter()
            .map(|pkgid| {
                let res = self.pool.id_to_pkg(pkgid).unwrap();
                if !improve::is_best(&self.pool, pkgid).unwrap() {
                    warn!("Cannot select best version of {}", res.name);
                }
                res
            })
            .collect();

        Ok(pkgs)
    }
}

/// Helper function to get PkgID list
pub fn solve(solver: &mut varisat::Solver) -> Result<Vec<usize>> {
    let mut res = Vec::new();
    if !solver.solve().unwrap() {
        bail!("Cannot satisfy package requirements");
    } else {
        let model = solver.model().unwrap();
        for i in model {
            if i.is_positive() {
                let id = i.to_dimacs() as usize;
                res.push(id);
            }
        }
    }
    Ok(res)
}
