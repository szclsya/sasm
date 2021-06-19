mod deb;
mod pool;
mod types;
mod version;

use std::path::PathBuf;
use varisat::{lit::Lit, CnfFormula, ExtendFormula};

use crate::solver::version::PackageVersion;

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
    pool: pool::PackagePool,
}

impl Solver {
    pub fn from_dpkg_dbs(dbs: &[PathBuf]) -> Result<Self, SolverError> {
        let mut pool = pool::PackagePool::new();

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
        let mut solver = varisat::Solver::new();
        // Add requested packages to solver
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
            let mut formula = CnfFormula::new();
            formula.add_clause(&[Lit::from_dimacs(choices[0].0 as isize)]);
            solver.add_formula(&formula);
        }
        // Add rules to solver
        let formula = self.pool.gen_formula();
        solver.add_formula(&formula);

        // Solve
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

        let mut best_res = Vec::new();
        for id in &res {
            let name = self.pool.id_to_pkg(*id).unwrap().0;
            let ids: Vec<usize> = self
                .pool
                .pkg_name_to_ids(&name)
                .unwrap()
                .iter()
                .map(|pkg| pkg.0)
                .collect();
            let remove_rule: Vec<Lit> = ids
                .iter()
                .map(|id| !Lit::from_dimacs(*id as isize))
                .collect();

            // Reduce redundant dependencies
            solver.assume(&remove_rule);
            if !solver.solve().unwrap() {
                // If it is a necessary package, try to upgrade it
                let latest_id = ids[0];
                solver.assume(&[Lit::from_dimacs(latest_id as isize)]);
                if solver.solve().unwrap() {
                    best_res.push(latest_id);
                } else {
                    best_res.push(*id);
                }
            }
        }

        // Generate result
        let pkgs: Vec<(String, PackageVersion)> = best_res
            .into_iter()
            .map(|pkgid| self.pool.id_to_pkg(pkgid).unwrap())
            .collect();

        Ok(pkgs)
    }
}
