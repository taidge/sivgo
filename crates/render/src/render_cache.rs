//! Render tree caching to avoid full rebuilds each frame.
//!
//! [`RenderCache`] stores the most recently rendered pixel buffer together
//! with a *generation* counter.  When the document has not changed since the
//! last render, the cached pixmap can be reused directly, skipping the entire
//! rendering pipeline.
//!
//! # Usage
//!
//! ```ignore
//! let mut cache = RenderCache::new();
//! let gen = cache.generation();
//!
//! // ... document editing happens, then:
//! cache.invalidate();          // bumps generation
//!
//! if !cache.is_valid(gen) {
//!     let pixels = render(doc);
//!     cache.store(pixels);
//! }
//!
//! // On the next frame, if no edits occurred:
//! let new_gen = cache.generation();
//! if cache.is_valid(new_gen) {
//!     // reuse cache.cached_pixmap() -- no render needed
//! }
//! ```

/// Cache for the most recently rendered frame.
///
/// The cache tracks a monotonically increasing *generation* number.  Every
/// call to [`invalidate`](RenderCache::invalidate) bumps the generation,
/// signalling that the previously cached pixmap is stale.
#[derive(Debug, Clone)]
pub struct RenderCache {
    /// Current generation counter.  Bumped on every invalidation.
    generation: u64,
    /// The generation at which the cached pixmap was stored.
    pixmap_generation: u64,
    /// Width (in pixels) of the cached pixmap.
    width: u32,
    /// Height (in pixels) of the cached pixmap.
    height: u32,
    /// The cached premultiplied RGBA pixel data from the last render pass.
    ///
    /// `None` until the first frame has been rendered and stored.
    cached_pixmap: Option<Vec<u8>>,
}

impl RenderCache {
    /// Create a new render cache with generation 0 and no cached data.
    pub fn new() -> Self {
        Self {
            generation: 0,
            pixmap_generation: u64::MAX, // sentinel: no pixmap stored yet
            width: 0,
            height: 0,
            cached_pixmap: None,
        }
    }

    /// Return the current generation counter.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Check whether a previously observed generation is still current.
    ///
    /// Returns `true` when `gen` equals the current generation **and** a
    /// cached pixmap is available, meaning the cache can be reused without
    /// re-rendering.
    pub fn is_valid(&self, gen: u64) -> bool {
        gen == self.generation
            && self.pixmap_generation == self.generation
            && self.cached_pixmap.is_some()
    }

    /// Invalidate the cache by bumping the generation counter.
    ///
    /// The stored pixmap is **not** cleared immediately -- it remains
    /// available via [`cached_pixmap`](RenderCache::cached_pixmap) until
    /// a new frame is stored.  However, [`is_valid`](RenderCache::is_valid)
    /// will return `false` for any previously recorded generation.
    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Store the rendered pixel data, associating it with the current
    /// generation.
    pub fn store(&mut self, pixels: Vec<u8>) {
        self.pixmap_generation = self.generation;
        self.cached_pixmap = Some(pixels);
    }

    /// Store rendered pixel data along with its dimensions.
    pub fn store_with_size(&mut self, pixels: Vec<u8>, width: u32, height: u32) {
        self.pixmap_generation = self.generation;
        self.width = width;
        self.height = height;
        self.cached_pixmap = Some(pixels);
    }

    /// Return a reference to the cached pixmap, if any.
    pub fn cached_pixmap(&self) -> Option<&[u8]> {
        self.cached_pixmap.as_deref()
    }

    /// Width of the cached pixmap in pixels (0 if not yet stored with size).
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of the cached pixmap in pixels (0 if not yet stored with size).
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Clear the cached pixmap data, freeing memory.
    ///
    /// The generation is **not** changed, so a subsequent `is_valid` call
    /// with the current generation will return `false` (because no pixmap is
    /// stored).
    pub fn clear(&mut self) {
        self.cached_pixmap = None;
        self.pixmap_generation = u64::MAX;
        self.width = 0;
        self.height = 0;
    }
}

impl Default for RenderCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_cache_starts_at_generation_zero() {
        let cache = RenderCache::new();
        assert_eq!(cache.generation(), 0);
    }

    #[test]
    fn new_cache_has_no_pixmap() {
        let cache = RenderCache::new();
        assert!(cache.cached_pixmap().is_none());
    }

    #[test]
    fn not_valid_without_pixmap() {
        let cache = RenderCache::new();
        assert!(!cache.is_valid(0));
    }

    #[test]
    fn store_makes_valid() {
        let mut cache = RenderCache::new();
        cache.store(vec![0u8; 16]);
        assert!(cache.is_valid(0));
    }

    #[test]
    fn invalidate_bumps_generation() {
        let mut cache = RenderCache::new();
        assert_eq!(cache.generation(), 0);
        cache.invalidate();
        assert_eq!(cache.generation(), 1);
        cache.invalidate();
        assert_eq!(cache.generation(), 2);
    }

    #[test]
    fn old_generation_is_not_valid() {
        let mut cache = RenderCache::new();
        cache.store(vec![0u8; 16]);
        assert!(cache.is_valid(0));
        cache.invalidate();
        // Old generation is stale.
        assert!(!cache.is_valid(0));
        // New generation is also not valid because the pixmap was stored
        // at the previous generation.  A fresh `store` is required.
        assert!(!cache.is_valid(1));
        // The stale pixmap is still accessible for fallback display.
        assert!(cache.cached_pixmap().is_some());
    }

    #[test]
    fn store_after_invalidate() {
        let mut cache = RenderCache::new();
        cache.store(vec![1, 2, 3, 4]);
        cache.invalidate();
        let gen = cache.generation();
        cache.store(vec![5, 6, 7, 8]);
        assert!(cache.is_valid(gen));
        assert_eq!(cache.cached_pixmap().unwrap(), &[5, 6, 7, 8]);
    }

    #[test]
    fn clear_removes_pixmap() {
        let mut cache = RenderCache::new();
        cache.store(vec![1, 2, 3]);
        assert!(cache.cached_pixmap().is_some());
        cache.clear();
        assert!(cache.cached_pixmap().is_none());
        // Still generation 0 but no pixmap
        assert!(!cache.is_valid(0));
    }

    #[test]
    fn store_with_size() {
        let mut cache = RenderCache::new();
        cache.store_with_size(vec![0u8; 40], 10, 1);
        assert_eq!(cache.width(), 10);
        assert_eq!(cache.height(), 1);
        assert!(cache.cached_pixmap().is_some());
    }

    #[test]
    fn default_is_new() {
        let cache = RenderCache::default();
        assert_eq!(cache.generation(), 0);
        assert!(cache.cached_pixmap().is_none());
    }
}
