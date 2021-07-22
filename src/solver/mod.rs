pub mod deb;
mod pool;
mod sort;

pub use crate::types::{PkgMeta, PkgRequirement, PkgVersion, VersionRequirement};
use anyhow::format_err;
use pool::PackagePool;
use std::collections::HashMap;
use thiserror::Error;
use varisat::{lit::Lit, ExtendFormula};

#[derive(Error, Clone, Debug)]
pub enum SolverError {
    #[error("Failed to satisfy package wishlist: {0}")]
    Unsolvable(String),
    #[error("Internal solver error: {0}")]
    InternalError(String),
}

impl From<anyhow::Error> for SolverError {
    fn from(e: anyhow::Error) -> Self {
        SolverError::InternalError(e.to_string())
    }
}

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
    ) -> Result<Vec<&PkgMeta>, SolverError> {
        let mut formula = self.pool.gen_formula();
        // Add requested packages to formula
        for (name, ver_req) in to_install {
            let choices: Vec<(usize, PkgVersion)> = match self.pool.pkg_name_to_ids(&name) {
                Some(pkgs) => pkgs
                    .iter()
                    .cloned()
                    .filter(|(_, ver)| ver_req.within(ver))
                    .collect(),
                None => {
                    return Err(SolverError::Unsolvable(format!(
                        "Package {} not found",
                        &name
                    )));
                }
            };
            let id = choices
                .get(0)
                .ok_or_else(|| format_err!("No suitable version for {}", &name))?;
            formula.add_clause(&[Lit::from_dimacs(id.0 as isize)]);
        }
        // Add rules to solver
        let mut solver = varisat::Solver::new();
        solver.add_formula(&formula);

        // Initial solve
        let mut res = solve(&mut solver)?;

        // Upgrade possible packages
        let mut reducable: Vec<Lit> = Vec::new();
        loop {
            let mut new_older = gen_update_assume(&self.pool, &res);
            if new_older.is_empty() {
                // All packages are up to date!
                break;
            }
            reducable.append(&mut new_older);
            solver.assume(&reducable);
            if !solver.solve().unwrap() {
                // It's not possible to improve any further
                break;
            } else {
                res = solve(&mut solver).unwrap();
            }
        }

        // Reduce redundant dependencies
        for pkg in &res {
            let previous_non_latest = gen_update_assume(&self.pool, &res).len();
            // See if this pkg can be reduced
            reducable.push(!Lit::from_dimacs(*pkg as isize));
            solver.assume(&reducable);
            if !solver.solve().unwrap() {
                // Not solvable, this pkg is required
                reducable.pop();
            } else {
                // Is reducible. See if non_latest pkgs increases
                let new_res = solve(&mut solver).unwrap();
                let new_non_latest = gen_update_assume(&self.pool, &new_res).len();
                if new_non_latest > previous_non_latest {
                    // The reduction introduces more non-latest pkgs, don't do it!
                    reducable.pop();
                }
            }
        }

        // Update again
        loop {
            let mut new_older = gen_update_assume(&self.pool, &res);
            if new_older.is_empty() {
                // All packages are up to date!
                break;
            }
            reducable.append(&mut new_older);
            solver.assume(&reducable);
            if !solver.solve().unwrap() {
                // It's not possible to improve any further
                break;
            } else {
                res = solve(&mut solver).unwrap();
            }
        }

        // Sort result
        sort::sort_pkgs(&self.pool, &mut res).unwrap();

        // Generate result
        let pkgs: Vec<&PkgMeta> = res
            .into_iter()
            .map(|pkgid| {
                let res = self.pool.id_to_pkg(pkgid).unwrap();
                if !is_up_to_date(&self.pool, pkgid).unwrap() {
                    println!("{} not latest!", res.name);
                }
                res
            })
            .collect();

        Ok(pkgs)
    }
}

fn solve(solver: &mut varisat::Solver) -> Result<Vec<usize>, SolverError> {
    let mut res = Vec::new();
    if !solver.solve().unwrap() {
        return Err(SolverError::Unsolvable(
            "Cannot satisfy requirements".to_string(),
        ));
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

/// Generate a list of Lit of all older packages
/// The idea is that with these assumptions, the SAT solver must choose more up-to-date
///   packages, or give Unsolvable
fn gen_update_assume(pool: &PackagePool, ids: &[usize]) -> Vec<Lit> {
    let mut res = Vec::new();
    for id in ids {
        if !is_up_to_date(pool, *id).unwrap() {
            // Find all newer versions of this package
            let name = &pool.id_to_pkg(*id).unwrap().name;
            let pkgids: Vec<usize> = pool
                .pkg_name_to_ids(name)
                .unwrap()
                .into_iter()
                .map(|pkg| pkg.0)
                .collect();

            let mut reached = false;
            for pkgid in pkgids {
                if pkgid == *id {
                    reached = true
                }
                if reached {
                    reached = true;
                    let lit = !Lit::from_dimacs(pkgid as isize);
                    res.push(lit);
                }
            }
        }
    }
    res
}

#[inline]
fn is_up_to_date(pool: &PackagePool, id: usize) -> Option<bool> {
    let name = &pool.id_to_pkg(id)?.name;
    let ids = pool.pkg_name_to_ids(name)?;
    if ids[0].0 != id {
        Some(false)
    } else {
        Some(true)
    }
}
