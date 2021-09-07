mod in_memory;
pub use in_memory::InMemoryPool;

use crate::types::PkgMeta;

use anyhow::{bail, format_err, Result};
use varisat::{lit::Lit, CnfFormula};

pub trait PkgPool {
    // Add a package to the pool
    fn add(&mut self, meta: PkgMeta) -> usize;
    // Finalize the pool, must call before using the pool
    fn finalize(&mut self);
    // Get PkgMeta from Pkg ID
    fn get_pkg_by_id(&self, id: usize) -> Option<&PkgMeta>;
    // Get a list of available package IDs based on the given name
    fn get_pkgs_by_name(&self, name: &str) -> Option<Vec<usize>>;
    // Generate formula for SAT solver, optionally use a subset of the packages
    fn gen_formula(&self, subset: Option<&[usize]>) -> CnfFormula;
}

pub fn get_deps(pool: &dyn PkgPool, pkgid: usize) -> Result<Vec<Vec<usize>>> {
    let pkg = pool
        .get_pkg_by_id(pkgid)
        .ok_or_else(|| format_err!("Package with ID {} not found", pkgid))?;
    let mut res = Vec::new();
    for dep in pkg.depends.iter() {
        let mut deps_id = Vec::new();
        let available = match pool.get_pkgs_by_name(&dep.0) {
            Some(d) => d,
            None => {
                bail!("Warning: Cannot find dependency {} for {}", dep.0, pkg.name);
            }
        };
        for dep_pkgid in available {
            let p = pool.get_pkg_by_id(dep_pkgid).unwrap();
            if dep.1.within(&p.version) {
                deps_id.push(dep_pkgid);
            }
        }
        if deps_id.is_empty() {
            bail!(
                "Warning: dependency {} can't be fulfilled for pkg {}",
                &dep.0,
                pkg.name
            );
        } else {
            res.push(deps_id);
        }
    }
    Ok(res)
}

fn pkg_to_rule(
    pool: &dyn PkgPool,
    pkgid: usize,
    subset: Option<&[usize]>,
) -> Result<Vec<Vec<Lit>>> {
    let pkg = pool.get_pkg_by_id(pkgid).unwrap();
    let mut res = Vec::new();
    // Enroll dependencies
    for dep in pkg.depends.iter() {
        let available = match pool.get_pkgs_by_name(&dep.0) {
            Some(pkgs) => match subset {
                Some(ids) => {
                    let pkgs: Vec<usize> =
                        pkgs.iter().filter(|id| ids.contains(id)).copied().collect();
                    pkgs
                }
                None => pkgs.iter().copied().collect(),
            },
            None => {
                bail!(
                    "Cannot fulfill dependency {} because no package found with this name",
                    dep.0
                );
            }
        };

        let mut clause = vec![!Lit::from_dimacs(pkgid as isize)];

        for dep_pkgid in available {
            let p = pool.get_pkg_by_id(dep_pkgid).unwrap();
            if dep.1.within(&p.version) {
                clause.push(Lit::from_dimacs(dep_pkgid as isize));
            }
        }

        if clause.len() > 1 {
            res.push(clause);
        } else {
            bail!(
                "Cannot fulfill dependency {} because no applicable version found",
                dep.0
            );
        }
    }

    // Enroll breaks
    for bk in pkg.breaks.iter() {
        let breakable = match pool.get_pkgs_by_name(&bk.0) {
            Some(pkgs) => match subset {
                Some(ids) => {
                    let pkgs: Vec<usize> = pkgs.into_iter().filter(|id| ids.contains(id)).collect();
                    pkgs
                }
                None => pkgs,
            },
            None => {
                // Nothing to break. Good!
                continue;
            }
        };

        for bk_pkgid in breakable {
            let p = pool.get_pkg_by_id(bk_pkgid).unwrap();
            if bk.1.within(&p.version) {
                let clause = vec![
                    !Lit::from_dimacs(pkgid as isize),
                    !Lit::from_dimacs(bk_pkgid as isize),
                ];
                res.push(clause);
            }
        }
    }

    // Enroll conflicts
    for conflict in pkg.conflicts.iter() {
        let conflicable = match pool.get_pkgs_by_name(&conflict.0) {
            Some(pkgs) => match subset {
                Some(ids) => {
                    let pkgs: Vec<usize> = pkgs.into_iter().filter(|id| ids.contains(id)).collect();
                    pkgs
                }
                None => pkgs,
            },
            None => {
                continue;
            }
        };

        for conflict_pkgid in conflicable {
            let p = pool.get_pkg_by_id(conflict_pkgid).unwrap();
            if conflict.1.within(&p.version) {
                let clause = vec![
                    !Lit::from_dimacs(pkgid as isize),
                    !Lit::from_dimacs(conflict_pkgid as isize),
                ];
                res.push(clause);
            }
        }
    }

    Ok(res)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::types::{Checksum, PkgMeta, PkgVersion, VersionRequirement};
    use std::convert::TryFrom;
    use varisat::ExtendFormula;

    #[test]
    fn trivial_pool() {
        let mut pool = InMemoryPool::new();
        let a_id = pool.add(PkgMeta {
            name: "a".to_string(),
            version: PkgVersion::try_from("1").unwrap(),
            depends: vec![(
                "c".to_string(),
                VersionRequirement {
                    lower_bond: None,
                    upper_bond: None,
                },
            )],
            breaks: vec![(
                "d".to_string(),
                VersionRequirement {
                    lower_bond: None,
                    upper_bond: None,
                },
            )],
            conflicts: Vec::new(),
            install_size: 0,
            url: String::new(),
            size: 0,
            checksum: Checksum::from_sha256_str(&str::repeat("a", 64)).unwrap(),
        });
        let b_id = pool.add(PkgMeta {
            name: "b".to_string(),
            version: PkgVersion::try_from("1").unwrap(),
            depends: vec![(
                "a".to_string(),
                VersionRequirement {
                    lower_bond: None,
                    upper_bond: None,
                },
            )],
            breaks: Vec::new(),
            conflicts: Vec::new(),
            install_size: 0,
            url: String::new(),
            size: 0,
            checksum: Checksum::from_sha256_str(&str::repeat("a", 64)).unwrap(),
        });
        let c_id = pool.add(PkgMeta {
            name: "c".to_string(),
            version: PkgVersion::try_from("1").unwrap(),
            depends: vec![(
                "b".to_string(),
                VersionRequirement {
                    lower_bond: None,
                    upper_bond: None,
                },
            )],
            breaks: Vec::new(),
            conflicts: Vec::new(),
            install_size: 0,
            url: String::new(),
            size: 0,
            checksum: Checksum::from_sha256_str(&str::repeat("a", 64)).unwrap(),
        });
        pool.add(PkgMeta {
            name: "e".to_string(),
            version: PkgVersion::try_from("1").unwrap(),
            depends: vec![(
                "b".to_string(),
                VersionRequirement {
                    lower_bond: None,
                    upper_bond: None,
                },
            )],
            breaks: Vec::new(),
            conflicts: Vec::new(),
            install_size: 0,
            url: String::new(),
            size: 0,
            checksum: Checksum::from_sha256_str(&str::repeat("a", 64)).unwrap(),
        });
        pool.finalize();

        let mut solver = varisat::Solver::new();
        let formula = pool.gen_formula(None);
        solver.add_formula(&formula);
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
