#![cfg_attr(not(feature = "host"), no_std)]

//! Shared definitions for Airbender tooling and runtimes.

extern crate alloc;

pub mod guest;
pub mod wire;

#[cfg(feature = "host")]
pub mod manifest;

#[cfg(feature = "host")]
pub mod host {
    pub use crate::manifest;
}
