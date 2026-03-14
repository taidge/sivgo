//! Web auto-save scheduling and state tracking.
//!
//! Manages the timing and state of periodic auto-saves to the server,
//! tracking pending changes and save intervals.

/// Auto-save state and scheduler.
#[derive(Debug, Clone)]
pub struct WebAutoSave {
    /// Interval between auto-saves in milliseconds.
    pub interval_ms: u64,
    /// Whether auto-save is enabled.
    pub enabled: bool,
    /// Timestamp (ms since epoch) of the last successful save.
    pub last_save_time: Option<u64>,
    /// Whether there are unsaved changes waiting to be flushed.
    pub pending_changes: bool,
    /// Total number of successful auto-saves since the session started.
    pub save_count: u64,
}

impl Default for WebAutoSave {
    fn default() -> Self {
        Self {
            interval_ms: 30_000, // 30 seconds
            enabled: true,
            last_save_time: None,
            pending_changes: false,
            save_count: 0,
        }
    }
}

impl WebAutoSave {
    /// Create a new auto-save state with the given interval.
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval_ms,
            ..Default::default()
        }
    }

    /// Schedule auto-save (enable it if disabled).
    pub fn schedule(&mut self) {
        self.enabled = true;
    }

    /// Cancel (disable) auto-save.
    pub fn cancel(&mut self) {
        self.enabled = false;
    }

    /// Mark the document as having unsaved changes.
    pub fn mark_dirty(&mut self) {
        self.pending_changes = true;
    }

    /// Record that a save completed successfully at the given timestamp.
    pub fn on_save_complete(&mut self, timestamp: u64) {
        self.last_save_time = Some(timestamp);
        self.pending_changes = false;
        self.save_count += 1;
    }

    /// Determine whether an auto-save should fire at the given time.
    ///
    /// Returns `true` if all of the following are true:
    /// - Auto-save is enabled.
    /// - There are pending changes.
    /// - Enough time has elapsed since the last save (or no save has
    ///   occurred yet).
    pub fn should_save(&self, current_time: u64) -> bool {
        if !self.enabled || !self.pending_changes {
            return false;
        }
        match self.last_save_time {
            Some(last) => current_time.saturating_sub(last) >= self.interval_ms,
            None => true, // no previous save -- save immediately
        }
    }

    /// Reset the auto-save state (keeps the interval and enabled flag).
    pub fn reset(&mut self) {
        self.last_save_time = None;
        self.pending_changes = false;
        self.save_count = 0;
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let auto = WebAutoSave::default();
        assert_eq!(auto.interval_ms, 30_000);
        assert!(auto.enabled);
        assert!(!auto.pending_changes);
        assert!(auto.last_save_time.is_none());
        assert_eq!(auto.save_count, 0);
    }

    #[test]
    fn new_with_interval() {
        let auto = WebAutoSave::new(5_000);
        assert_eq!(auto.interval_ms, 5_000);
        assert!(auto.enabled);
    }

    #[test]
    fn schedule_and_cancel() {
        let mut auto = WebAutoSave::default();
        auto.cancel();
        assert!(!auto.enabled);
        auto.schedule();
        assert!(auto.enabled);
    }

    #[test]
    fn should_save_when_dirty_and_no_previous_save() {
        let mut auto = WebAutoSave::new(10_000);
        auto.mark_dirty();
        assert!(auto.should_save(0));
    }

    #[test]
    fn should_not_save_when_clean() {
        let auto = WebAutoSave::new(10_000);
        assert!(!auto.should_save(100_000));
    }

    #[test]
    fn should_not_save_when_disabled() {
        let mut auto = WebAutoSave::new(10_000);
        auto.mark_dirty();
        auto.cancel();
        assert!(!auto.should_save(100_000));
    }

    #[test]
    fn should_save_after_interval_elapsed() {
        let mut auto = WebAutoSave::new(10_000);
        auto.mark_dirty();
        auto.on_save_complete(1_000);

        // Too soon.
        auto.mark_dirty();
        assert!(!auto.should_save(5_000));

        // Exactly at the interval.
        assert!(auto.should_save(11_000));

        // After the interval.
        assert!(auto.should_save(20_000));
    }

    #[test]
    fn on_save_complete_increments_count() {
        let mut auto = WebAutoSave::default();
        auto.mark_dirty();
        auto.on_save_complete(1_000);
        assert_eq!(auto.save_count, 1);
        assert!(!auto.pending_changes);
        assert_eq!(auto.last_save_time, Some(1_000));

        auto.mark_dirty();
        auto.on_save_complete(2_000);
        assert_eq!(auto.save_count, 2);
    }

    #[test]
    fn reset_clears_state() {
        let mut auto = WebAutoSave::new(5_000);
        auto.mark_dirty();
        auto.on_save_complete(1_000);
        auto.mark_dirty();

        auto.reset();
        assert!(auto.last_save_time.is_none());
        assert!(!auto.pending_changes);
        assert_eq!(auto.save_count, 0);
        // Interval and enabled are preserved.
        assert_eq!(auto.interval_ms, 5_000);
        assert!(auto.enabled);
    }
}
