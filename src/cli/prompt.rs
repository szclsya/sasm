use crate::types::config::Opts;

use anyhow::Result;
use console::style;
use dialoguer::{theme::Theme, Confirm};
use std::fmt;

pub fn ask_confirm(opts: &Opts, msg: &str) -> Result<bool> {
    if opts.yes {
        return Ok(true);
    }

    let prefix = super::gen_prefix("");
    let msg = format!("{prefix}{msg}");
    let res = Confirm::new().with_prompt(msg).interact()?;
    Ok(res)
}

/// Theme for dialoguer
#[derive(Default)]
pub struct OmaTheme;

impl Theme for OmaTheme {
    fn format_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        active: bool,
    ) -> fmt::Result {
        let prefix = match active {
            true => (crate::cli::gen_prefix(&style("->").bold().to_string())),
            false => (crate::cli::gen_prefix("")),
        };

        write!(f, "{prefix}{text}")
    }
}
