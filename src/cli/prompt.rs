use crate::types::config::Opts;

use anyhow::Result;
use dialoguer::Confirm;

pub fn ask_confirm(opts: &Opts, msg: &str) -> Result<bool> {
    if opts.yes {
        return Ok(true);
    }

    let res = Confirm::new().with_prompt(msg).interact()?;
    Ok(res)
}
