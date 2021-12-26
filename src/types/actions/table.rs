/// Show actions in tables
use super::PkgActions;

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
    #[header("Install Size")]
    size: String,
    // Show details to this specific installation
    #[header("Details")]
    detail: String,
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
}

pub fn show_table(actions: &PkgActions) -> Result<()> {
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
        // Installed-Size is in KiB, but HumanBytes counts in bytes
        install_size_change *= 1024;
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
            detail: String::new(),
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
        let mut detail_sections = vec![style("Purge configs").red().to_string()];
        if *essential {
            detail_sections.insert(0, style("Essential package").on_white().red().to_string());
        }
        let detail = detail_sections.join(",");
        let row = RemoveRow {
            name: style(name).red().to_string(),
            size: HumanBytes(*size).to_string(),
            detail,
        };
        remove_rows.push(row);
    }

    for name in &actions.configure {
        let row = ConfigureRow { name: name.clone() };
        configure_rows.push(row);
    }

    // Set-up pager
    let pager_cmd = std::env::var("PAGER").unwrap_or_else(|_| "less".to_owned());
    let pager_cmd_segments: Vec<&str> = pager_cmd.split_ascii_whitespace().collect();
    let pager_name = pager_cmd_segments.get(0).unwrap_or(&"less");
    let mut p = std::process::Command::new(&pager_name);
    if pager_name == &"less" {
        p.arg("-R"); // Show ANSI escape sequences correctly
        p.arg("-c"); // Start from the top of the screen
        p.env("LESSCHARSET", "UTF-8"); // Rust uses UTF-8
    } else if pager_cmd_segments.len() > 1 {
        p.args(&pager_cmd_segments[1..]);
    }
    let mut pager_process = p.stdin(std::process::Stdio::piped()).spawn()?;
    let out = pager_process
        .stdin
        .as_mut()
        .expect("Cannot take stdin for pager");

    if !install_rows.is_empty() {
        writeln!(
            out,
            "These packages will be {}:",
            style("installed").green().bold()
        )?;
        let table = Table::new(&install_rows)
            .with(Modify::new(Full).with(Alignment::left()))
            // Install Size column should align right
            .with(Modify::new(Column(2..3)).with(Alignment::right()))
            .with(Modify::new(Full).with(|s: &str| format!(" {} ", s)))
            .with(Style::psql());
        writeln!(out, "{}", table)?;
    }

    if !upgrade_rows.is_empty() {
        writeln!(
            out,
            "These packages will be {}:",
            style("upgraded").green().bold()
        )?;
        let table = Table::new(&upgrade_rows)
            .with(Modify::new(Full).with(Alignment::left()))
            // Install Size column should align right
            .with(Modify::new(Column(2..3)).with(Alignment::right()))
            .with(Modify::new(Full).with(|s: &str| format!(" {} ", s)))
            .with(Style::psql());
        writeln!(out, "{}", table)?;
    }

    if !downgrade_rows.is_empty() {
        writeln!(
            out,
            "These packages will be {}:",
            style("downgraded").green().bold()
        )?;
        let table = Table::new(&downgrade_rows)
            .with(Modify::new(Full).with(Alignment::left()))
            // Install Size column should align right
            .with(Modify::new(Column(1..2)).with(Alignment::right()))
            .with(Modify::new(Full).with(|s: &str| format!(" {} ", s)))
            .with(Style::psql());
        writeln!(out, "{}", table)?;
    }

    if !remove_rows.is_empty() {
        writeln!(
            out,
            "These packages will be {}:",
            style("removed").red().bold()
        )?;
        let table = Table::new(&remove_rows)
            .with(Modify::new(Full).with(Alignment::left()))
            // Install Size column should align right
            .with(Modify::new(Column(1..2)).with(Alignment::right()))
            .with(Modify::new(Full).with(|s: &str| format!(" {} ", s)))
            .with(Style::psql());
        writeln!(out, "{}", table)?;
    }

    if !configure_rows.is_empty() {
        writeln!(
            out,
            "These packages will be {}:",
            style("configured").blue().bold()
        )?;
        let table = Table::new(&configure_rows)
            .with(Modify::new(Full).with(Alignment::left()))
            .with(Modify::new(Full).with(|s: &str| format!(" {} ", s)))
            .with(Style::psql());
        writeln!(out, "{}", table)?;
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
        style("Estimated total size change:").bold(),
        symbol,
        HumanBytes(abs_install_size_change)
    )?;

    // Wait until pager exits
    pager_process.wait()?;
    Ok(())
}
