//! Touch input handling and gesture detection.
//!
//! Processes multi-touch events and detects gestures such as pan, pinch,
//! rotate, and long-press for use on touch-screen devices.

/// A single touch contact point.
#[derive(Debug, Clone, Copy)]
pub struct TouchPoint {
    /// Unique identifier for the touch (from the browser).
    pub id: u32,
    /// X coordinate in CSS pixels.
    pub x: f64,
    /// Y coordinate in CSS pixels.
    pub y: f64,
    /// Pressure / force of the touch (0.0 to 1.0).
    pub force: f64,
}

/// A recognised touch gesture.
#[derive(Debug, Clone, PartialEq)]
pub enum TouchGesture {
    /// A single-finger drag.
    Pan { dx: f64, dy: f64 },
    /// A two-finger pinch (zoom).
    Pinch {
        scale: f64,
        center: (f64, f64),
    },
    /// A two-finger rotation.
    Rotate { angle: f64 },
    /// A sustained press without movement.
    LongPress { x: f64, y: f64 },
}

/// Tracks active touches and detects gestures.
#[derive(Debug, Clone)]
pub struct TouchState {
    /// Currently active touch points.
    pub touches: Vec<TouchPoint>,
    /// The last detected gesture, if any.
    pub gesture: Option<TouchGesture>,
    /// Current cumulative pinch scale factor.
    pub pinch_scale: f64,
    /// Current cumulative rotation angle in radians.
    pub rotation: f64,
    /// Snapshot of touches at the start of the gesture.
    prev_touches: Vec<TouchPoint>,
}

impl Default for TouchState {
    fn default() -> Self {
        Self {
            touches: Vec::new(),
            gesture: None,
            pinch_scale: 1.0,
            rotation: 0.0,
            prev_touches: Vec::new(),
        }
    }
}

impl TouchState {
    /// Create a new, empty touch state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle a `touchstart` event.
    pub fn on_touch_start(&mut self, touches: Vec<TouchPoint>) {
        self.touches = touches.clone();
        self.prev_touches = touches;
        self.gesture = None;
        self.pinch_scale = 1.0;
        self.rotation = 0.0;
    }

    /// Handle a `touchmove` event.
    pub fn on_touch_move(&mut self, touches: Vec<TouchPoint>) {
        self.prev_touches = self.touches.clone();
        self.touches = touches;
        self.gesture = self.detect_gesture();
    }

    /// Handle a `touchend` event.
    pub fn on_touch_end(&mut self, remaining: Vec<TouchPoint>) {
        self.touches = remaining;
        if self.touches.is_empty() {
            self.gesture = None;
            self.pinch_scale = 1.0;
            self.rotation = 0.0;
        }
    }

    /// Attempt to detect a gesture from the current and previous touch state.
    pub fn detect_gesture(&self) -> Option<TouchGesture> {
        match self.touches.len() {
            1 if self.prev_touches.len() == 1 => {
                let cur = &self.touches[0];
                let prev = &self.prev_touches[0];
                let dx = cur.x - prev.x;
                let dy = cur.y - prev.y;
                if dx.abs() > 0.5 || dy.abs() > 0.5 {
                    Some(TouchGesture::Pan { dx, dy })
                } else {
                    None
                }
            }
            2 if self.prev_touches.len() == 2 => {
                self.detect_two_finger_gesture()
            }
            _ => None,
        }
    }

