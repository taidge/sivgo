//! Keyboard navigation / focus management (17.7.1).
//!
//! Provides a focus ring that tracks which UI element has keyboard focus,
//! supports Tab / Shift+Tab cycling, focus trapping for dialogs, and
//! programmatic focus by element ID.

/// The type of focusable UI element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusElementType {
    /// A dockable panel (layers, properties, etc.).
    Panel,
    /// A tool button in the toolbar.
    Tool,
    /// A menu or menu item.
    Menu,
    /// A modal or non-modal dialog.
    Dialog,
    /// The main drawing canvas.
    Canvas,
}

/// A single focusable element tracked by the [`FocusManager`].
#[derive(Debug, Clone)]
pub struct FocusableElement {
    /// Unique identifier for this element.
    pub id: String,
    /// What kind of UI element this is.
    pub element_type: FocusElementType,
    /// Tab order index; lower values receive focus first.
    pub tab_index: i32,
    /// Human-readable label (used for accessibility announcements).
    pub label: String,
}

/// Manages keyboard focus across the application.
///
/// Maintains an ordered ring of focusable elements and provides methods to
/// move focus forward, backward, jump to a specific element, and optionally
/// trap focus within a subset (e.g., inside a modal dialog).
#[derive(Debug, Clone)]
pub struct FocusManager {
    /// Ordered list of focusable elements (sorted by `tab_index`).
    pub focus_ring: Vec<FocusableElement>,
    /// Index into `focus_ring` of the currently focused element.
    pub current_index: usize,
    /// When `true`, focus cycles only within the current ring
    /// (used for modal dialogs).
    pub trap_focus: bool,
}

impl FocusManager {
    /// Create a new, empty focus manager.
    pub fn new() -> Self {
        Self {
            focus_ring: Vec::new(),
            current_index: 0,
            trap_focus: false,
        }
    }

    /// Register a focusable element. The element is inserted in sorted
    /// order by `tab_index`.
    pub fn register(&mut self, element: FocusableElement) {
        // Check for duplicate IDs.
        if self.focus_ring.iter().any(|e| e.id == element.id) {
            return;
        }

        let tab = element.tab_index;
        let pos = self
            .focus_ring
            .iter()
            .position(|e| e.tab_index > tab)
            .unwrap_or(self.focus_ring.len());
        self.focus_ring.insert(pos, element);
    }

    /// Unregister (remove) a focusable element by its ID.
    pub fn unregister(&mut self, id: &str) {
        if let Some(pos) = self.focus_ring.iter().position(|e| e.id == id) {
            self.focus_ring.remove(pos);
            if self.focus_ring.is_empty() {
                self.current_index = 0;
            } else if self.current_index >= self.focus_ring.len() {
                self.current_index = self.focus_ring.len() - 1;
            }
        }
    }

    /// Move focus to the next element in the ring.
    pub fn next(&mut self) {
        if self.focus_ring.is_empty() {
            return;
        }
        self.current_index = (self.current_index + 1) % self.focus_ring.len();
    }

    /// Move focus to the previous element in the ring.
    pub fn previous(&mut self) {
        if self.focus_ring.is_empty() {
            return;
        }
        if self.current_index == 0 {
            self.current_index = self.focus_ring.len() - 1;
        } else {
            self.current_index -= 1;
        }
    }

    /// Move focus to the element with the given `id`.
    /// Returns `true` if the element was found and focused.
    pub fn focus_by_id(&mut self, id: &str) -> bool {
        if let Some(pos) = self.focus_ring.iter().position(|e| e.id == id) {
            self.current_index = pos;
            true
        } else {
            false
        }
    }

    /// Get a reference to the currently focused element, if any.
    pub fn get_current(&self) -> Option<&FocusableElement> {
        self.focus_ring.get(self.current_index)
    }

    /// Enable or disable focus trapping (for modal dialogs).
    pub fn set_trap(&mut self, enabled: bool) {
        self.trap_focus = enabled;
    }

    /// Get the number of registered focusable elements.
    pub fn len(&self) -> usize {
        self.focus_ring.len()
    }

