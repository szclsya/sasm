mod deb;
mod pool;
mod types;
mod version;

use std::path::PathBuf;
use varisat::{lit::Lit, CnfFormula, ExtendFormula};

use pool::PackagePool;
use version::PackageVersion;

#[derive(Clone, Debug)]
pub enum SolverError {
    Unsolvable(String),
    DatabaseInitError(String),
    InternalError(String),
}

impl From<anyhow::Error> for SolverError {
    fn from(e: anyhow::Error) -> Self {
        SolverError::InternalError(e.to_string())
    }
}

pub struct Solver {
    pool: PackagePool,
}

impl Solver {
    pub fn from_dpkg_dbs(dbs: &[PathBuf]) -> Result<Self, SolverError> {
        let mut pool = PackagePool::new();

        for db_path in dbs {
            deb::read_deb_db(db_path, &mut pool)?;
        }

        pool.finalize();
        Ok(Solver { pool })
    }

    pub fn install(
        &self,
        to_install: &[String],
    ) -> Result<Vec<(String, PackageVersion)>, SolverError> {
        let mut formula = self.pool.gen_formula();
        // Add requested packages to formula 
        for pkg in to_install {
            let choices = match self.pool.pkg_name_to_ids(pkg) {
                Some(pkgs) => pkgs,
                None => {
                    return Err(SolverError::Unsolvable(format!(
                        "Package {} not found",
                        pkg
                    )));
                }
            };
            let id = choices[0].0;
            let pkg_info = self.pool.id_to_pkg(choices[0].0).unwrap();
            println!("{}: {} {}", id, pkg_info.0, pkg_info.1.to_string());
            formula.add_clause(&[Lit::from_dimacs(choices[0].0 as isize)]);
        }
        // Add rules to solver
        let mut solver = varisat::Solver::new();
        solver.add_formula(&formula);

        // Initial solve
        let mut res = solve(&mut solver)?;
        println!("Init solve finished");

        // Upgrade possible packages
        let mut older: Vec<Lit> = Vec::new();
        loop {
            let mut new_older = gen_update_assume(&self.pool, &res);
            if new_older.is_empty() {
                // All packages are up to date!
                break;
            }
            older.append(&mut new_older);
            solver.assume(&older);
            if !solver.solve().unwrap() {
                // It's not possible to improve any further
                break;
            } else {
                res = solve(&mut solver).unwrap();
            }
        }

        // Reduce redundant dependencies
        let mut min_res = Vec::new();
        for id in &res {
            let name = self.pool.id_to_pkg(*id).unwrap().0;
            let remove_rule: Vec<Lit> = self
                .pool
                .pkg_name_to_ids(&name)
                .unwrap()
                .iter()
                .map(|pkg| !Lit::from_dimacs(pkg.0 as isize))
                .collect();

            solver.assume(&remove_rule);
            if !solver.solve().unwrap() {
                min_res.push(*id);
            }
        }

        // Generate result
        let pkgs: Vec<(String, PackageVersion)> = min_res
            .into_iter()
            .map(|pkgid| {
                let res = self.pool.id_to_pkg(pkgid).unwrap();
                if !is_up_to_date(&self.pool, pkgid).unwrap() {
                    println!("{} not latest!", res.0);
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
        println!("Failed core:");
        let failed_core = solver.failed_core().unwrap();
        for c in failed_core {
            print!(", {}", c.to_dimacs());
        }
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
            let name = pool.id_to_pkg(*id).unwrap().0;
            let pkgids: Vec<usize> = pool
                .pkg_name_to_ids(&name)
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
    let name = pool.id_to_pkg(id)?.0;
    let ids = pool.pkg_name_to_ids(&name).unwrap();
    if ids[0].0 != id {
        Some(false)
    } else {
        Some(true)
    }
}
