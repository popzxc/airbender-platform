//! Build and package guest artifacts into a `dist/` directory.

mod config;
mod constants;
mod errors;
mod utils;

pub use airbender_core::host::manifest::{Manifest, Profile, MANIFEST_FORMAT_VERSION};
pub use config::{build_dist, BuildConfig, DistArtifacts};
pub use constants::DEFAULT_GUEST_TOOLCHAIN;
pub use errors::{BuildError, Result};
