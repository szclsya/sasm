mod parse;
mod variables;
use parse::{read_blueprint_from_file, BlueprintLine};

use crate::{error, info, msg, types::VersionRequirement};

use anyhow::{bail, Context, Result};
use console::style;
use std::{fs::OpenOptions, os::unix::fs::FileExt, path::PathBuf};

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub struct PkgRequest {
    pub name: String,
    pub version: VersionRequirement,
    pub added_by: Option<String>,
    pub local: bool,
}

impl std::fmt::Display for PkgRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.name)?;
        // Sections of package property
        let mut sections = Vec::new();
        if !self.version.is_arbitary() {
            sections.push(self.version.to_string());
        }
        if let Some(pkgname) = &self.added_by {
            sections.push(format!("added_by = {}", pkgname));
        }
        if self.local {
            sections.push("local".to_owned());
        }
        // Write it
        if !sections.is_empty() {
            let joined = sections.join(", ");
            write!(f, " ({})", joined)?;
        }
        Ok(())
    }
}

/// A collection of blueprints
pub struct Blueprints {
    user_blueprint_path: PathBuf,
    // If we need to export the blueprint back to disk
    user_blueprint_modified: bool,
    user: Vec<BlueprintLine>,
    vendor: Vec<(PathBuf, Vec<BlueprintLine>)>,
}

impl Blueprints {
    pub fn from_files(user: PathBuf, vendor: &[PathBuf]) -> Result<Self> {
        let user_blueprint = read_blueprint_from_file(&user)?;
        let mut vendor_blueprints = Vec::with_capacity(vendor.len());
        for path in vendor {
            vendor_blueprints.push((path.clone(), read_blueprint_from_file(path)?));
        }

        Ok(Blueprints {
            user_blueprint_path: user,
            user_blueprint_modified: false,
            user: user_blueprint,
            vendor: vendor_blueprints,
        })
    }

    pub fn get_pkg_requests(&self) -> Vec<&PkgRequest> {
        // Add user blueprint first
        let mut res: Vec<&PkgRequest> = self
            .user
            .iter()
            .filter_map(|line| match line {
                BlueprintLine::PkgRequest(req) => Some(req),
                _ => None,
            })
            .collect();

        // Then add vendor blueprint
        for (_, vendor) in &self.vendor {
            for line in vendor {
                if let BlueprintLine::PkgRequest(req) = line {
                    res.push(req);
                }
            }
        }

        // Duplicates are allowed, so we shall dedup here
        res.dedup();
        res
    }

    pub fn add(
        &mut self,
        pkgname: &str,
        added_by: Option<&str>,
        ver_req: Option<VersionRequirement>,
        local: bool,
    ) -> Result<()> {
        if self.user_list_contains(pkgname) {
            bail!("Package {} already exists in user blueprint", pkgname);
        }

        let version = ver_req.unwrap_or_default();
        let pkgreq = PkgRequest {
            name: pkgname.to_string(),
            version,
            added_by: added_by.map(|pkgname| pkgname.to_owned()),
            local,
        };
        self.user.push(BlueprintLine::PkgRequest(pkgreq));
        self.user_blueprint_modified = true;
        Ok(())
    }

    pub fn remove(&mut self, pkgname: &str, remove_recomms: bool) -> Result<()> {
        if !self.user_list_contains(pkgname) {
            if let Some(path) = self.vendor_list_contains(pkgname) {
                error!(
                    "Package {} not found in user blueprint",
                    style(pkgname).bold()
                );
                info!(
                    "However, it exists in vendor blueprint at {}",
                    style(path.display()).bold()
                );
                msg!(
                    "",
                    "You cannot remove packages in vendor blueprints via Omakase CLI for safety reason. But if you really wish to remove this package, edit the file above directly."
                );
            } else {
                error!(
                    "Package {} not found in all blueprints",
                    style(pkgname).bold()
                );
            }
            bail!("Cannot remove {} from user blueprint", pkgname)
        } else {
            self.user.retain(|line| match line {
                BlueprintLine::PkgRequest(req) => req.name != pkgname,
                _ => true,
            });
            if remove_recomms {
                self.remove_affiliated(pkgname);
            }
            self.user_blueprint_modified = true;
            Ok(())
        }
    }

    pub fn remove_affiliated(&mut self, pkgname: &str) {
        let prev_len = self.user.len();
        self.user.retain(|line| match line {
            BlueprintLine::PkgRequest(req) => req.added_by != Some(pkgname.to_string()),
            _ => true,
        });
        if self.user.len() < prev_len {
            self.user_blueprint_modified = true;
        }
    }

    // Write back user blueprint
    pub fn export(&self) -> Result<bool> {
        if !self.user_blueprint_modified {
            // If not modified, nothing to do here.
            return Ok(false);
        }

        let mut res = String::new();
        for l in &self.user {
            match l {
                BlueprintLine::Comment(content) => res.push_str(&format!("#{}\n", content)),
                BlueprintLine::EmptyLine => res.push('\n'),
                BlueprintLine::PkgRequest(req) => res.push_str(&format!("{}\n", req.to_string())),
            }
        }

        // Open user blueprint
        let blueprint_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.user_blueprint_path)?;
        blueprint_file.set_len(0)?;
        blueprint_file
            .write_all_at(&res.into_bytes(), 0)
            .context(format!(
                "Failed to write to blueprint file at {}",
                self.user_blueprint_path.display()
            ))?;

        Ok(true)
    }

    fn user_list_contains(&self, pkgname: &str) -> bool {
        for line in &self.user {
            if let BlueprintLine::PkgRequest(req) = line {
                if req.name == pkgname {
                    return true;
                }
            }
        }
        false
    }

    fn vendor_list_contains(&self, pkgname: &str) -> Option<PathBuf> {
        for (path, vendor) in &self.vendor {
            for line in vendor {
                if let BlueprintLine::PkgRequest(req) = line {
                    if req.name == pkgname {
                        return Some(path.clone());
                    }
                }
            }
        }
        None
    }
}
