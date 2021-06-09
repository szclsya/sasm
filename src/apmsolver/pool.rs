use super::types::PackageMeta;
use super::version::PackageVersion;

pub struct PackagePool {
    pkgs: Vec<(String, PackageMeta)>,
}

impl PackagePool {
    pub fn new() -> Self {
        PackagePool { pkgs: Vec::new() }
    }

    pub fn add(&mut self, name: &str, version: PackageMeta) -> usize {
        self.pkgs.push((name.to_string(), version));
        // Return the index of the last element
        self.pkgs.len() - 1
    }

    pub fn get_ids(&self, name: &str) -> Vec<(usize, PackageVersion)> {
        let mut res = Vec::new();
        for (pos, e) in self.pkgs.iter().enumerate() {
            if e.0 == name {
                res.push((pos, e.1.version.clone()));
            }
        }
        res
    }
}
