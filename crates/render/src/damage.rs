//! Damage tracking for incremental rendering.
//!
//! [`DamageTracker`] records rectangular regions of the canvas that have been
//! modified ("damaged") since the last render pass.  The renderer can query
//! the dirty regions to avoid re-rasterizing unchanged parts of the document.

use sivgo_math::Rect;

/// Tracks dirty rectangular regions on the canvas.
///
/// After elements are edited, callers mark the affected areas with
/// [`mark_dirty`](DamageTracker::mark_dirty).  Before rendering, the backend
/// can inspect [`dirty_regions`](DamageTracker::dirty_regions) (or
/// [`union_all`](DamageTracker::union_all)) to decide which parts of the
/// canvas to re-rasterize.  After rendering, call
/// [`clear`](DamageTracker::clear) to reset the tracker for the next frame.
#[derive(Debug, Clone)]
pub struct DamageTracker {
    regions: Vec<Rect>,
}

impl DamageTracker {
    /// Create a new, empty damage tracker.
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Mark a rectangular region as dirty (needs re-rendering).
    ///
    /// Empty rectangles (zero or negative area) are silently ignored.
    pub fn mark_dirty(&mut self, rect: Rect) {
        if !rect.is_empty() {
            self.regions.push(rect);
        }
    }

    /// Return a slice of all currently-dirty regions.
    pub fn dirty_regions(&self) -> &[Rect] {
        &self.regions
    }

    /// Return the number of dirty regions currently tracked.
    pub fn dirty_count(&self) -> usize {
        self.regions.len()
    }

    /// Returns `true` when there are no dirty regions.
    pub fn is_clean(&self) -> bool {
        self.regions.is_empty()
    }

    /// Clear all recorded damage, typically called after a render pass.
    pub fn clear(&mut self) {
        self.regions.clear();
    }

    /// Compute the axis-aligned bounding box that encloses every dirty region.
    ///
    /// Returns `None` when there are no dirty regions.
    pub fn union_all(&self) -> Option<Rect> {
        let mut iter = self.regions.iter();
        let first = *iter.next()?;
        Some(iter.fold(first, |acc, r| acc.union(*r)))
    }

    /// Check whether a given rectangle overlaps any dirty region.
    ///
    /// This is useful for culling: if an element's bounding box does not
    /// intersect any damaged area, it does not need to be re-rendered.
    pub fn intersects_any(&self, rect: Rect) -> bool {
        self.regions.iter().any(|r| {
            let isect = r.intersect(rect);
            !isect.is_empty()
        })
    }
}

impl Default for DamageTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tracker_is_clean() {
        let tracker = DamageTracker::new();
        assert!(tracker.is_clean());
        assert_eq!(tracker.dirty_count(), 0);
        assert!(tracker.dirty_regions().is_empty());
        assert!(tracker.union_all().is_none());
    }

    #[test]
    fn mark_dirty_adds_region() {
        let mut tracker = DamageTracker::new();
        tracker.mark_dirty(Rect::new(10.0, 20.0, 50.0, 60.0));
        assert!(!tracker.is_clean());
        assert_eq!(tracker.dirty_count(), 1);
        assert_eq!(
            tracker.dirty_regions()[0],
            Rect::new(10.0, 20.0, 50.0, 60.0)
        );
    }

    #[test]
    fn empty_rect_is_ignored() {
        let mut tracker = DamageTracker::new();
        // Zero-width rect
        tracker.mark_dirty(Rect::new(10.0, 20.0, 10.0, 60.0));
        assert!(tracker.is_clean());
        // Negative-area rect
        tracker.mark_dirty(Rect::new(50.0, 50.0, 10.0, 10.0));
        assert!(tracker.is_clean());
    }

    #[test]
    fn clear_resets_tracker() {
        let mut tracker = DamageTracker::new();
        tracker.mark_dirty(Rect::new(0.0, 0.0, 100.0, 100.0));
        tracker.mark_dirty(Rect::new(50.0, 50.0, 200.0, 200.0));
        assert_eq!(tracker.dirty_count(), 2);
        tracker.clear();
        assert!(tracker.is_clean());
        assert_eq!(tracker.dirty_count(), 0);
    }

    #[test]
    fn union_all_returns_bounding_box() {
        let mut tracker = DamageTracker::new();
        tracker.mark_dirty(Rect::new(10.0, 20.0, 50.0, 60.0));
        tracker.mark_dirty(Rect::new(30.0, 10.0, 80.0, 90.0));
        let bbox = tracker.union_all().unwrap();
        assert_eq!(bbox, Rect::new(10.0, 10.0, 80.0, 90.0));
    }

    #[test]
    fn union_all_single_region() {
        let mut tracker = DamageTracker::new();
        let r = Rect::new(5.0, 5.0, 15.0, 15.0);
        tracker.mark_dirty(r);
        assert_eq!(tracker.union_all().unwrap(), r);
    }

    #[test]
    fn intersects_any_positive() {
        let mut tracker = DamageTracker::new();
        tracker.mark_dirty(Rect::new(10.0, 10.0, 50.0, 50.0));
        // Overlapping query
        assert!(tracker.intersects_any(Rect::new(40.0, 40.0, 80.0, 80.0)));
        // Fully inside
        assert!(tracker.intersects_any(Rect::new(20.0, 20.0, 30.0, 30.0)));
    }

    #[test]
    fn intersects_any_negative() {
        let mut tracker = DamageTracker::new();
        tracker.mark_dirty(Rect::new(10.0, 10.0, 50.0, 50.0));
        // Completely outside
        assert!(!tracker.intersects_any(Rect::new(60.0, 60.0, 100.0, 100.0)));
        // Touching edge only (zero-area intersection)
        assert!(!tracker.intersects_any(Rect::new(50.0, 50.0, 100.0, 100.0)));
    }

    #[test]
    fn intersects_any_when_clean() {
        let tracker = DamageTracker::new();
        assert!(!tracker.intersects_any(Rect::new(0.0, 0.0, 100.0, 100.0)));
    }

    #[test]
    fn multiple_dirty_regions_union() {
        let mut tracker = DamageTracker::new();
        tracker.mark_dirty(Rect::new(0.0, 0.0, 10.0, 10.0));
        tracker.mark_dirty(Rect::new(90.0, 90.0, 100.0, 100.0));
        tracker.mark_dirty(Rect::new(40.0, 40.0, 60.0, 60.0));
        let bbox = tracker.union_all().unwrap();
        assert_eq!(bbox, Rect::new(0.0, 0.0, 100.0, 100.0));
        assert_eq!(tracker.dirty_count(), 3);
    }

    #[test]
    fn default_is_clean() {
        let tracker = DamageTracker::default();
        assert!(tracker.is_clean());
    }
}
