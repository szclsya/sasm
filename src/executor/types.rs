use crate::types::PkgVersion;

use anyhow::{bail, format_err, Context, Error, Result};
use std::collections::HashMap;

/// Status of package on this instance, extracted from pacman local state db
/// Usually located at /var/lib/pacman/local
#[derive(Clone)]
pub struct PkgStatus {
    pub name: String,
    pub version: PkgVersion,
    pub install_size: u64,
}
