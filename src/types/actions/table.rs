/// Show actions in tables
use super::PkgActions;
use crate::utils::pager::Pager;

use anyhow::Result;
use console::style;
use indicatif::HumanBytes;
use std::io::Write;
use tabled::{Alignment, Column, Full, Modify, Style, Table, Tabled};

#[derive(Tabled)]
struct InstallRow {
    #[header("Name")]
    name: String,
    #[header("Version")]
    version: String,
    #[header("Installed Size")]
    size: String,
}

#[derive(Tabled)]
struct RemoveRow {
    #[header("Name")]
    name: String,
    #[header("Package Size")]
    size: String,
    // Show details to this specific removal. Eg: if this is an essential package
    #[header("Details")]
    detail: String,
}

#[derive(Tabled)]
struct ConfigureRow {
    #[header("Name")]
    name: String,
    #[header("Version")]
    version: String,
}

pub fn show_table(actions: &PkgActions, no_pager: bool) -> Result<()> {
    let mut install_rows = Vec::new();
    let mut upgrade_rows = Vec::new();
    let mut downgrade_rows = Vec::new();
    let mut remove_rows = Vec::new();
    let mut configure_rows = Vec::new();

    for (new, old) in actions.install.iter().rev() {
        let mut install_size_change: i128 = new.install_size.into();
        if let Some((_, oldsize)) = old {
            install_size_change -= *oldsize as i128;
        }
        let mut install_size_change_str = HumanBytes(install_size_change.abs() as u64).to_string();
        if install_size_change >= 0 {
            install_size_change_str.insert(0, '+');
        } else {
            install_size_change_str.insert(0, '-');
        }
        let mut row = InstallRow {
            name: new.name.clone(),
            version: match old {
                Some((oldver, _)) => format!("{} -> {}", oldver, new.version),
                None => new.version.to_string(),
            },
            size: install_size_change_str,
        };
        // Insert to different row based on operation
        if let Some(old) = old {
            // Upgrade/downgrade
            if old.0 < new.version {
                row.name = style(row.name).green().to_string();
                upgrade_rows.push(row);
            } else {
                row.name = style(row.name).yellow().to_string();
                downgrade_rows.push(row);
            }
        } else {
            // New package
            row.name = style(row.name).green().to_string();
            install_rows.push(row);
        }
    }

    for (name, size, essential) in &actions.remove {
        let detail = if *essential {
            style("Essential").on_white().red().to_string()
        } else {
            String::new()
        };
        let row = RemoveRow {
            name: style(name).red().to_string(),
            size: HumanBytes(*size).to_string(),
            detail,
        };
        remove_rows.push(row);
    }

    for (name, size, essential) in &actions.purge {
        let mut detail_sections = vec![style("Purge configuration files.").red().to_string()];
        if *essential {
            detail_sections.insert(0, style("Essential package.").on_white().red().to_string());
        }
        let detail = detail_sections.join(",");
        let row = RemoveRow {
            name: style(name).red().to_string(),
            size: HumanBytes(*size).to_string(),
            detail,
        };
        remove_rows.push(row);
    }

    for (name, version) in &actions.configure {
        let row = ConfigureRow {
            name: name.clone(),
            version: version.to_string(),
        };
        configure_rows.push(row);
    }

    let mut pager = Pager::new(no_pager)?;
    let pager_name = pager.pager_name().to_owned();
    let mut out = pager.get_writer()?;

    write_review_help_message(&mut out)?;
    // Show help message about how to exit review view
    if pager_name == Some("less") {
        writeln!(out, "{}", style("Press [q] to finish review.\n").bold())?;
    }

    if !remove_rows.is_empty() {
        writeln!(
            out,
            "The following packages will be {}:\n",
            style("REMOVED").red().bold()
        )?;
        let table = Table::new(&remove_rows)
            .with(Modify::new(Full).with(Alignment::left()))
            // Install Size column should align right
            .with(Modify::new(Column(1..2)).with(Alignment::right()))
            .with(Modify::new(Full).with(|s: &str| format!(" {s} ")))
            .with(Style::PSQL);
        writeln!(out, "{table}")?;
    }

    if !install_rows.is_empty() {
        writeln!(
            out,
            "The following packages will be {}:\n",
            style("installed").green().bold()
        )?;
        let table = Table::new(&install_rows)
            .with(Modify::new(Full).with(Alignment::left()))
            // Install Size column should align right
            .with(Modify::new(Column(2..3)).with(Alignment::right()))
            .with(Modify::new(Full).with(|s: &str| format!(" {s} ")))
            .with(Style::PSQL);
        writeln!(out, "{table}")?;
    }

    if !upgrade_rows.is_empty() {
        writeln!(
            out,
            "The following packages will be {}:\n",
            style("upgraded").green().bold()
        )?;
        let table = Table::new(&upgrade_rows)
            .with(Modify::new(Full).with(Alignment::left()))
            // Install Size column should align right
            .with(Modify::new(Column(2..3)).with(Alignment::right()))
            .with(Modify::new(Full).with(|s: &str| format!(" {s} ")))
            .with(Style::PSQL);
        writeln!(out, "{table}")?;
    }

    if !downgrade_rows.is_empty() {
        writeln!(
            out,
            "The following packages will be {}:\n",
            style("downgraded").yellow().bold()
        )?;
        let table = Table::new(&downgrade_rows)
            .with(Modify::new(Full).with(Alignment::left()))
            // Install Size column should align right
            .with(Modify::new(Column(1..2)).with(Alignment::right()))
            .with(Modify::new(Full).with(|s: &str| format!(" {s} ")))
            .with(Style::PSQL);
        writeln!(out, "{table}")?;
    }

    if !configure_rows.is_empty() {
        writeln!(
            out,
            "The following packages will be {}:\n",
            style("configured").blue().bold()
        )?;
        let table = Table::new(&configure_rows)
            .with(Modify::new(Full).with(Alignment::left()))
            .with(Modify::new(Full).with(|s: &str| format!(" {s} ")))
            .with(Style::PSQL);
        writeln!(out, "{table}")?;
    }

    // Write size changes
    writeln!(
        out,
        "{} {}",
        style("Total download size:").bold(),
        HumanBytes(actions.calculate_download_size())
    )?;
    let install_size_change = actions.calculate_size_change();
    let abs_install_size_change = install_size_change.abs() as u64;
    let symbol = if install_size_change >= 0 { '+' } else { '-' };
    writeln!(
        out,
        "{} {}{}",
        style("Estimated change in storage usage:").bold(),
        symbol,
        HumanBytes(abs_install_size_change)
    )?;

    // Finish writing
    drop(out);
    // Wait until pager exits
    pager.wait_for_exit()?;

    Ok(())
}

fn write_review_help_message(w: &mut dyn Write) -> Result<()> {
    writeln!(w, "{}", style("Pending Operations").bold())?;
    writeln!(w)?;
    writeln!(w, "Shown below is an overview of the pending changes Omakase will apply to your system, please review them carefully.")?;
    writeln!(w, "Please note that Omakase may {}, {}, {}, {}, or {} packages in order to fulfill your requested changes.", style("install").green(), style("remove").red(), style("upgrade").green(), style("downgrade").yellow(), style("configure").blue())?;
    writeln!(w)?;
    Ok(())
}
