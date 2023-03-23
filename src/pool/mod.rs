mod in_memory;
pub mod source;
pub use in_memory::InMemoryPool;

use crate::{
    msg,
    types::{PkgMeta, PkgSource, PkgVersion, VersionRequirement},
    warn,
};

use anyhow::{anyhow, bail, format_err, Context, Result};
use console::style;
use varisat::{lit::Lit, CnfFormula, ExtendFormula};

/// The basic PkgPool interface
pub trait BasicPkgPool {
    // Add a package to the pool
    fn add(&mut self, meta: PkgMeta) -> usize;
    // Finalize the pool, must call before using the pool
    fn finalize(&mut self);
    // Get PkgMeta from Pkg ID
    fn get_pkg_by_id(&self, id: usize) -> Option<&PkgMeta>;
    // Get a list of available package IDs based on the given name
    fn get_pkgs_by_name(&self, name: &str) -> Option<Vec<usize>>;
    // Get an Iterator of (PkgName, &[(id, PkgVersion)])
    fn pkgname_iter(&self) -> Box<dyn Iterator<Item = (&str, &[(usize, PkgVersion)])> + '_>;
    // Get an Iterator of (PkgId, PkgMeta)
    fn pkgid_iter(&self) -> Box<dyn Iterator<Item = (usize, &PkgMeta)> + '_>;
}

/// Additional tools
pub trait PkgPool: BasicPkgPool {
    fn get_deps(&self, pkgid: usize) -> Result<Vec<Vec<usize>>> {
        let pkg = self
            .get_pkg_by_id(pkgid)
            .ok_or_else(|| format_err!("Package with ID {pkgid} not found."))?;
        let mut res = Vec::new();
        for dep in &pkg.depends {
            let mut deps_id = Vec::new();
            let available = match self.get_pkgs_by_name(&dep.0) {
                Some(d) => d,
                None => {
                    bail!(
                        "Cannot find dependency {} for {}.",
                        style(&dep.0).bold(),
                        style(&pkg.name).bold()
                    );
                }
            };
            for dep_pkgid in &available {
                let p = self.get_pkg_by_id(*dep_pkgid).unwrap();
                if dep.1.contains(&p.version) {
                    deps_id.push(*dep_pkgid);
                }
            }
            if deps_id.is_empty() {
                let error = anyhow!(
                    "{} requires {} ({}), but only the following version(s) are available: {}.",
                    pkg.name,
                    dep.0,
                    dep.1,
                    available
                        .iter()
                        .map(|id| self.get_pkg_by_id(*id).unwrap().version.to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                );
                let context = format!(
                    "Cannot fulfill dependency {} for {}: no suitable version.",
                    style(&dep.0).bold(),
                    style(&pkg.name).bold()
                );
                return Err(error).context(context);
            } else {
                res.push(deps_id);
            }
        }
        Ok(res)
    }

    fn pick_best_pkg(
        &self,
        pkgname: &str,
        ver_req: &VersionRequirement,
        need_local: bool,
    ) -> Result<usize> {
        if let Some(pkgs) = self.get_pkgs_by_name(pkgname) {
            let mut first_valid_version = true;
            for id in pkgs {
                // Safe unless the pool is broken
                let pkg = self.get_pkg_by_id(id).unwrap();
                let is_local = matches!(pkg.source, PkgSource::Local(_));
                if ver_req.contains(&pkg.version) {
                    if need_local == is_local {
                        return Ok(id);
                    } else if first_valid_version {
                        // First version that matches version requirement but can't use it because local
                        // Tell it to the user
                        warn!("Keeping local version of {}, but a newer version is available in upstream repositories.",
                              style(pkgname).bold());
                        msg!(
                            "Remove the {} keyword from your blueprint to use the latest version.",
                            style("local").bold()
                        );
                        first_valid_version = false;
                    }
                }
            }
            // We haven't found a suitable candidate
            bail!("Cannot find a suitable version for {pkgname}.");
        } else {
            bail!("Package {pkgname} not found.");
        }
    }

    fn find_provide(&self, name: &str, ver_req: &Option<VersionRequirement>) -> Option<String> {
        let ver_req = ver_req.clone().unwrap_or_default();
        for (_, pkg) in self.pkgid_iter() {
            for provide in &pkg.provides {
                if provide.0 == name && provide.1.combine(&ver_req).is_ok() {
                    return Some(pkg.name.to_owned());
                }
            }
        }

        None
    }

    fn find_replacement(&self, name: &str, ver_req: &VersionRequirement) -> Option<String> {
        for (_, pkg) in self.pkgid_iter() {
            for replace in &pkg.replaces {
                if replace.0 == name && replace.1.within(ver_req) {
                    return Some(pkg.name.to_owned());
                }
            }
        }

        None
    }

    fn pkg_to_rule(&self, pkgid: usize, subset: Option<&[usize]>) -> Result<Vec<Vec<Lit>>> {
        let pkg = self.get_pkg_by_id(pkgid).unwrap();
        let mut res = Vec::new();
        // Enroll dependencies
        for dep in &pkg.depends {
            let available = match self.get_pkgs_by_name(&dep.0) {
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
                        "Cannot find a package which fulfills dependency {}.",
                        style(&dep.0).bold()
                    );
                }
            };

            let mut clause = vec![!Lit::from_dimacs(pkgid as isize)];

            for dep_pkgid in available {
                let p = self.get_pkg_by_id(dep_pkgid).unwrap();
                if dep.1.contains(&p.version) {
                    clause.push(Lit::from_dimacs(dep_pkgid as isize));
                }
            }

            if clause.len() > 1 {
                res.push(clause);
            } else {
                bail!(
                    "Cannot find an applicable version for dependency {}.",
                    style(&dep.0).bold()
                );
            }
        }

