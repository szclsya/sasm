mod pkg;
mod provide;
pub use pkg::search_deb_db;
pub use provide::provide_file;

use crate::{
    executor::{MachineStatus, PkgState},
    types::PkgVersion,
};

use anyhow::Result;
use console::style;

pub struct PkgInfo {
    pub name: String,
    pub section: String,
    pub description: String,
    pub version: PkgVersion,
    pub has_dbg_pkg: bool,
}

impl PkgInfo {
    pub fn show(&self, machine_status: &MachineStatus) -> Result<()> {
        // Construct prefix
        let prefix = match machine_status.pkgs.get(&self.name) {
            Some(pkg) => match pkg.state {
                PkgState::Installed => style("INSTALLED").green(),
                PkgState::Unpacked => style("UNPACKED").yellow(),
                _ => style("PACKAGE").dim(),
            },
            None => style("PACKAGE").dim(),
        }
        .to_string();
        // Construct pkg info line
        let mut pkg_info_line = style(&self.name).bold().to_string();
        pkg_info_line.push(' ');
        pkg_info_line.push_str(&style(&self.version).green().to_string());
        if self.has_dbg_pkg {
            pkg_info_line.push(' ');
            pkg_info_line.push_str(&style("(debug symbols available)").dim().to_string());
        }
        crate::WRITER.writeln(&prefix, &pkg_info_line)?;
        crate::WRITER.writeln("", &self.description)?;

        Ok(())
    }
}
