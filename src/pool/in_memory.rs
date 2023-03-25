use super::{BasicPkgPool, PkgPool};
use crate::types::{PkgMeta, PkgVersion, VersionRequirement};

use rayon::prelude::*;
use std::collections::HashMap;

pub struct InMemoryPool {
    pkgs: Vec<PkgMeta>,
    // The id of packages for each name, sorted by version
    name_to_ids: HashMap<String, Vec<(usize, PkgVersion)>>,
}

impl InMemoryPool {
    pub fn new() -> Self {
        InMemoryPool {
            pkgs: Vec::new(),
            name_to_ids: HashMap::new(),
        }
    }
}

impl BasicPkgPool for InMemoryPool {
    fn add(&mut self, meta: PkgMeta) -> usize {
        let name = meta.name.clone();
        let version = meta.version.clone();
        self.pkgs.push(meta);
        let index = self.pkgs.len();

        if self.name_to_ids.contains_key(&name) {
            let ids = self.name_to_ids.get_mut(&name).unwrap();
            ids.push((index, version));
        } else {
            self.name_to_ids.insert(name, Vec::from([(index, version)]));
        }

        index
    }

    fn finalize(&mut self) {
        // Sort versions
        self.name_to_ids.iter_mut().for_each(|(_, pkgs)| {
            // Sort in descending order
            pkgs.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        });
    }

    fn get_pkg_by_id(&self, id: usize) -> Option<&PkgMeta> {
        if id > self.pkgs.len() {
            return None;
        }
        // Since our SAT solver only accepts int > 0 as Literal, we offset pos by 1
        let pos = id - 1;
        let pkg = &self.pkgs[pos];
        Some(pkg)
    }

    fn get_pkgs_by_name(&self, name: &str) -> Option<Vec<usize>> {
        match self.name_to_ids.get(name) {
            Some(pkgs) => {
                let res: Vec<usize> = pkgs.iter().map(|(pkgid, _)| *pkgid).collect();
                Some(res)
            }
            None => None,
        }
    }

    fn get_pkgs_by_provide(&self, name: &str, ver_req: &VersionRequirement) -> Vec<(usize, &PkgMeta)> {
        let res: Vec<(usize, &PkgMeta)> = self
            .pkgs
            .par_iter()
            .enumerate()
            .filter(|(i, pkg)| pkg.provides.iter().filter(|p| p.0 == name && ver_req.within(&p.1)).count() > 0)
            .map(|(i, pkg)| (i+1, pkg))
            .collect();
        res
    }

    fn pkgname_iter(&self) -> Box<dyn Iterator<Item = (&str, &[(usize, PkgVersion)])> + '_> {
        Box::new(
            self.name_to_ids
                .iter()
                .map(|(name, pkgs)| (name.as_str(), pkgs.as_slice())),
        )
    }

    fn pkgid_iter(&self) -> Box<dyn Iterator<Item = (usize, &PkgMeta)> + '_> {
        // PkgID = pos + 1
        Box::new(self.pkgs.iter().enumerate().map(|(pos, meta)| {
            let id = pos + 1;
            (id, meta)
        }))
    }
}

impl PkgPool for InMemoryPool {}
