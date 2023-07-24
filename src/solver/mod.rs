/// The sasm dependency solver
/// Comes with the pool it uses to calculate dependencies upon
pub mod pool;

mod improve;
mod incompatible;
mod sort;

use crate::debug;
use crate::pool::PkgPool;
use crate::types::{config::Blueprints, PkgMeta};
use anyhow::{bail, format_err, Context, Result};
use varisat::{lit::Lit, ExtendFormula};

pub struct Solver {
    pub pool: Box<dyn PkgPool>,
}

impl From<Box<dyn PkgPool>> for Solver {
    fn from(pool: Box<dyn PkgPool>) -> Self {
        Solver { pool }
    }
}

impl Solver {
    pub fn install(&self, blueprints: &Blueprints) -> Result<Vec<&PkgMeta>> {
        let mut formula = self.pool.gen_formula(None);
        debug!("Adding requested packages to solver formula...");
        let mut ids = Vec::new();
        for req in blueprints.get_pkg_requests() {
            let id = self.pool.pick_best_pkg(&req.name, &req.version, req.local)?;
            formula.add_clause(&[Lit::from_dimacs(id as isize)]);
            ids.push(id);
        }
        // Add rules to solver
        let mut solver = varisat::Solver::new();
        solver.add_formula(&formula);

        // Initial solve
        debug!("Computing initial solution...");
        let mut res = match solve(&mut solver) {
            Ok(r) => r,
            Err(_) => {
                return Err(format_err!(incompatible::find_incompatible_friendly(
                    self.pool.as_ref(),
                    &ids
                )))
                .context("sasm cannot satisfy package requirements.")
            }
        };

        // Improve the result to remove redundant packages
        // and select best possible packages
        debug!("Refining dependency solution...");
        improve::upgrade(self.pool.as_ref(), &mut res, &mut solver)?;
        improve::reduce(self.pool.as_ref(), &mut res, &ids)?;
        // Sort result
        sort::sort_pkgs(self.pool.as_ref(), &mut res).context("Failed to sort packages")?;

        // Generate result
        let pkgs: Vec<&PkgMeta> =
            res.into_iter().map(|pkgid| self.pool.get_pkg_by_id(pkgid).unwrap()).collect();

        Ok(pkgs)
    }
}

/// Helper function to get PkgID list
pub fn solve(solver: &mut varisat::Solver) -> Result<Vec<usize>> {
    let mut res = Vec::new();
    if !solver.solve().unwrap() {
        bail!("sasm cannot satisfy package requirements.");
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

