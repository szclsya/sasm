use super::PkgVersion;

#[derive(Default)]
pub struct PkgActions {
    /// Vec<(Name, URL, size, ThisVersion, Option<OlderVersion>)
    pub install: Vec<(String, String, u64, PkgVersion, Option<PkgVersion>)>,
    pub remove: Vec<String>,
    pub purge: Vec<String>,
    pub configure: Vec<String>,
}

impl PkgActions {
    pub fn is_empty(&self) -> bool {
        self.install.is_empty()
            && self.remove.is_empty()
            && self.purge.is_empty()
            && self.configure.is_empty()
    }

    pub fn show(&self) {
        let to_install: Vec<String> = self
            .install
            .iter()
            .filter_map(|pkg| {
                let mut msg = pkg.0.to_string();
                match &pkg.4 {
                    Some(_) => None,
                    None => {
                        let ver_str = format!("({})", pkg.3);
                        msg.push_str(&console::style(ver_str).dim().to_string());
                        Some(msg)
                    }
                }
            })
            .collect();
        crate::WRITER.write_chunks("INSTALL", &to_install).unwrap();

        let to_upgrade: Vec<String> = self
            .install
            .iter()
            .filter_map(|pkg| {
                let mut msg = pkg.0.to_string();
                match &pkg.4 {
                    Some(oldver) => {
                        let ver_str = format!("({} -> {})", pkg.3, oldver);
                        msg.push_str(&console::style(ver_str).dim().to_string());
                        Some(msg)
                    }
                    None => None,
                }
            })
            .collect();
        crate::WRITER.write_chunks("UPGRADE", &to_upgrade).unwrap();

        crate::WRITER
            .write_chunks("CONFIGURE", &self.configure)
            .unwrap();
        crate::WRITER.write_chunks("PURGE", &self.purge).unwrap();
        crate::WRITER.write_chunks("REMOVE", &self.remove).unwrap();
    }
}
