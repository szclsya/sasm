use super::{Checksum, PkgVersion};
use console::style;
use indicatif::HumanBytes;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct PkgActions {
    pub install: Vec<(PkgInstallAction, Option<(PkgVersion, u64)>)>,
    pub unpack: Vec<(PkgInstallAction, Option<(PkgVersion, u64)>)>,
    // (Name, InstallSize)
    pub remove: Vec<(String, u64)>,
    pub purge: Vec<(String, u64)>,
    pub configure: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PkgInstallAction {
    pub name: String,
    pub url: String,
    pub download_size: u64,
    pub install_size: u64,
    pub checksum: Checksum,
    pub version: PkgVersion,
}

/// Alter PkgActions based on user configuration, system state, etc.
pub trait PkgActionModifier {
    fn apply(&self, actions: &mut PkgActions);
}

impl PkgActions {
    pub fn is_empty(&self) -> bool {
        self.install.is_empty()
            && self.unpack.is_empty()
            && self.remove.is_empty()
            && self.purge.is_empty()
            && self.configure.is_empty()
    }

    pub fn show(&self) {
        let to_install: Vec<String> = self
            .install
            .iter()
            .filter_map(|(install, old_ver)| match old_ver {
                Some(_) => None,
                None => {
                    let mut msg = install.name.clone();
                    let ver_str = format!("({})", install.version);
                    msg.push_str(&style(ver_str).dim().to_string());
                    Some(msg)
                }
            })
            .collect();
        crate::WRITER.write_chunks("INSTALL", &to_install).unwrap();

        let to_upgrade: Vec<String> = self
            .install
            .iter()
            .filter_map(|(install, oldpkg)| match oldpkg {
                Some(oldpkg) => {
                    let mut msg = install.name.clone();
                    let ver_str = format!("({} -> {})", oldpkg.0, install.version);
                    msg.push_str(&style(ver_str).dim().to_string());
                    Some(msg)
                }
                None => None,
            })
            .collect();
        crate::WRITER.write_chunks("UPGRADE", &to_upgrade).unwrap();

        let to_unpack: Vec<String> = self
            .unpack
            .iter()
            .map(|(install, oldpkg)| {
                let mut msg = install.name.clone();
                match oldpkg {
                    Some(oldpkg) => {
                        let ver_str = format!("({} -> {})", oldpkg.0, install.version);
                        msg.push_str(&style(ver_str).dim().to_string());
                    }
                    None => {
                        let ver_str = format!("({})", install.version);
                        msg.push_str(&style(ver_str).dim().to_string());
                    }
                };
                msg
            })
            .collect();
        crate::WRITER.write_chunks("UNPACK", &to_unpack).unwrap();

        crate::WRITER
            .write_chunks("CONFIGURE", &self.configure)
            .unwrap();
        let purge_header = style("PURGE").red().to_string();
        let purges: Vec<&str> = self.purge.iter().map(|(name, _)| name.as_str()).collect();
        crate::WRITER.write_chunks(&purge_header, &purges).unwrap();

        let remove_header = style("REMOVE").red().to_string();
        let removes: Vec<&str> = self.remove.iter().map(|(name, _)| name.as_str()).collect();
        crate::WRITER
            .write_chunks(&remove_header, &removes)
            .unwrap();
    }

    pub fn show_size_change(&self) {
        crate::WRITER
            .writeln(
                "",
                &format!(
                    "{} {}",
                    &style("Total download size:").bold().to_string(),
                    HumanBytes(self.calculate_download_size())
                ),
            )
            .unwrap();
        let install_size_change = self.calculate_size_change();
        let abs_install_size_change = install_size_change.abs() as u64;
        if install_size_change >= 0 {
            crate::WRITER
                .writeln(
                    "",
                    &format!(
                        "{} +{}",
                        &style("Estimated total size change:").bold().to_string(),
                        HumanBytes(abs_install_size_change)
                    ),
                )
                .unwrap();
        } else {
            crate::WRITER
                .writeln(
                    "",
                    &format!(
                        "{} -{}",
                        &style("Estimated total size change:").bold().to_string(),
                        HumanBytes(abs_install_size_change)
                    ),
                )
                .unwrap();
        }
    }

    fn calculate_size_change(&self) -> i128 {
        let mut res: i128 = 0;
        for install in &self.install {
            res += i128::from(install.0.install_size);
            if let Some(oldpkg) = &install.1 {
                res -= i128::from(oldpkg.1);
            }
        }

        for unpack in &self.unpack {
            res += i128::from(unpack.0.install_size);
            if let Some(oldpkg) = &unpack.1 {
                res -= i128::from(oldpkg.1);
            }
        }

        for remove in &self.remove {
            res -= i128::from(remove.1);
        }

        for purge in &self.purge {
            res -= i128::from(purge.1);
        }

        // Installed-Size is in kilobytes
        res *= 1024;
        res
    }

    fn calculate_download_size(&self) -> u64 {
        let mut res = 0;
        for install in &self.install {
            res += install.0.download_size;
        }

        for unpack in &self.unpack {
            res += unpack.0.download_size;
        }

        res
    }
}
