//! Transform panel data model (11.4.5).
//!
//! Pure data structures for position, scale, rotation, and skew state.

/// The complete transform panel state for a selection.
#[derive(Debug, Clone, PartialEq)]
pub struct TransformState {
    /// Horizontal translation (move X).
    pub move_x: f64,
    /// Vertical translation (move Y).
    pub move_y: f64,
    /// Scale width (absolute value or percentage depending on context).
    pub scale_w: f64,
    /// Scale height (absolute value or percentage depending on context).
    pub scale_h: f64,
    /// Whether scaling preserves the aspect ratio.
    pub scale_proportional: bool,
    /// Rotation angle in degrees.
    pub rotate_angle: f64,
    /// Horizontal skew angle in degrees.
    pub skew_h: f64,
    /// Vertical skew angle in degrees.
    pub skew_v: f64,
    /// Whether transformations are applied to each object individually
    /// (as opposed to the group bounding box).
    pub apply_to_each: bool,
}

impl Default for TransformState {
    fn default() -> Self {
        Self {
            move_x: 0.0,
            move_y: 0.0,
            scale_w: 100.0,
            scale_h: 100.0,
            scale_proportional: true,
            rotate_angle: 0.0,
            skew_h: 0.0,
            skew_v: 0.0,
            apply_to_each: false,
        }
    }
}

impl TransformState {
    /// Return `true` if this state represents the identity transform
    /// (no move, 100% scale, no rotation, no skew).
    pub fn is_identity(&self) -> bool {
        self.move_x == 0.0
            && self.move_y == 0.0
            && (self.scale_w - 100.0).abs() < f64::EPSILON
            && (self.scale_h - 100.0).abs() < f64::EPSILON
            && self.rotate_angle == 0.0
            && self.skew_h == 0.0
            && self.skew_v == 0.0
    }

    /// Reset all fields to their default (identity) values.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// If `scale_proportional` is set, adjust `scale_h` to match the
    /// ratio implied by `scale_w`, given the original aspect ratio
    /// `original_w / original_h`.
    pub fn enforce_proportional(&mut self, original_w: f64, original_h: f64) {
        if self.scale_proportional && original_w > 0.0 && original_h > 0.0 {
            self.scale_h = self.scale_w;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_identity() {
        let state = TransformState::default();
        assert!(state.is_identity());
    }

    #[test]
    fn non_identity_after_move() {
        let mut state = TransformState::default();
        state.move_x = 10.0;
        assert!(!state.is_identity());
    }

    #[test]
    fn reset_restores_identity() {
        let mut state = TransformState::default();
        state.move_x = 42.0;
        state.rotate_angle = 90.0;
        state.reset();
        assert!(state.is_identity());
    }
}