    /// Detect a long-press gesture.
    ///
    /// This should be called by an external timer after a sustained press
    /// of ~500 ms without movement.  Returns `Some(LongPress)` if there
    /// is exactly one touch point that has not moved significantly.
    pub fn detect_long_press(&self) -> Option<TouchGesture> {
        if self.touches.len() == 1 && self.prev_touches.len() == 1 {
            let cur = &self.touches[0];
            let prev = &self.prev_touches[0];
            let dx = (cur.x - prev.x).abs();
            let dy = (cur.y - prev.y).abs();
            if dx < 10.0 && dy < 10.0 {
                return Some(TouchGesture::LongPress {
                    x: cur.x,
                    y: cur.y,
                });
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    fn detect_two_finger_gesture(&self) -> Option<TouchGesture> {
        let (p0, p1) = (&self.prev_touches[0], &self.prev_touches[1]);
        let (c0, c1) = (&self.touches[0], &self.touches[1]);

        let prev_dist = distance(p0.x, p0.y, p1.x, p1.y);
        let cur_dist = distance(c0.x, c0.y, c1.x, c1.y);

        let prev_angle = (p1.y - p0.y).atan2(p1.x - p0.x);
        let cur_angle = (c1.y - c0.y).atan2(c1.x - c0.x);
        let angle_delta = cur_angle - prev_angle;

        let center = ((c0.x + c1.x) / 2.0, (c0.y + c1.y) / 2.0);

        // Determine whether pinch or rotation dominates.
        let scale_ratio = if prev_dist > 0.0 {
            cur_dist / prev_dist
        } else {
            1.0
        };

        if (scale_ratio - 1.0).abs() > angle_delta.abs() {
            Some(TouchGesture::Pinch {
                scale: scale_ratio,
                center,
            })
        } else if angle_delta.abs() > 0.01 {
            Some(TouchGesture::Rotate {
                angle: angle_delta,
            })
        } else {
            None
        }
    }
}

/// Euclidean distance between two points.
fn distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt()
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(id: u32, x: f64, y: f64) -> TouchPoint {
        TouchPoint {
            id,
            x,
            y,
            force: 1.0,
        }
    }

    #[test]
    fn default_state() {
        let state = TouchState::new();
        assert!(state.touches.is_empty());
        assert!(state.gesture.is_none());
        assert!((state.pinch_scale - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn touch_start_records_points() {
        let mut state = TouchState::new();
        state.on_touch_start(vec![pt(0, 100.0, 200.0)]);
        assert_eq!(state.touches.len(), 1);
        assert!(state.gesture.is_none());
    }

    #[test]
    fn single_finger_pan() {
        let mut state = TouchState::new();
        state.on_touch_start(vec![pt(0, 100.0, 200.0)]);
        state.on_touch_move(vec![pt(0, 110.0, 205.0)]);

        match &state.gesture {
            Some(TouchGesture::Pan { dx, dy }) => {
                assert!((dx - 10.0).abs() < f64::EPSILON);
                assert!((dy - 5.0).abs() < f64::EPSILON);
            }
            other => panic!("Expected Pan, got {other:?}"),
        }
    }

    #[test]
    fn two_finger_pinch() {
        let mut state = TouchState::new();
        // Two fingers 100px apart.
        state.on_touch_start(vec![pt(0, 100.0, 200.0), pt(1, 200.0, 200.0)]);
        // Move them further apart to 200px.
        state.on_touch_move(vec![pt(0, 50.0, 200.0), pt(1, 250.0, 200.0)]);

        match &state.gesture {
            Some(TouchGesture::Pinch { scale, center }) => {
                assert!(*scale > 1.0); // zooming in
                assert!((center.1 - 200.0).abs() < f64::EPSILON);
            }
            other => panic!("Expected Pinch, got {other:?}"),
        }
    }

    #[test]
    fn touch_end_clears_gesture() {
        let mut state = TouchState::new();
        state.on_touch_start(vec![pt(0, 100.0, 200.0)]);
        state.on_touch_move(vec![pt(0, 110.0, 200.0)]);
        assert!(state.gesture.is_some());

        state.on_touch_end(vec![]);
        assert!(state.gesture.is_none());
        assert!(state.touches.is_empty());
    }

    #[test]
    fn long_press_detection() {
        let mut state = TouchState::new();
        state.on_touch_start(vec![pt(0, 100.0, 200.0)]);
        // Tiny movement (within threshold).
        state.on_touch_move(vec![pt(0, 101.0, 201.0)]);

        let lp = state.detect_long_press();
        assert!(lp.is_some());
        match lp.unwrap() {
            TouchGesture::LongPress { x, y } => {
                assert!((x - 101.0).abs() < f64::EPSILON);
                assert!((y - 201.0).abs() < f64::EPSILON);
            }
            other => panic!("Expected LongPress, got {other:?}"),
        }
    }

    #[test]
    fn no_long_press_if_moved() {
        let mut state = TouchState::new();
        state.on_touch_start(vec![pt(0, 100.0, 200.0)]);
        state.on_touch_move(vec![pt(0, 200.0, 300.0)]); // large movement
        assert!(state.detect_long_press().is_none());
    }

    #[test]
    fn distance_fn() {
        assert!((distance(0.0, 0.0, 3.0, 4.0) - 5.0).abs() < f64::EPSILON);
        assert!((distance(1.0, 1.0, 1.0, 1.0)).abs() < f64::EPSILON);
    }
}
