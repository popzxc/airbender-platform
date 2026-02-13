//! Commit helpers for mapping values into output registers.

pub use airbender_core::guest::Commit;

/// Commit values to the default output registers and exit successfully.
pub fn commit<T: Commit>(value: T) -> ! {
    let words = value.commit_words();
    airbender_rt::sys::exit_success(&words)
}

/// Exit with an error.
pub fn exit_error() -> ! {
    airbender_rt::sys::exit_error()
}