        // Enroll conflicts
        for conflict in pkg.conflicts.iter() {
            let conflicable = match self.get_pkgs_by_name(&conflict.0) {
                Some(pkgs) => match subset {
                    Some(ids) => {
                        let pkgs: Vec<usize> =
                            pkgs.into_iter().filter(|id| ids.contains(id)).collect();
                        pkgs
                    }
                    None => pkgs,
                },
                None => {
                    continue;
                }
            };

            for conflict_pkgid in conflicable {
                let p = self.get_pkg_by_id(conflict_pkgid).unwrap();
                if conflict.1.contains(&p.version) {
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

    fn gen_formula(&self, subset: Option<&[usize]>) -> CnfFormula {
        let mut formula = CnfFormula::new();

        // Generating rules from pool
        for (id, meta) in self.pkgid_iter() {
            let valid = match subset {
                Some(ids) => ids.contains(&id),
                // If there's no subset requirement, then all packages are valid
                None => true,
            };
            if valid {
                match self.pkg_to_rule(id, subset) {
                    Ok(rules) => {
                        for rule in rules {
                            formula.add_clause(&rule);
                        }
                    }
                    Err(e) => {
                        warn!("Ignoring package {}: {}", style(&meta.name).bold(), e);
                    }
                }
            }
        }

        // Generate conflict for different versions of the same package
        for (_, versions) in self.pkgname_iter() {
            let versions: Vec<usize> = match subset {
                Some(ids) => versions
                    .iter()
                    .filter(|pkg| ids.contains(&pkg.0))
                    .map(|pkg| pkg.0)
                    .collect(),
                None => versions.iter().map(|(id, _)| *id).collect(),
            };
            if versions.len() > 1 {
                let clause: Vec<Lit> = versions
                    .into_iter()
                    .map(|pkgid| !Lit::from_dimacs(pkgid as isize))
                    .collect();
                formula.add_clause(&clause);
            }
        }

        formula
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::types::{PkgMeta, PkgVersion, VersionRequirement};
    use std::path::PathBuf;

    #[test]
    fn trivial_pool() {
        let mut pool = InMemoryPool::new();
        let a_id = pool.add(PkgMeta {
            name: "a".to_string(),
            description: "".to_string(),
            section: "".to_string(),
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
            recommends: None,
            suggests: None,
            replaces: None,
            provides: None,
            install_size: 0,
            essential: false,
            source: PkgSource::Local(PathBuf::new()),
        });
        let b_id = pool.add(PkgMeta {
            name: "b".to_string(),
            description: "".to_string(),
            section: "".to_string(),
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
            recommends: None,
            suggests: None,
            replaces: None,
            provides: None,
            install_size: 0,
            essential: false,
            source: PkgSource::Local(PathBuf::new()),
        });
        let c_id = pool.add(PkgMeta {
            name: "c".to_string(),
            description: "".to_string(),
            section: "".to_string(),
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
            recommends: None,
            suggests: None,
            replaces: None,
            provides: None,
            install_size: 0,
            essential: false,
            source: PkgSource::Local(PathBuf::new()),
        });
        let d_id = pool.add(PkgMeta {
            name: "d".to_string(),
            description: "".to_string(),
            section: "".to_string(),
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
            recommends: None,
            suggests: None,
            replaces: None,
            provides: None,
            install_size: 0,
            essential: false,
            source: PkgSource::Local(PathBuf::new()),
        });
        pool.finalize();

        let mut solver = varisat::Solver::new();
        let formula = pool.gen_formula(None);
        solver.add_formula(&formula);
        solver.assume(&[Lit::from_dimacs(c_id as isize)]);

        solver.solve().unwrap();
        assert_eq!(
            solver.model().unwrap(),
            vec![
                Lit::from_dimacs(a_id as isize),
                Lit::from_dimacs(b_id as isize),
                Lit::from_dimacs(c_id as isize),
                !Lit::from_dimacs(d_id as isize),
            ]
        );
    }
}
