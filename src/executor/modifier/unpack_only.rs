use crate::types::{PkgActionModifier, PkgActions};

/// Unpack-only mode, used for alternate root usage
/// Can be used for OS bootstraping
#[derive(Default)]
pub struct UnpackOnly;

impl PkgActionModifier for UnpackOnly {
    fn apply(&self, actions: &mut PkgActions) {
        // All installs should be unpacks
        actions.unpack.append(&mut actions.install);
        // Configures should be ignored
        actions.configure.clear();
    }
}
