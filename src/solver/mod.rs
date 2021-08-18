pub mod deb;
mod improve;
mod incompatible;
mod pool;
mod sort;

use crate::types::{config::Wishlist, PkgMeta, PkgVersion};
use crate::{info, warn};
use anyhow::{bail, format_err, Context, Result};
use pool::PackagePool;
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

    pub fn install(&self, wishlist: &Wishlist) -> Result<Vec<&PkgMeta>> {
        let mut formula = self.pool.gen_formula();
        // Add requested packages to formula
        let mut ids = Vec::new();
        for req in wishlist.get_pkg_requests() {
            let choices: Vec<(usize, PkgVersion)> = match self.pool.pkg_name_to_ids(&req.name) {
                Some(pkgs) => pkgs
                    .iter()
                    .cloned()
                    .filter(|(_, ver)| req.version.within(ver))
                    .collect(),
                None => {
                    bail!("Package {} not found", &req.name);
                }
            };
            let id = choices
                .get(0)
                .ok_or_else(|| format_err!("No suitable version for {}", &req.name))?;
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
        info!("Improving result...");
        improve::upgrade(&self.pool, &mut res, &mut solver)?;
        info!("Reducing result...");
        improve::reduce(&self.pool, &mut res, &ids)?;
        // Sort result
        sort::sort_pkgs(&self.pool, &mut res).context("Failed to sort packages")?;

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
