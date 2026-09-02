//! Tool trait and tool manager.

use std::collections::HashMap;

use sivgo_dom::SvgDocument;

use crate::command::Command;
use crate::input::{KeyEvent, MouseEvent};
use crate::selection::SelectionSet;
use crate::viewport::Viewport;

/// The response a tool returns after handling an input event.
pub enum ToolResponse {
    /// The event was ignored.
    None,
    /// The event was consumed but no command needs to be executed.
    Handled,
    /// The event produced a command that should be executed.
    ExecuteCommand(Box<dyn Command>),
}

/// Every tool must implement this trait.
pub trait Tool: std::fmt::Debug {
    /// Human-readable name.
    fn name(&self) -> &str;

    fn on_mouse_down(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse;

    fn on_mouse_move(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse;

    fn on_mouse_up(
        &mut self,
        event: MouseEvent,
        doc: &SvgDocument,
        selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse;

    fn on_key_down(
        &mut self,
        event: KeyEvent,
        doc: &SvgDocument,
        selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse;

    #[allow(unused_variables)]
    fn on_key_up(
        &mut self,
        event: KeyEvent,
        doc: &SvgDocument,
        selection: &SelectionSet,
        viewport: &Viewport,
    ) -> ToolResponse {
        ToolResponse::None
    }

    /// Called when this tool becomes the active tool.
    fn on_activate(&mut self) {}

    /// Called when this tool is deactivated.
    fn on_deactivate(&mut self) {}

    /// CSS cursor name for this tool.
    fn cursor(&self) -> &str {
        "default"
    }

    /// Downcast handle for reaching a tool's own settings.
    ///
    /// Tools are stored behind `dyn Tool`, so without this a caller cannot get
    /// at the concrete type to change, say, the number of points on the star
    /// tool. Every implementation is one line:
    ///
    /// ```ignore
    /// fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
    ///     self
    /// }
    /// ```
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// Shared counterpart of [`Tool::as_any_mut`], for reading settings.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Identifier for each tool type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolId {
    Select,
    Node,
    Rect,
    Ellipse,
    Star,
    Spiral,
    Pen,
    Pencil,
    Calligraphy,
    Text,
    Gradient,
    Dropper,
    BucketFill,
    Connector,
    Eraser,
    Spray,
    Tweak,
    Measure,
    Zoom,
    Mesh,
    Box3d,
    Marker,
    Pages,
    Lpe,
}

/// Owns all registered tools and tracks which one is active.
pub struct ToolManager {
    tools: HashMap<ToolId, Box<dyn Tool>>,
    active: ToolId,
}

impl ToolManager {
    /// Create a new tool manager with Select as the default active tool.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            active: ToolId::Select,
        }
    }

    /// Register a tool.
    pub fn register(&mut self, id: ToolId, tool: Box<dyn Tool>) {
        self.tools.insert(id, tool);
    }

    /// Switch to a different tool.
    ///
    /// If the target tool has not been registered, the switch is silently
    /// ignored so that the engine never enters a state where the active
    /// tool id has no backing implementation.
    pub fn switch_to(&mut self, id: ToolId) {
        if id == self.active {
            return;
        }
        if !self.tools.contains_key(&id) {
            return;
        }
        if let Some(tool) = self.tools.get_mut(&self.active) {
            tool.on_deactivate();
        }
        self.active = id;
        if let Some(tool) = self.tools.get_mut(&self.active) {
            tool.on_activate();
        }
    }

    /// Get a reference to the active tool.
    ///
    /// # Panics
    ///
    /// Panics if the active tool id has not been registered.
    pub fn active_tool(&self) -> &dyn Tool {
        self.tools
            .get(&self.active)
            .expect("active tool not registered")
            .as_ref()
    }

    /// Get a mutable reference to the active tool.
    ///
    /// # Panics
    ///
    /// Panics if the active tool id has not been registered.
    pub fn active_tool_mut(&mut self) -> &mut dyn Tool {
        self.tools
            .get_mut(&self.active)
            .expect("active tool not registered")
            .as_mut()
    }

    /// Borrow a registered tool as its concrete type.
    ///
    /// Returns `None` when the tool is not registered or is a different type.
    /// This is how a settings panel reaches a tool's own options — the manager
    /// stores them behind `dyn Tool`, so there is otherwise no way to call, for
    /// example, `StarTool::set_points`.
    pub fn get<T: Tool + 'static>(&self, id: ToolId) -> Option<&T> {
        self.tools.get(&id)?.as_any().downcast_ref::<T>()
    }

    /// Mutable counterpart of [`ToolManager::get`].
    pub fn get_mut<T: Tool + 'static>(&mut self, id: ToolId) -> Option<&mut T> {
        self.tools.get_mut(&id)?.as_any_mut().downcast_mut::<T>()
    }

    /// The currently active tool id.
    pub fn active_id(&self) -> ToolId {
        self.active
    }
}

impl Default for ToolManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ToolManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolManager")
            .field("active", &self.active)
            .field("registered", &self.tools.keys().collect::<Vec<_>>())
            .finish()
    }
}
