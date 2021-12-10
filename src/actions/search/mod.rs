mod pkg;
mod provide;
pub use pkg::search_deb_db;
pub use provide::provide_file;

use crate::{
    executor::{MachineStatus, PkgState},
    types::PkgMeta,
};

use anyhow::Result;
use console::style;

pub struct PkgInfo<'a> {
    pub pkg: &'a PkgMeta,
    // Additional info
    pub has_dbg_pkg: bool,
    pub additional_info: Option<String>,
}

impl<'a> PkgInfo<'a> {
    pub fn from(pkg: &'a PkgMeta, has_dbg_pkg: bool, additional_info: Option<String>) -> Self {
        PkgInfo {
            pkg,
            has_dbg_pkg,
            additional_info,
        }
    }

    pub fn show(&self, machine_status: &MachineStatus) -> Result<()> {
        // Construct prefix
        let prefix = match machine_status.pkgs.get(&self.pkg.name) {
            Some(pkg) => match pkg.state {
                PkgState::Installed => style("INSTALLED").green(),
                PkgState::Unpacked => style("UNPACKED").yellow(),
                _ => style("PACKAGE").dim(),
            },
            None => style("PACKAGE").dim(),
        }
        .to_string();
        // Construct pkg info line
        let mut pkg_info_line = style(&self.pkg.name).bold().to_string();
        pkg_info_line.push(' ');
        pkg_info_line.push_str(&style(&self.pkg.version).green().to_string());
        if self.has_dbg_pkg {
            pkg_info_line.push(' ');
            pkg_info_line.push_str(&style("(debug symbols available)").dim().to_string());
        }
        crate::WRITER.writeln(&prefix, &pkg_info_line)?;
        // Write additional info, if applicable
        if let Some(additional_info) = &self.additional_info {
            crate::WRITER.writeln("", &additional_info)?;
        }

        // Write package description
        crate::WRITER.writeln("", &self.pkg.description)?;

        // Write recommended packages
        if let Some(recommends) = &self.pkg.recommends {
            let prefix = style("Recommends:").dim().to_string();
            let names: Vec<&str> = recommends.iter().map(|(name, _)| name.as_str()).collect();
            crate::WRITER.write_chunks(&prefix, &names)?;
        }

        // Write suggested packages
        if let Some(suggests) = &self.pkg.suggests {
            let prefix = style("Suggests:").dim().to_string();
            let names: Vec<&str> = suggests.iter().map(|(name, _)| name.as_str()).collect();
            crate::WRITER.write_chunks(&prefix, &names)?;
        }

        Ok(())
    }
}
