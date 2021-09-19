use super::{Checksum, PkgVersion};

#[derive(Default, Debug, PartialEq, Eq)]
pub struct PkgActions {
    pub install: Vec<(PkgInstallAction, Option<PkgVersion>)>,
    pub unpack: Vec<(PkgInstallAction, Option<PkgVersion>)>,
    pub remove: Vec<String>,
    pub purge: Vec<String>,
    pub configure: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PkgInstallAction {
    pub name: String,
    pub url: String,
    pub size: u64,
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
                    let mut msg = install.name.to_string();
                    let ver_str = format!("({})", install.version);
                    msg.push_str(&console::style(ver_str).dim().to_string());
                    Some(msg)
                }
            })
            .collect();
        crate::WRITER.write_chunks("INSTALL", &to_install).unwrap();

        let to_upgrade: Vec<String> = self
            .install
            .iter()
            .filter_map(|(install, old_ver)| match old_ver {
                Some(old_ver) => {
                    let mut msg = install.name.to_string();
                    let ver_str = format!("({} -> {})", old_ver, install.version);
                    msg.push_str(&console::style(ver_str).dim().to_string());
                    Some(msg)
                }
                None => None,
            })
            .collect();
        crate::WRITER.write_chunks("UPGRADE", &to_upgrade).unwrap();

        let to_unpack: Vec<String> = self
            .unpack
            .iter()
            .map(|(install, old_ver)| {
                let mut msg = install.name.to_string();
                match old_ver {
                    Some(old_ver) => {
                        let ver_str = format!("({} -> {})", old_ver, install.version);
                        msg.push_str(&console::style(ver_str).dim().to_string());
                    }
                    None => {
                        let ver_str = format!("({})", install.version);
                        msg.push_str(&console::style(ver_str).dim().to_string());
                    }
                };
                msg
            })
            .collect();
        crate::WRITER.write_chunks("UNPACK", &to_unpack).unwrap();

        crate::WRITER
            .write_chunks("CONFIGURE", &self.configure)
            .unwrap();
        let purge_header = console::style("PURGE").red().to_string();
        crate::WRITER.write_chunks(&purge_header, &self.purge).unwrap();
        let remove_header = console::style("REMOVE").red().to_string();
        crate::WRITER.write_chunks(&remove_header, &self.remove).unwrap();
    }
}
