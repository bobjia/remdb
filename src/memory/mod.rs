pub mod allocator;
pub mod pool;

// Re-export types and functions from remdb-alloc
pub use remdb_alloc::{
    MemoryBlock, MemoryStats, StaticAllocator,
    init_global_allocator, reset_allocator as reset_global_allocator,
};

// Re-export Mutex for no_std compatibility (used by time_series modules)
#[cfg(feature = "std")]
pub use std::sync::Mutex;
#[cfg(not(feature = "std"))]
pub use remdb_alloc::Mutex;