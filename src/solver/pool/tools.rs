use super::PkgPool;

use anyhow::{bail, Result};
use varisat::lit::Lit;

pub fn pkg_to_rule(
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
