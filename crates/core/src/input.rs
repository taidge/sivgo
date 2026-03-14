//! Input event types for the editor engine.

/// A mouse event with position and modifiers.
#[derive(Debug, Clone, Copy)]
pub struct MouseEvent {
    /// X coordinate in screen pixels.
    pub x: f64,
    /// Y coordinate in screen pixels.
    pub y: f64,
    /// Which button was pressed/released.
    pub button: MouseButton,
    /// Active modifier keys.
    pub modifiers: Modifiers,
}

impl MouseEvent {
    /// Create a simple left-click event at (x, y) with no modifiers.
    pub fn left(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
        }
    }
}

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// Keyboard modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Modifiers {
    /// No modifiers pressed.
    pub fn none() -> Self {
        Self {
            ctrl: false,
            shift: false,
            alt: false,
        }
    }
}

impl Default for Modifiers {
    fn default() -> Self {
        Self::none()
    }
}

/// A keyboard event.
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub key: Key,
    pub modifiers: Modifiers,
}

/// Key identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// A specific character key (letters, digits, symbols).
    Char(char),
    Delete,
    Backspace,
    Escape,
    Enter,
    Tab,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Plus,
    Minus,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    PageUp,
    PageDown,
    Home,
    End,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}
