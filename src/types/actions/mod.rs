mod table;

use super::{Checksum, PkgMeta, PkgSource, PkgVersion};

use anyhow::Result;
use console::style;
use indicatif::HumanBytes;

#[derive(Default, Debug)]
pub struct PkgActions<'a> {
    pub install: Vec<(&'a PkgMeta, Option<(PkgVersion, u64)>)>,
    pub unpack: Vec<(&'a PkgMeta, Option<(PkgVersion, u64)>)>,
    // (Name, InstallSize, Essential?)
    pub remove: Vec<(String, u64, bool)>,
    pub purge: Vec<(String, u64, bool)>,
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

impl PkgActions<'_> {
    pub fn is_empty(&self) -> bool {
        self.install.is_empty()
            && self.unpack.is_empty()
            && self.remove.is_empty()
            && self.purge.is_empty()
            && self.configure.is_empty()
    }

    pub fn remove_essential(&self) -> bool {
        for (_, _, essential) in &self.remove {
            if *essential {
                return true;
            }
        }

        for (_, _, essential) in &self.purge {
            if *essential {
                return true;
            }
        }

        false
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
        let install_prefix = style("INSTALL").on_blue().bold().to_string();
        crate::WRITER
            .write_chunks(&install_prefix, &to_install)
            .unwrap();

        let to_upgrade: Vec<String> = self
            .install
            .iter()
            .filter_map(|(install, oldpkg)| match oldpkg {
                Some(oldpkg) => {
                    if install.version > oldpkg.0 {
                        let mut msg = install.name.clone();
                        let ver_str = format!("({} -> {})", oldpkg.0, install.version);
                        msg.push_str(&style(ver_str).dim().to_string());
                        Some(msg)
                    } else {
                        None
                    }
                }
                None => None,
            })
            .collect();
        let upgrade_prefix = style("UPGRADE").on_green().black().bold().to_string();
        crate::WRITER
            .write_chunks(&upgrade_prefix, &to_upgrade)
            .unwrap();

        let to_downgrade: Vec<String> = self
            .install
            .iter()
            .filter_map(|(install, oldpkg)| match oldpkg {
                Some(oldpkg) => {
                    if install.version < oldpkg.0 {
                        let mut msg = install.name.clone();
                        let ver_str = format!("({} -> {})", oldpkg.0, install.version);
                        msg.push_str(&style(ver_str).dim().to_string());
                        Some(msg)
                    } else {
                        None
                    }
                }
                None => None,
            })
            .collect();
        let downgrade_prefix = style("DOWNGRADE").on_yellow().white().bold().to_string();
        crate::WRITER
            .write_chunks(&downgrade_prefix, &to_downgrade)
            .unwrap();

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
        let unpack_prefix = style("UNPACK").on_blue().bold().to_string();
        crate::WRITER
            .write_chunks(&unpack_prefix, &to_unpack)
            .unwrap();

        let configure_prefix = style("CONFIGURE").on_white().bold().to_string();
        crate::WRITER
            .write_chunks(&configure_prefix, &self.configure)
            .unwrap();

        let removes: Vec<String> = self
            .remove
            .iter()
            .map(|(name, _, essential)| {
                let mut pkg = name.clone();
                if *essential {
                    pkg.push_str(&style("(essential)").red().to_string());
                }
                pkg
            })
            .collect();
        let remove_prefix = style("REMOVE").on_red().bold().white().to_string();
        crate::WRITER
            .write_chunks(&remove_prefix, &removes)
            .unwrap();

        let purge_prefix = style("PURGE").on_red().white().bold().to_string();
        let purges: Vec<String> = self
            .purge
            .iter()
            .map(|(name, _, essential)| {
                let mut pkg = name.clone();
                if *essential {
                    pkg.push_str(&style("(essential)").red().to_string());
                }
                pkg
            })
            .collect();
        crate::WRITER.write_chunks(&purge_prefix, &purges).unwrap();
    }

    pub fn show_tables(&self, no_pager: bool) -> Result<()> {
        table::show_table(self, no_pager)
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
                        &style("Estimated change in storage usage:")
                            .bold()
                            .to_string(),
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
                        &style("Estimated change in storage usage:")
                            .bold()
                            .to_string(),
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

        res
    }

    fn calculate_download_size(&self) -> u64 {
        let mut res = 0;
        for install in &self.install {
            if let PkgSource::Http((_, size, _)) = install.0.source {
                res += size;
            }
        }

        for unpack in &self.unpack {
            if let PkgSource::Http((_, size, _)) = unpack.0.source {
                res += size;
            }
        }

        res
    }
}
