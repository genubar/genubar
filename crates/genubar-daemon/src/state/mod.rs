pub mod canonical_cache;
pub mod l1_cache;
pub mod window_registry;

pub use canonical_cache::CanonicalMenuCache;
pub use l1_cache::L1SkeletonCache;
pub use window_registry::WindowRegistry;

/// Composite runtime state holding live windows, RAM Canonical caches, and L1 disk skeletons.
#[derive(Clone)]
pub struct DaemonCaches {
    pub windows: WindowRegistry,
    pub canonical: CanonicalMenuCache,
    pub l1: L1SkeletonCache,
}

impl DaemonCaches {
    pub fn load_or_new() -> Self {
        Self {
            windows: WindowRegistry::default(),
            canonical: CanonicalMenuCache::default(),
            l1: L1SkeletonCache::load_or_new(),
        }
    }
}
