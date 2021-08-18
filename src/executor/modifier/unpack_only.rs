use crate::types::{PkgActionModifier, PkgActions};

/// Unpack-only mode, used for alternate root usage
/// Can be used for OS bootstraping
pub struct UnpackOnly;

impl PkgActionModifier for UnpackOnly {
    fn apply(actions: &mut PkgActions) {
        // All installs should be unpacks
        actions.unpack.append(&mut actions.install);
        // Configures should be ignored
        actions.configure.clear();
    }
}