    /// Whether the focus ring is empty.
    pub fn is_empty(&self) -> bool {
        self.focus_ring.is_empty()
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_element(id: &str, typ: FocusElementType, tab: i32) -> FocusableElement {
        FocusableElement {
            id: id.to_string(),
            element_type: typ,
            tab_index: tab,
            label: id.to_string(),
        }
    }

    #[test]
    fn new_manager_is_empty() {
        let fm = FocusManager::new();
        assert!(fm.is_empty());
        assert_eq!(fm.len(), 0);
        assert!(fm.get_current().is_none());
    }

    #[test]
    fn register_adds_elements() {
        let mut fm = FocusManager::new();
        fm.register(make_element("canvas", FocusElementType::Canvas, 0));
        fm.register(make_element("toolbar", FocusElementType::Tool, 1));
        assert_eq!(fm.len(), 2);
    }

    #[test]
    fn register_sorted_by_tab_index() {
        let mut fm = FocusManager::new();
        fm.register(make_element("b", FocusElementType::Panel, 10));
        fm.register(make_element("a", FocusElementType::Tool, 1));
        fm.register(make_element("c", FocusElementType::Menu, 5));

        let ids: Vec<&str> = fm.focus_ring.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "c", "b"]);
    }

    #[test]
    fn register_duplicate_ignored() {
        let mut fm = FocusManager::new();
        fm.register(make_element("a", FocusElementType::Panel, 0));
        fm.register(make_element("a", FocusElementType::Tool, 1));
        assert_eq!(fm.len(), 1);
    }

    #[test]
    fn unregister_removes_element() {
        let mut fm = FocusManager::new();
        fm.register(make_element("a", FocusElementType::Panel, 0));
        fm.register(make_element("b", FocusElementType::Tool, 1));
        fm.unregister("a");
        assert_eq!(fm.len(), 1);
        assert_eq!(fm.focus_ring[0].id, "b");
    }

    #[test]
    fn unregister_adjusts_index() {
        let mut fm = FocusManager::new();
        fm.register(make_element("a", FocusElementType::Panel, 0));
        fm.register(make_element("b", FocusElementType::Tool, 1));
        fm.current_index = 1; // focus on "b"
        fm.unregister("b");
        assert_eq!(fm.current_index, 0);
    }

    #[test]
    fn next_cycles_forward() {
        let mut fm = FocusManager::new();
        fm.register(make_element("a", FocusElementType::Panel, 0));
        fm.register(make_element("b", FocusElementType::Tool, 1));
        fm.register(make_element("c", FocusElementType::Menu, 2));

        assert_eq!(fm.get_current().unwrap().id, "a");
        fm.next();
        assert_eq!(fm.get_current().unwrap().id, "b");
        fm.next();
        assert_eq!(fm.get_current().unwrap().id, "c");
        fm.next();
        assert_eq!(fm.get_current().unwrap().id, "a"); // wraps around
    }

    #[test]
    fn previous_cycles_backward() {
        let mut fm = FocusManager::new();
        fm.register(make_element("a", FocusElementType::Panel, 0));
        fm.register(make_element("b", FocusElementType::Tool, 1));
        fm.register(make_element("c", FocusElementType::Menu, 2));

        assert_eq!(fm.get_current().unwrap().id, "a");
        fm.previous();
        assert_eq!(fm.get_current().unwrap().id, "c"); // wraps backward
        fm.previous();
        assert_eq!(fm.get_current().unwrap().id, "b");
    }

    #[test]
    fn focus_by_id_works() {
        let mut fm = FocusManager::new();
        fm.register(make_element("a", FocusElementType::Panel, 0));
        fm.register(make_element("b", FocusElementType::Tool, 1));
        fm.register(make_element("c", FocusElementType::Menu, 2));

        assert!(fm.focus_by_id("c"));
        assert_eq!(fm.get_current().unwrap().id, "c");
    }

    #[test]
    fn focus_by_id_not_found() {
        let mut fm = FocusManager::new();
        fm.register(make_element("a", FocusElementType::Panel, 0));
        assert!(!fm.focus_by_id("nonexistent"));
        assert_eq!(fm.get_current().unwrap().id, "a"); // unchanged
    }

    #[test]
    fn set_trap_toggles() {
        let mut fm = FocusManager::new();
        assert!(!fm.trap_focus);
        fm.set_trap(true);
        assert!(fm.trap_focus);
        fm.set_trap(false);
        assert!(!fm.trap_focus);
    }

    #[test]
    fn next_on_empty_is_noop() {
        let mut fm = FocusManager::new();
        fm.next(); // should not panic
        fm.previous(); // should not panic
        assert!(fm.get_current().is_none());
    }

    #[test]
    fn element_type_equality() {
        assert_eq!(FocusElementType::Panel, FocusElementType::Panel);
        assert_ne!(FocusElementType::Panel, FocusElementType::Tool);
    }

    #[test]
    fn get_current_returns_correct_element() {
        let mut fm = FocusManager::new();
        fm.register(make_element("canvas", FocusElementType::Canvas, 0));
        let current = fm.get_current().unwrap();
        assert_eq!(current.id, "canvas");
        assert_eq!(current.element_type, FocusElementType::Canvas);
        assert_eq!(current.tab_index, 0);
        assert_eq!(current.label, "canvas");
    }
}
