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
        self.pool.add_rules_to_solver(&mut solver, 0);

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
                    res.push(self.pool.id_to_pkg(id)?);
                }
            }
        }

        Ok(res)
    }
}
