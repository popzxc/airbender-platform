//! Guest allocator backends.

#[cfg(not(any(
    feature = "allocator-bump",
    feature = "allocator-talc",
    feature = "allocator-custom"
)))]
compile_error!(
    "enable one allocator feature: `allocator-bump`, `allocator-talc`, or `allocator-custom`"
);

#[cfg(any(
    all(feature = "allocator-bump", feature = "allocator-talc"),
    all(feature = "allocator-bump", feature = "allocator-custom"),
    all(feature = "allocator-talc", feature = "allocator-custom"),
))]
compile_error!(
    "allocator features are mutually exclusive; enable only one of `allocator-bump`, `allocator-talc`, `allocator-custom`"
);

#[cfg(feature = "allocator-bump")]
mod bump_allocator;
#[cfg(feature = "allocator-bump")]
pub use bump_allocator::{init, BumpAllocator};

#[cfg(feature = "allocator-talc")]
mod talc_allocator;
#[cfg(feature = "allocator-talc")]
pub use talc_allocator::{init, TalcAllocator};
