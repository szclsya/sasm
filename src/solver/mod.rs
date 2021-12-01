pub mod deb;
mod improve;
mod incompatible;
mod pool;
mod sort;

use crate::types::{config::Blueprints, PkgMeta};
use crate::{debug, warn};
use anyhow::{bail, format_err, Context, Result};
use pool::{InMemoryPool, PkgPool};
use varisat::{lit::Lit, ExtendFormula};

pub struct Solver {
    pub pool: Box<dyn PkgPool>,
}

impl Solver {
    pub fn new() -> Self {
        Solver {
            pool: Box::new(InMemoryPool::new()),
        }
    }

    pub fn finalize(&mut self) {
        self.pool.finalize();
    }

    pub fn install(&self, blueprints: &Blueprints) -> Result<Vec<&PkgMeta>> {
        let mut formula = self.pool.gen_formula(None);
        debug!("Adding requested packages to formula...");
        let mut ids = Vec::new();
        for req in blueprints.get_pkg_requests() {
            let id = self.pool.pick_best_pkg(&req.name, &req.version)?;
            formula.add_clause(&[Lit::from_dimacs(id as isize)]);
            ids.push(id);
        }
        // Add rules to solver
        let mut solver = varisat::Solver::new();
        solver.add_formula(&formula);

        // Initial solve
        debug!("Initial solve");
        let mut res = match solve(&mut solver) {
            Ok(r) => r,
            Err(_) => {
                return Err(format_err!(incompatible::find_incompatible_friendly(
                    self.pool.as_ref(),
                    &ids
                )))
                .context("Cannot satisfy package requirements")
            }
        };

        // Improve the result to remove redundant packages
        // and select best possible packages
        debug!("Improving dependency tree...");
        improve::upgrade(self.pool.as_ref(), &mut res, &mut solver)?;
        improve::reduce(self.pool.as_ref(), &mut res, &ids)?;
        // Sort result
        sort::sort_pkgs(self.pool.as_ref(), &mut res).context("Failed to sort packages")?;

        // Generate result
        let pkgs: Vec<&PkgMeta> = res
            .into_iter()
            .map(|pkgid| {
                let res = self.pool.get_pkg_by_id(pkgid).unwrap();
                if !improve::is_best(self.pool.as_ref(), pkgid).unwrap() {
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
