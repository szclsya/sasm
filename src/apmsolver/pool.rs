use super::types::PackageMeta;
use super::version::PackageVersion;
use std::collections::HashMap;

pub struct PackagePool {
    pkgs: Vec<(String, PackageMeta)>,
}

impl PackagePool {
    pub fn new() -> Self {
        PackagePool { pkgs: Vec::new() }
    }

    pub fn add(&mut self, name: &str, meta: PackageMeta) -> usize {
        self.pkgs.push((name.to_string(), meta));
        self.pkgs.len() - 1
    }

    pub fn get_ids<'a>(&self, name: &str) -> Vec<(usize, PackageMeta)> {
        let mut res: Vec<(usize, PackageMeta)> = Vec::new();

        for (pos, pkg) in self.pkgs.iter().enumerate() {
            if pkg.0 == name {
                res.push((pos, pkg.1.clone()));
            }
        }
        res
    }
}
