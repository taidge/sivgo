/// Normalized native wheel units; screen pixels, lines, and pages use distinct zoom scales.
#[derive(Clone, Copy)]
enum WheelDelta {
    Pixels(f64),
    #[cfg(test)]
    Lines(f64),
    #[cfg(test)]
    Pages(f64),
}
impl WheelDelta {
    fn pixels(_: f64, y: f64, _: f64) -> Self {
        Self::Pixels(y)
    }
    #[cfg(test)]
    fn lines(_: f64, y: f64, _: f64) -> Self {
        Self::Lines(y)
    }
    #[cfg(test)]
    fn pages(_: f64, y: f64, _: f64) -> Self {
        Self::Pages(y)
    }
}
use keyboard_types::Key;
use keyboard_types::{Code, Modifiers as AcceleratorModifiers};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Canvas dimensions in pixels.
const CANVAS_WIDTH: u32 = 800;
const CANVAS_HEIGHT: u32 = 600;
const PX_PER_MM: f64 = 96.0 / 25.4;
const CURSOR_UPDATE_SCREEN_EPSILON: f64 = 8.0;
const INSPECTOR_WIDTH_DEFAULT: f64 = 300.0;
const INSPECTOR_WIDTH_MIN: f64 = 248.0;
const INSPECTOR_WIDTH_MAX: f64 = 520.0;
const LEFT_RAIL_WIDTH: f64 = 44.0;
const INSPECTOR_RESIZE_HANDLE_WIDTH: f64 = 8.0;
const CANVAS_RULER_THICKNESS: f64 = 22.0;
const CANVAS_SCROLLBAR_THICKNESS: f64 = 14.0;
const ZOOM_MIN: f64 = 0.05;
const ZOOM_MAX: f64 = 64.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorPanel {
    Style,
    Text,
    Objects,
    Pages,
    Align,
    Transform,
    Lpe,
    Filters,
    Resources,
    Document,
    Preferences,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CanvasDragPreview {
    tool: ToolId,
    start_x: f64,
    start_y: f64,
    current_x: f64,
    current_y: f64,
    ctrl: bool,
    shift: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct CanvasPathPreview {
    tool: ToolId,
    points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CanvasSelectPreview {
    start_x: f64,
    start_y: f64,
    current_x: f64,
    current_y: f64,
    selection_bounds: Option<Rect>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct InspectorResizeDrag {
    start_client_x: f64,
    start_width_px: f64,
}

impl InspectorResizeDrag {
    fn width_at(self, current_client_x: f64) -> f64 {
        sanitize_inspector_width(self.start_width_px - (current_client_x - self.start_client_x))
    }
}

impl CanvasDragPreview {
    fn new(tool: ToolId, x: f64, y: f64, ctrl: bool, shift: bool) -> Self {
        Self {
            tool,
            start_x: x,
            start_y: y,
            current_x: x,
            current_y: y,
            ctrl,
            shift,
        }
    }

    fn with_current(mut self, x: f64, y: f64, ctrl: bool, shift: bool) -> Self {
        self.current_x = x;
        self.current_y = y;
        self.ctrl = ctrl;
        self.shift = shift;
        self
    }
}

impl CanvasPathPreview {
    fn new(tool: ToolId, x: f64, y: f64) -> Self {
        Self {
            tool,
            points: vec![(x, y)],
        }
    }

    fn line(tool: ToolId, x: f64, y: f64) -> Self {
        Self {
            tool,
            points: vec![(x, y), (x, y)],
        }
    }

    fn with_point(mut self, x: f64, y: f64) -> Self {
        self.points.push((x, y));
        self
    }

    fn with_current(mut self, x: f64, y: f64) -> Self {
        if self.points.len() == 1 {
            self.points.push((x, y));
        } else if let Some(last) = self.points.last_mut() {
            *last = (x, y);
        } else {
            self.points.push((x, y));
        }
        self
    }
}

impl CanvasSelectPreview {
    fn new(start_x: f64, start_y: f64, selection_bounds: Option<Rect>) -> Self {
        Self {
            start_x,
            start_y,
            current_x: start_x,
            current_y: start_y,
            selection_bounds,
        }
    }

    fn with_current(mut self, x: f64, y: f64) -> Self {
        self.current_x = x;
        self.current_y = y;
        self
    }

    fn screen_delta(self) -> (f64, f64) {
        (self.current_x - self.start_x, self.current_y - self.start_y)
    }

    fn is_active(self) -> bool {
        let (dx, dy) = self.screen_delta();
        (dx * dx + dy * dy).sqrt() >= 3.0
    }
}

#[cfg(test)]
fn in_window_menu_rendered() -> bool {
    true
}

fn inspector_panel_for_tool(tool: ToolId) -> Option<InspectorPanel> {
    match tool {
        ToolId::Pages => Some(InspectorPanel::Pages),
        ToolId::Text => Some(InspectorPanel::Text),
        ToolId::Lpe => Some(InspectorPanel::Lpe),
        _ => None,
    }
}

#[cfg(not(test))]
static INITIAL_DESKTOP_SETTINGS: std::sync::OnceLock<DesktopSettings> = std::sync::OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct DesktopSettings {
    preferences: PreferencesState,
    document_defaults: DocumentDefaults,
    layout: LayoutSettings,
}

impl DesktopSettings {
    fn sanitized(mut self) -> Self {
        self.preferences.auto_save_interval_secs =
            self.preferences.auto_save_interval_secs.clamp(10, 86_400);
        self.preferences.recent_files_count = self.preferences.recent_files_count.min(50);
        self.preferences.snap_distance =
            sanitize_positive(self.preferences.snap_distance, 10.0, 1.0, 512.0);
        self.document_defaults = self.document_defaults.sanitized();
        self.layout = self.layout.sanitized();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct LayoutSettings {
    inspector_width_px: f64,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        Self {
            inspector_width_px: INSPECTOR_WIDTH_DEFAULT,
        }
    }
}

impl LayoutSettings {
    fn sanitized(mut self) -> Self {
        self.inspector_width_px = sanitize_inspector_width(self.inspector_width_px);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct DocumentDefaults {
    width_px: f64,
    height_px: f64,
    background_color: String,
    title: String,
    author: String,
    description: String,
    grid_enabled: bool,
    grid_spacing_x: f64,
    grid_spacing_y: f64,
    snap_to_grid: bool,
    snap_to_guides: bool,
}

impl Default for DocumentDefaults {
    fn default() -> Self {
        Self {
            width_px: CANVAS_WIDTH as f64,
            height_px: CANVAS_HEIGHT as f64,
            background_color: "#ffffff".to_string(),
            title: String::new(),
            author: String::new(),
            description: String::new(),
            grid_enabled: false,
            grid_spacing_x: 10.0,
            grid_spacing_y: 10.0,
            snap_to_grid: true,
            snap_to_guides: true,
        }
    }
}

impl DocumentDefaults {
    fn sanitized(mut self) -> Self {
        self.width_px = sanitize_positive(self.width_px, CANVAS_WIDTH as f64, 1.0, 16_384.0);
        self.height_px = sanitize_positive(self.height_px, CANVAS_HEIGHT as f64, 1.0, 16_384.0);
        self.background_color = sanitize_hex_string(&self.background_color, "#ffffff");
        self.grid_spacing_x = sanitize_positive(self.grid_spacing_x, 10.0, 0.1, 10_000.0);
        self.grid_spacing_y = sanitize_positive(self.grid_spacing_y, 10.0, 0.1, 10_000.0);
        self
    }

    fn from_engine(engine: &EditorEngine) -> Self {
        let (width_px, height_px) = engine.document_size();
        let grid_spacing = engine.snapping.prefs.grid_spacing;
        Self {
            width_px,
            height_px,
            background_color: page_color(engine),
            title: root_text(engine, "title"),
            author: root_text(engine, "metadata"),
            description: root_text(engine, "desc"),
            grid_enabled: grid_visible(engine),
            grid_spacing_x: grid_spacing,
            grid_spacing_y: grid_spacing,
            snap_to_grid: engine.snapping.prefs.snap_to_grid,
            snap_to_guides: engine.snapping.prefs.snap_to_guides,
        }
        .sanitized()
    }
}

/// Embedded CSS for the desktop editor chrome.
const CSS: &str = r#"
/* ===================================================================
   Sivgo Studio – Professional SVG Editor Theme
   =================================================================== */
:root {
    --bg-0: #1b1b1f;
    --bg-1: #232328;
    --bg-2: #2c2c32;
    --bg-3: #38383f;
    --panel-bg: #232328;
    --panel-border: rgba(255, 255, 255, 0.07);
    --ink-0: #e8e8ec;
    --ink-1: #a0a0aa;
    --ink-2: #6e6e78;
    --ink-3: #53535c;
    --accent: #4d9cf6;
    --accent-hover: #6cb0ff;
    --accent-soft: rgba(77, 156, 246, 0.14);
    --accent-border: rgba(77, 156, 246, 0.3);
    --danger: #f06;
    --success: #3ddc84;
    --canvas-bg: #3a3a40;
    --transition: 120ms ease;
    --radius-sm: 4px;
    --radius-md: 6px;
    --radius-lg: 8px;
    --opacity-disabled: 0.35;
}

* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

html, body {
    width: 100%;
    height: 100%;
    overflow: hidden;
}

body {
    font-family: "Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, sans-serif;
    font-size: 12px;
    color: var(--ink-0);
    user-select: none;
    background: var(--bg-0);
}

button, input, select {
    font: inherit;
    color: inherit;
}

button {
    border: none;
    background: none;
    cursor: pointer;
}

input, select {
    outline: none;
}

option {
    background: var(--bg-2);
    color: var(--ink-0);
}

*::-webkit-scrollbar {
    width: 7px;
    height: 7px;
}
*::-webkit-scrollbar-track {
    background: transparent;
}
*::-webkit-scrollbar-thumb {
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.10);
}
*::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.18);
}

/* ------- App Shell ------- */
.app {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
}

/* ------- Top Bar (Menu + Actions) ------- */
.topbar {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 36px;
    padding: 0 6px 0 4px;
    background: var(--bg-1);
    border-bottom: 1px solid var(--panel-border);
    flex-shrink: 0;
    min-width: 0;
    overflow: hidden;
}

.topbar-brand {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px 0 6px;
    margin-right: 4px;
    height: 100%;
    border-right: 1px solid var(--panel-border);
    flex: 0 0 auto;
}

.brand-mark {
    width: 22px;
    height: 22px;
    border-radius: var(--radius-sm);
    background: var(--accent);
    display: grid;
    place-items: center;
    color: #fff;
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0;
}

.brand-title {
    font-size: 12px;
    font-weight: 700;
    color: var(--ink-0);
    letter-spacing: 0;
}

.topbar-actions {
    display: flex;
    align-items: center;
    gap: 1px;
    flex: 1 1 auto;
    justify-content: flex-end;
    min-width: 0;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: thin;
}

.menu-bar {
    display: flex;
    align-items: center;
    height: 100%;
    gap: 1px;
    flex: 0 1 auto;
    min-width: 0;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
}

.menu-bar::-webkit-scrollbar {
    display: none;
}

.menu-dropdown {
    position: relative;
    height: 100%;
    display: flex;
    align-items: center;
}

.menu-top-btn,
.tb-btn,
.menu-btn {
    border: 1px solid transparent;
}

.menu-top-btn {
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    color: var(--ink-1);
    font-size: 11px;
    white-space: nowrap;
    transition: background var(--transition), color var(--transition), border-color var(--transition);
}

.menu-top-btn:hover,
.menu-top-btn.active {
    background: rgba(255, 255, 255, 0.06);
    color: var(--ink-0);
}

.menu-dropdown-panel {
    position: absolute;
    top: 32px;
    left: 0;
    z-index: 20;
    min-width: 230px;
    max-height: min(72vh, 620px);
    overflow-y: auto;
    padding: 4px;
    background: #28282e;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: var(--radius-sm);
    box-shadow: 0 18px 34px rgba(0, 0, 0, 0.34);
}

.menu-item {
    width: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 24px;
    align-items: center;
    min-height: 26px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    color: var(--ink-1);
    text-align: left;
}

.menu-item:hover:not(:disabled) {
    background: var(--accent-soft);
    color: var(--ink-0);
}

.menu-item:disabled {
    opacity: 0.38;
    cursor: default;
}

.menu-item-label {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
}

.menu-item-shortcut {
    color: var(--ink-3);
    font-variant-numeric: tabular-nums;
}

.menu-sep {
    height: 1px;
    margin: 4px 6px;
    background: rgba(255, 255, 255, 0.08);
}

.tb-btn {
    height: 28px;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    color: var(--ink-1);
    font-size: 11px;
    font-weight: 500;
    flex: 0 0 auto;
    white-space: nowrap;
    transition: background var(--transition), color var(--transition), border-color var(--transition);
}

.tb-zoom-input {
    width: 64px;
    height: 28px;
    flex: 0 0 64px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(255, 255, 255, 0.09);
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-0);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    text-align: center;
}

.tb-zoom-input:focus {
    border-color: var(--accent-border);
    box-shadow: 0 0 0 2px rgba(77, 156, 246, 0.12);
}

.tb-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.06);
    color: var(--ink-0);
}
.tb-btn:disabled {
    opacity: var(--opacity-disabled);
    cursor: default;
}
.tb-btn.active {
    background: var(--accent-soft);
    border-color: var(--accent-border);
    color: var(--accent-hover);
}

.tb-sep {
    width: 1px;
    height: 18px;
    margin: 0 4px;
    background: var(--panel-border);
    flex-shrink: 0;
}

.topbar-spacer {
    flex: 0 1 8px;
    min-width: 8px;
}

.topbar-info {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 10px;
    color: var(--ink-2);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
}

.topbar-info span {
    white-space: nowrap;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
}

.doc-name {
    color: var(--ink-1);
    font-weight: 600;
    max-width: 180px;
}

/* ------- Workspace Grid ------- */
.workspace {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) 8px 300px;
    flex: 1;
    min-height: 0;
    position: relative;
    background: #202027;
    isolation: isolate;
}

/* ------- Left Tool Rail ------- */
.left-rail {
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
    border-right: 1px solid var(--panel-border);
    padding: 4px 3px;
    overflow-y: auto;
    overflow-x: hidden;
}

.toolbar-group {
    padding: 4px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
}
.toolbar-group:last-child {
    border-bottom: none;
}

.toolbar-group-label {
    display: none; /* hidden in compact mode */
}

.tool-btn {
    display: grid;
    place-items: center;
    width: 36px;
    height: 30px;
    margin: 0 auto 1px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: transparent;
    color: var(--ink-1);
    font-size: 15px;
    cursor: pointer;
    transition: background var(--transition), color var(--transition), border-color var(--transition);
}
.tool-btn:hover {
    background: rgba(255, 255, 255, 0.06);
    color: var(--ink-0);
}
.tool-btn.active {
    background: var(--accent-soft);
    border-color: var(--accent-border);
    color: var(--accent-hover);
}

.tool-btn-main {
    display: contents;
}

.tool-icon {
    font-size: 14px;
    line-height: 1;
}

.tool-svg-icon {
    width: 18px;
    height: 18px;
    display: block;
    pointer-events: none;
}

.tool-label,
.tool-shortcut,
.tool-meta {
    display: none; /* hidden in compact rail */
}

/* ------- Canvas Stage ------- */
.canvas-stage {
    display: grid;
    grid-template-rows: 36px minmax(0, 1fr) auto;
    min-width: 0;
    min-height: 0;
    background: #303036;
    overflow: hidden;
}

.tool-controls {
    display: flex;
    align-items: center;
    min-width: 0;
    gap: 8px;
    height: 36px;
    padding: 0 10px;
    background: #26262c;
    border-bottom: 1px solid var(--panel-border);
    overflow-x: auto;
    overflow-y: hidden;
}

.tc-group {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: max-content;
}

.tc-label,
.tc-section-label {
    color: var(--ink-2);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0;
}

.tc-input,
.tc-btn {
    height: 26px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(255, 255, 255, 0.09);
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-0);
    font-size: 11px;
}

.tc-input {
    padding: 0 7px;
}

.tc-num {
    width: 68px;
}

.tc-text {
    width: 150px;
}

.tc-range {
    width: 120px;
    accent-color: var(--accent);
}

.tc-btn {
    min-width: 28px;
    padding: 0 8px;
}

.tc-btn:hover,
.tc-btn.active {
    background: var(--accent-soft);
    border-color: var(--accent-border);
    color: var(--accent-hover);
}

.tc-separator {
    width: 1px;
    height: 18px;
    background: var(--panel-border);
}

.canvas-workbench {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) 14px;
    grid-template-rows: 22px minmax(0, 1fr) 14px;
    min-height: 0;
    overflow: hidden;
}

.ruler-corner {
    background: #26262c;
    border-right: 1px solid rgba(255, 255, 255, 0.10);
    border-bottom: 1px solid rgba(255, 255, 255, 0.10);
}

.canvas-ruler-x,
.canvas-ruler-y,
.canvas-scroll-x,
.canvas-scroll-y {
    min-width: 0;
    min-height: 0;
}

.canvas-scroll-x,
.canvas-scroll-y {
    display: flex;
    min-width: 0;
    min-height: 0;
}

.canvas-scroll-x .scrollbar-horizontal {
    width: 100% !important;
}

.canvas-scroll-y .scrollbar-vertical {
    height: 100% !important;
}

.canvas-viewport {
    position: relative;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    cursor: crosshair;
    background-image:
        linear-gradient(45deg, #35353b 25%, transparent 25%),
        linear-gradient(-45deg, #35353b 25%, transparent 25%),
        linear-gradient(45deg, transparent 75%, #35353b 75%),
        linear-gradient(-45deg, transparent 75%, #35353b 75%);
    background-size: 16px 16px;
    background-position: 0 0, 0 8px, 8px -8px, -8px 0px;
}

.canvas-document {
    position: absolute;
    background: #fff;
    box-shadow:
        0 0 0 1px rgba(0, 0, 0, 0.32),
        0 12px 34px rgba(0, 0, 0, 0.36);
    transform-origin: top left;
    overflow: visible;
    pointer-events: none;
}

.canvas-document img {
    display: block;
    width: 100%;
    height: 100%;
    background: white;
    pointer-events: none;
    image-rendering: auto;
}

.canvas-text-editor {
    position: absolute;
    z-index: 8;
    padding: 2px 4px;
    border: 1px solid rgba(77, 156, 246, 0.9);
    outline: none;
    background: rgba(255, 255, 255, 0.92);
    color: #111827;
    resize: both;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.45);
}

.canvas-drag-capture {
    position: absolute;
    inset: 0;
    z-index: 12;
    cursor: crosshair;
    background: transparent;
}

.canvas-drag-preview {
    position: absolute;
    z-index: 11;
    pointer-events: none;
    border: 1px solid rgba(77, 156, 246, 0.95);
    background: rgba(77, 156, 246, 0.16);
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.24) inset;
}

.canvas-drag-preview.ellipse {
    border-radius: 999px;
}

.canvas-shape-preview-svg {
    position: absolute;
    inset: 0;
    z-index: 11;
    pointer-events: none;
    overflow: visible;
}

.canvas-shape-preview-ellipse {
    fill: rgba(100, 149, 237, 0.86);
    stroke: rgba(0, 0, 0, 0.96);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
}

.canvas-drag-preview.text {
    border-style: dashed;
}

.canvas-drag-preview.mesh {
    border-style: dashed;
    background:
        linear-gradient(to right, rgba(77, 156, 246, 0.32) 1px, transparent 1px),
        linear-gradient(to bottom, rgba(77, 156, 246, 0.32) 1px, transparent 1px),
        rgba(77, 156, 246, 0.12);
    background-size: 33.333% 100%, 100% 33.333%, auto;
}

.canvas-drag-preview.page {
    border-style: dashed;
    background: rgba(245, 199, 90, 0.14);
    border-color: rgba(245, 199, 90, 0.9);
}

.canvas-drag-preview.star {
    clip-path: polygon(50% 0%, 62.361% 37.390%, 100% 38.197%, 70% 62.111%, 80.902% 100%, 50% 77.390%, 19.098% 100%, 30% 62.111%, 0% 38.197%, 37.639% 37.390%);
}

.canvas-path-preview-svg {
    position: absolute;
    inset: 0;
    z-index: 11;
    pointer-events: none;
    overflow: visible;
}

.canvas-path-preview-line {
    fill: none;
    stroke: rgba(77, 156, 246, 0.98);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
    filter: drop-shadow(0 0 2px rgba(0, 0, 0, 0.55));
}

.canvas-path-preview-line.calligraphy {
    stroke-width: 6;
}

.canvas-path-preview-line.measure {
    stroke-dasharray: 6 4;
}

.canvas-path-preview-line.gradient,
.canvas-path-preview-line.connector {
    stroke-dasharray: 8 5;
}

.canvas-path-preview-line.connector {
    stroke: rgba(245, 199, 90, 0.98);
}

.canvas-selection-overlay,
.canvas-rubber-band-preview {
    position: absolute;
    z-index: 10;
    pointer-events: none;
    box-sizing: border-box;
}

.canvas-selection-overlay {
    border: 2px solid rgba(77, 156, 246, 0.98);
    background: rgba(77, 156, 246, 0.045);
    box-shadow:
        0 0 0 1px rgba(255, 255, 255, 0.62),
        0 0 0 3px rgba(10, 18, 24, 0.34),
        0 0 12px rgba(77, 156, 246, 0.34);
}

.canvas-selection-overlay.dragging {
    border-style: dashed;
    background: rgba(77, 156, 246, 0.10);
}

.canvas-selection-handle {
    position: absolute;
    width: 9px;
    height: 9px;
    border-radius: 2px;
    border: 1px solid rgba(12, 18, 24, 0.95);
    background: #ffffff;
    box-shadow: 0 0 0 1px rgba(77, 156, 246, 0.98);
}

.canvas-selection-handle.nw {
    left: -6px;
    top: -6px;
}

.canvas-selection-handle.ne {
    right: -6px;
    top: -6px;
}

.canvas-selection-handle.sw {
    left: -6px;
    bottom: -6px;
}

.canvas-selection-handle.se {
    right: -6px;
    bottom: -6px;
}

.canvas-rubber-band-preview {
    border: 1px dashed rgba(77, 156, 246, 0.98);
    background: rgba(77, 156, 246, 0.12);
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.36) inset;
}

.canvas-tool-feedback {
    position: absolute;
    left: 50%;
    bottom: 44px;
    transform: translateX(-50%);
    z-index: 13;
    max-width: min(480px, calc(100% - 32px));
    min-height: 28px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0 12px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(245, 199, 90, 0.42);
    background: rgba(27, 27, 31, 0.92);
    color: var(--ink-1);
    box-shadow: 0 10px 26px rgba(0, 0, 0, 0.28);
    font-size: 12px;
    line-height: 1.3;
    text-align: center;
    pointer-events: none;
}

.canvas-page-label {
    position: absolute;
    left: 12px;
    bottom: 10px;
    display: inline-flex;
    align-items: center;
    height: 22px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    background: rgba(27, 27, 31, 0.82);
    border: 1px solid rgba(255, 255, 255, 0.09);
    color: var(--ink-1);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
}

.page-overlay {
    position: absolute;
    pointer-events: none;
    border: 1px solid rgba(77, 156, 246, 0.72);
    background: rgba(77, 156, 246, 0.045);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.18);
}

.page-overlay.active {
    border-color: rgba(132, 204, 22, 0.9);
    background: rgba(132, 204, 22, 0.055);
}

.page-overlay.page-tool-overlay {
    border-style: dashed;
    border-color: rgba(250, 204, 21, 0.92);
    background: rgba(250, 204, 21, 0.035);
}

.page-overlay.active.page-tool-overlay {
    border-color: rgba(132, 204, 22, 0.95);
    background: rgba(132, 204, 22, 0.075);
}

.page-overlay-label {
    position: absolute;
    left: 4px;
    top: 4px;
    display: inline-flex;
    align-items: center;
    height: 18px;
    max-width: 180px;
    padding: 0 6px;
    border-radius: var(--radius-sm);
    background: rgba(27, 27, 31, 0.88);
    color: var(--ink-1);
    font-size: 10px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.page-resize-handle {
    position: absolute;
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: #facc15;
    border: 1px solid rgba(27, 27, 31, 0.72);
    pointer-events: none;
}

.page-resize-handle.nw {
    left: -4px;
    top: -4px;
}

.page-resize-handle.ne {
    right: -4px;
    top: -4px;
}

.page-resize-handle.sw {
    left: -4px;
    bottom: -4px;
}

.page-resize-handle.se {
    right: -4px;
    bottom: -4px;
}

.guide-line {
    position: absolute;
    height: 1px;
    background: rgba(56, 189, 248, 0.78);
    transform-origin: 0 0;
    pointer-events: none;
    z-index: 4;
    box-shadow: 0 0 0 1px rgba(8, 47, 73, 0.18);
}

.snap-line {
    position: absolute;
    height: 1px;
    background: #facc15;
    transform-origin: 0 0;
    pointer-events: none;
    z-index: 5;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.18);
}

.snap-label {
    position: absolute;
    z-index: 6;
    pointer-events: none;
    height: 18px;
    padding: 0 6px;
    border-radius: var(--radius-sm);
    background: rgba(27, 27, 31, 0.88);
    color: #fef3c7;
    font-size: 10px;
    line-height: 18px;
}

.canvas-scroll-corner {
    background: #202025;
    border-left: 1px solid rgba(255, 255, 255, 0.08);
    border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.color-palette {
    min-width: 0;
    border-top: 1px solid var(--panel-border);
    background: #24242a;
}

/* ------- Inspector (Right Panel) ------- */
.inspector-resize-handle {
    position: relative;
    z-index: 2;
    min-width: 8px;
    cursor: col-resize;
    background: linear-gradient(180deg, #202027 0%, #24242c 100%);
    border-left: 1px solid rgba(255, 255, 255, 0.05);
    border-right: 1px solid rgba(0, 0, 0, 0.28);
}

.inspector-resize-handle::before {
    content: "";
    position: absolute;
    top: 50%;
    left: 50%;
    width: 2px;
    height: 40px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.16);
    transform: translate(-50%, -50%);
    box-shadow: 0 -8px 0 rgba(255, 255, 255, 0.08), 0 8px 0 rgba(255, 255, 255, 0.08);
}

.inspector-resize-handle:hover,
.inspector-resize-handle.active {
    background: rgba(77, 156, 246, 0.16);
}

.inspector-resize-handle:hover::before,
.inspector-resize-handle.active::before {
    background: rgba(104, 176, 255, 0.7);
    box-shadow: 0 -8px 0 rgba(104, 176, 255, 0.3), 0 8px 0 rgba(104, 176, 255, 0.3);
}

.inspector-resize-capture {
    position: absolute;
    inset: 0;
    z-index: 40;
    cursor: col-resize;
    user-select: none;
    background: transparent;
}

.resizing-inspector,
.resizing-inspector * {
    cursor: col-resize !important;
    user-select: none;
}

.inspector-shell {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    background: linear-gradient(180deg, #24242b 0%, #202027 100%);
    border-left: 1px solid rgba(255, 255, 255, 0.06);
    box-shadow: inset 1px 0 0 rgba(255, 255, 255, 0.03);
}

.right-panels {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
}

.right-panels > * {
    min-width: 0;
}

.inspector-tabs {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 1px;
    padding: 4px;
    background: #1f1f24;
    border-bottom: 1px solid var(--panel-border);
}

.inspector-tab {
    height: 28px;
    min-width: 0;
    border-radius: var(--radius-sm);
    color: var(--ink-2);
    font-size: 11px;
    font-weight: 600;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
}

.inspector-tab:hover,
.inspector-tab.active {
    background: var(--accent-soft);
    color: var(--accent-hover);
}

.properties-panel,
.layers-panel,
.doc-properties-panel,
.preferences-dialog,
.align-panel,
.transform-panel,
.pages-panel,
.objects-panel,
.text-panel,
.lpe-panel,
.filter-editor-panel,
.resources-panel {
    border-bottom: 1px solid var(--panel-border);
}

.panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    background: rgba(255, 255, 255, 0.02);
    min-width: 0;
}

.panel-header > * {
    min-width: 0;
}

.panel-title-wrap {
    display: flex;
    flex-direction: column;
    gap: 0;
}

.panel-kicker {
    display: none; /* removed decorative kicker */
}

.panel-title {
    font-size: 11px;
    font-weight: 700;
    color: var(--ink-0);
    letter-spacing: 0;
}

.panel-meta {
    display: none; /* removed decorative description */
}

.panel-badge {
    display: inline-flex;
    align-items: center;
    height: 20px;
    padding: 0 7px;
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.05);
    border: none;
    color: var(--ink-2);
    font-size: 10px;
    font-weight: 600;
}

.panel-action-btn {
    height: 24px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-1);
    font-size: 11px;
}

.panel-action-btn:hover {
    background: var(--accent-soft);
    border-color: var(--accent-border);
    color: var(--accent-hover);
}

.panel-swatch-pair {
    display: flex;
    gap: 4px;
    align-items: center;
}

.panel-swatch {
    width: 14px;
    height: 14px;
    border-radius: 3px;
    border: 1px solid rgba(255, 255, 255, 0.14);
}

/* ------- Property Sections ------- */
.prop-section {
    padding: 10px 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
}
.prop-section:last-child {
    border-bottom: none;
}

.prop-section-header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
}

.prop-section-header {
    font-size: 11px;
    font-weight: 700;
    color: var(--ink-0);
}

.paint-type-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    margin-bottom: 8px;
}

.paint-tab {
    display: inline-flex;
    align-items: center;
    height: 24px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-1);
    font-size: 11px;
    cursor: pointer;
    min-width: 0;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: all var(--transition);
}
.paint-tab:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--ink-0);
}
.paint-tab.active {
    background: var(--accent-soft);
    border-color: var(--accent-border);
    color: var(--accent-hover);
}

.prop-row,
.layer-controls {
    display: grid;
    align-items: center;
    gap: 8px;
}

.prop-row {
    grid-template-columns: minmax(46px, 60px) minmax(0, 1fr) auto;
    margin-bottom: 6px;
}

.prop-row > * {
    min-width: 0;
}
.prop-row.stacked {
    grid-template-columns: 1fr;
    align-items: stretch;
}
.prop-row:last-child,
.prop-color-row:last-child {
    margin-bottom: 0;
}

.prop-color-row {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr);
    gap: 6px;
    margin-bottom: 6px;
}

.prop-label {
    color: var(--ink-2);
    font-size: 11px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.prop-value {
    min-width: 0;
    color: var(--ink-0);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.prop-color,
.prop-input,
.prop-hex-input,
.prop-select,
.prop-textarea {
    height: 28px;
    width: 100%;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-0);
    font-size: 11px;
}

input[type="number"].prop-input {
    color-scheme: dark;
    padding-right: 22px;
}

input[type="number"].prop-input::-webkit-inner-spin-button,
input[type="number"].prop-input::-webkit-outer-spin-button {
    width: 16px;
    min-height: 24px;
    margin: 0 -4px 0 4px;
    opacity: 0.62;
    filter: invert(1) brightness(0.7) saturate(0.55);
}

input[type="number"].prop-input:hover::-webkit-inner-spin-button,
input[type="number"].prop-input:focus::-webkit-inner-spin-button {
    opacity: 0.9;
    filter: invert(1) brightness(0.95) saturate(0.75);
}

.prop-textarea {
    height: auto;
    min-height: 62px;
    padding: 7px 8px;
    resize: vertical;
}

.prop-unit {
    color: var(--ink-2);
    font-size: 10px;
}

.prop-checkbox {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
}

.toggle-btn,
.dialog-close-btn,
.pref-section-btn,
.align-btn,
.transform-toggle,
.object-btn {
    height: 26px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-1);
    font-size: 11px;
}

.toggle-btn:hover,
.toggle-btn.active,
.pref-section-btn:hover,
.pref-section-btn.active,
.align-btn:hover,
.transform-toggle:hover,
.transform-toggle.active,
.object-btn:hover {
    background: var(--accent-soft);
    border-color: var(--accent-border);
    color: var(--accent-hover);
}

.align-section {
    padding: 10px 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
}

.align-section-header {
    margin-bottom: 8px;
    font-size: 11px;
    font-weight: 700;
    color: var(--ink-0);
}

.align-buttons {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
}

.align-btn,
.transform-toggle {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

/* ------- Dense Inspector Panels ------- */
.text-align-buttons,
.lpe-add-buttons,
.filter-add-buttons {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
    margin-top: 8px;
}

.text-align-btn,
.action-btn,
.lpe-btn,
.filter-btn,
.resource-btn {
    height: 26px;
    min-width: 0;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-1);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.text-align-btn:hover,
.text-align-btn.active,
.action-btn:hover:not(:disabled),
.lpe-btn:hover:not(:disabled),
.filter-btn:hover:not(:disabled),
.resource-btn:hover:not(:disabled) {
    background: var(--accent-soft);
    border-color: var(--accent-border);
    color: var(--accent-hover);
}

.action-btn:disabled,
.lpe-btn:disabled,
.filter-btn:disabled,
.resource-btn:disabled {
    opacity: var(--opacity-disabled);
    cursor: default;
}

.text-content-input {
    min-height: 88px;
}

.lpe-list,
.filter-chain-list,
.resources-list {
    display: grid;
    gap: 6px;
    margin-top: 8px;
}

.lpe-item,
.filter-prim-item {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 34px 32px 32px 28px;
    gap: 4px;
    align-items: center;
    min-width: 0;
    min-height: 32px;
    padding: 4px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: rgba(255, 255, 255, 0.025);
}

.lpe-item:hover,
.filter-prim-item:hover,
.resource-item:hover {
    background: rgba(255, 255, 255, 0.05);
}

.lpe-item.selected,
.filter-prim-item.selected,
.resource-item.selected {
    background: var(--accent-soft);
    border-color: var(--accent-border);
}

.lpe-label,
.filter-prim-label {
    min-width: 0;
    color: var(--ink-0);
    font-size: 11px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.lpe-label.disabled,
.filter-prim-label.disabled {
    opacity: 0.45;
}

.lpe-btn,
.filter-btn,
.resource-btn {
    width: 100%;
    padding: 0 4px;
}

.lpe-btn.delete:hover,
.filter-btn.delete:hover,
.resource-btn.delete:hover {
    border-color: rgba(244, 114, 103, 0.42);
    color: #fca5a5;
}

.lpe-list-empty,
.filter-chain-empty,
.resources-empty {
    padding: 12px 8px;
    border-radius: var(--radius-sm);
    color: var(--ink-2);
    font-size: 11px;
    background: rgba(255, 255, 255, 0.025);
}

.resource-item {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-rows: auto auto auto;
    gap: 4px 8px;
    min-width: 0;
    padding: 7px 8px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: rgba(255, 255, 255, 0.025);
    cursor: pointer;
}

.resource-kind-badge {
    grid-column: 1;
    grid-row: 1;
    justify-self: start;
    max-width: 100%;
    height: 18px;
    padding: 0 6px;
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.06);
    color: var(--ink-2);
    font-size: 10px;
    line-height: 18px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.resource-name,
.resource-id {
    grid-column: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.resource-name {
    grid-row: 2;
    color: var(--ink-0);
    font-size: 11px;
    font-weight: 600;
}

.resource-id {
    grid-row: 3;
    color: var(--ink-2);
    font-size: 10px;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
}

.resource-usage {
    grid-column: 2;
    grid-row: 1;
    color: var(--ink-2);
    font-size: 10px;
    white-space: nowrap;
}

.resource-usage.unused {
    color: #fca5a5;
}

.resource-btn {
    grid-column: 2;
    grid-row: 2 / span 2;
    align-self: stretch;
    width: 28px;
}

.dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    padding: 10px 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.04);
}

.objects-tree {
    padding: 4px;
}

.object-root-drop {
    margin: 6px 8px 4px;
    padding: 6px 8px;
    border: 1px dashed rgba(255, 255, 255, 0.12);
    border-radius: var(--radius-sm);
    color: var(--ink-2);
    font-size: 10px;
}

.object-root-drop:hover {
    border-color: var(--accent-border);
    color: var(--accent-hover);
}

.object-row {
    display: grid;
    grid-template-columns: 34px 26px 26px minmax(0, 1fr);
    align-items: center;
    gap: 4px;
    min-height: 28px;
    padding: 2px 4px;
    border-radius: var(--radius-sm);
    color: var(--ink-1);
}

.object-row:hover {
    background: rgba(255, 255, 255, 0.04);
}

.object-row.selected {
    background: var(--accent-soft);
    color: var(--accent-hover);
}

.object-type-icon {
    color: var(--ink-2);
    font-size: 10px;
    font-weight: 700;
}

.object-btn {
    width: 26px;
    padding: 0;
}

.object-label,
.object-rename-input,
.layer-rename-input {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
}

.object-label {
    cursor: pointer;
}

.object-rename-input,
.layer-rename-input {
    height: 24px;
    padding: 0 6px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--accent-border);
    background: rgba(255, 255, 255, 0.05);
    color: var(--ink-0);
}

.object-empty {
    padding: 16px 12px;
    color: var(--ink-2);
    font-size: 11px;
}

.page-list {
    padding: 6px;
}

.page-row {
    display: grid;
    gap: 8px;
    padding: 8px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: rgba(255, 255, 255, 0.025);
    margin-bottom: 6px;
}

.page-row.active {
    border-color: var(--accent-border);
    background: var(--accent-soft);
}

.page-row-head {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) 54px;
    gap: 6px;
    align-items: center;
}

.page-nav-btn,
.page-delete-btn,
.page-label-input {
    height: 26px;
    min-width: 0;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-1);
    font-size: 11px;
}

.page-label-input {
    width: 100%;
    padding: 0 8px;
    color: var(--ink-0);
}

.page-nav-btn,
.page-delete-btn {
    padding: 0 6px;
}

.page-nav-btn:hover,
.page-delete-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--ink-0);
}

.page-delete-btn:hover {
    border-color: rgba(244, 114, 103, 0.42);
    color: #fca5a5;
}

.page-geometry-grid {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) 18px minmax(0, 1fr);
    gap: 6px;
    align-items: center;
}

.preferences-layout {
    display: grid;
    grid-template-columns: 86px minmax(0, 1fr);
    min-height: 0;
}

.preferences-sidebar {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 8px 6px;
    border-right: 1px solid rgba(255, 255, 255, 0.06);
}

.preferences-content {
    min-width: 0;
}
.prop-color {
    padding: 0;
    overflow: hidden;
    cursor: pointer;
}
.prop-input:focus,
.prop-hex-input:focus,
.prop-select:focus {
    border-color: var(--accent-border);
    box-shadow: 0 0 0 2px rgba(77, 156, 246, 0.1);
}

.prop-slider,
.layer-opacity-slider {
    width: 100%;
    accent-color: var(--accent);
    height: 4px;
}

.prop-value,
.layer-opacity-value {
    min-width: 34px;
    text-align: right;
    color: var(--ink-2);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
}

.prop-empty,
.layer-empty {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 16px 12px;
    color: var(--ink-2);
    font-size: 11px;
    line-height: 1.5;
}
.empty-title {
    color: var(--ink-1);
    font-weight: 600;
    font-size: 11px;
}

/* ------- Layer Controls ------- */
.layer-controls {
    grid-column: 1 / -1;
    grid-template-columns: 48px minmax(0, 1fr) auto;
}

.layer-control-label {
    color: var(--ink-2);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0;
}

/* ------- Layers ------- */
.layers-list {
    padding: 6px;
}

.layer-item {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    margin-bottom: 4px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: rgba(255, 255, 255, 0.025);
    transition: background var(--transition), border-color var(--transition);
    min-width: 0;
}
.layer-item:last-child {
    margin-bottom: 0;
}
.layer-item:hover {
    background: rgba(255, 255, 255, 0.05);
}
.layer-item.selected {
    background: var(--accent-soft);
    border-color: var(--accent-border);
}
.layer-item.active {
    background: rgba(132, 204, 22, 0.12);
    border-color: rgba(132, 204, 22, 0.42);
}

.layer-main-row {
    display: grid;
    grid-template-columns: 26px 26px minmax(0, 1fr) 22px 22px;
    align-items: center;
    gap: 4px;
    min-width: 0;
}

.layer-btn {
    width: 26px;
    height: 26px;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    color: var(--ink-2);
    font-size: 12px;
    cursor: pointer;
    display: grid;
    place-items: center;
    transition: background var(--transition), color var(--transition);
}

.layer-icon {
    width: 15px;
    height: 15px;
    display: block;
    pointer-events: none;
}

.layer-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    color: var(--ink-0);
}
.layer-btn:disabled {
    opacity: 0.3;
    cursor: default;
}

.layer-label-wrap {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 1px;
    min-width: 0;
    cursor: pointer;
}

.layer-opacity-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 38px auto;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding-left: 60px;
}

.layer-blend-select {
    height: 22px;
    max-width: 96px;
    padding: 0 4px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-1);
    font-size: 10px;
}
.layer-blend-select:focus {
    border-color: var(--accent-border);
}

.layer-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--ink-0);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.layer-meta {
    font-size: 10px;
    color: var(--ink-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.layer-blend-meta {
    color: var(--ink-1);
}

/* ------- Status Bar ------- */
.status-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    height: 26px;
    padding: 0 10px;
    background: var(--bg-1);
    border-top: 1px solid var(--panel-border);
    flex-shrink: 0;
    min-width: 0;
    overflow: hidden;
}

.status-section {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    overflow: hidden;
}

.status-cluster {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 0 8px;
    height: 20px;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    min-width: 0;
}
.status-cluster.emphasis {
    background: var(--accent-soft);
}

.status-label {
    color: var(--ink-2);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0;
}

.status-item {
    color: var(--ink-1);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

/* ------- Responsive ------- */
@media (max-width: 1200px) {
    .workspace {
        grid-template-columns: 44px minmax(0, 1fr) 8px 252px;
    }

    .topbar-info {
        display: none;
    }
}

@media (max-width: 1120px) {
    .brand-title {
        display: none;
    }

    .topbar-brand {
        padding-right: 6px;
        margin-right: 2px;
    }

    .menu-top-btn,
    .tb-btn {
        padding-left: 6px;
        padding-right: 6px;
    }

    .tool-controls {
        gap: 6px;
        padding-left: 8px;
        padding-right: 8px;
    }
}
"#;

#[cfg(test)]
mod shortcut_tests {
    use super::*;

    /// Every shortcut in the default binding table has to reach something.
    ///
    /// Before the bindings were wired up, the menu displayed shortcut text
    /// that no key press ever triggered.
    #[test]
    fn every_default_shortcut_resolves_to_a_wired_action() {
        let manager = sivgo_core::ShortcutManager::new();
        let mut unresolved = Vec::new();

        for (shortcut, action) in manager.bindings() {
            // Tool switching is handled directly, not through the menu.
            if matches!(action, sivgo_core::Action::SwitchTool(_)) {
                continue;
            }
            match menu_action_for_shortcut(action) {
                Some(menu_action) => assert!(
                    sivgo_ui::menu_model::is_action_wired(menu_action),
                    "{shortcut:?} maps to {menu_action}, which has no handler"
                ),
                None => unresolved.push(format!("{action:?}")),
            }
        }

        // A binding with no menu equivalent is acceptable, but the list should
        // stay short and deliberate rather than growing silently.
        assert!(
            unresolved.len() <= 12,
            "too many shortcuts lead nowhere: {unresolved:?}"
        );
    }

    #[test]
    fn common_editing_shortcuts_are_bound() {
        use sivgo_core::{Key as EditorKey, Modifiers};
        let manager = sivgo_core::ShortcutManager::new();
        let ctrl = Modifiers {
            ctrl: true,
            shift: false,
            alt: false,
        };

        for (key, expected) in [
            (EditorKey::S, "file.save"),
            (EditorKey::Z, "edit.undo"),
            (EditorKey::C, "edit.copy"),
            (EditorKey::V, "edit.paste"),
            (EditorKey::A, "edit.select_all"),
            (EditorKey::G, "object.group"),
        ] {
            let action = manager
                .lookup(key, ctrl)
                .unwrap_or_else(|| panic!("{key:?} should be bound"));
            assert_eq!(
                menu_action_for_shortcut(action),
                Some(expected),
                "{key:?} should trigger {expected}"
            );
        }
    }

    #[test]
    fn menu_shortcut_hints_parse_into_accelerators() {
        // Anything the menu shows as a shortcut must be registrable, or the
        // label promises a key that does nothing.
        let mut unparsed = Vec::new();
        for node in sivgo_ui::menu_model::default_menu_bar() {
            collect_unparsed_shortcuts(&node, &mut unparsed);
        }
        assert!(
            unparsed.is_empty(),
            "menu shortcut hints that no accelerator matches: {unparsed:?}"
        );
    }

    fn collect_unparsed_shortcuts(node: &MenuNode, out: &mut Vec<String>) {
        if let Some(hint) = node.shortcut.as_deref() {
            if !hint.is_empty() && parse_accelerator(hint).is_none() {
                out.push(hint.to_string());
            }
        }
        for child in &node.children {
            collect_unparsed_shortcuts(child, out);
        }
    }

    #[test]
    fn key_translation_covers_letters_and_named_keys() {
        assert_eq!(
            to_editor_key(&Key::Character("a".into())),
            Some(sivgo_core::Key::A)
        );
        // Case is normalised so Shift+S and s reach the same binding.
        assert_eq!(
            to_editor_key(&Key::Character("S".into())),
            Some(sivgo_core::Key::S)
        );
        assert_eq!(
            to_editor_key(&Key::Character("3".into())),
            Some(sivgo_core::Key::Digit3)
        );
        // Punctuation has no named variant and falls back to `Char`.
        assert_eq!(
            to_editor_key(&Key::Character("+".into())),
            Some(sivgo_core::Key::Plus)
        );
        assert_eq!(
            to_editor_key(&Key::Named(keyboard_types::NamedKey::Delete)),
            Some(sivgo_core::Key::Delete)
        );
        assert_eq!(
            to_editor_key(&Key::Named(keyboard_types::NamedKey::Escape)),
            Some(sivgo_core::Key::Escape)
        );
        // Multi-character input is not a single key press.
        assert_eq!(to_editor_key(&Key::Character("ab".into())), None);
    }
}

#[cfg(test)]
mod frontend_layout_tests {
    use super::*;

    #[test]
    fn topbar_and_status_css_are_overflow_hardened() {
        assert!(CSS.contains(".topbar-actions"));
        assert!(CSS.contains("overflow-x: auto"));
        assert!(CSS.contains(".status-item"));
        assert!(CSS.contains("text-overflow: ellipsis"));
        assert!(CSS.contains("@media (max-width: 1120px)"));
    }

    #[test]
    fn inspector_split_layout_has_resizer_and_dark_number_spinners() {
        assert!(CSS.contains("grid-template-columns: 44px minmax(0, 1fr) 8px 300px"));
        assert!(CSS.contains(".inspector-resize-handle"));
        assert!(CSS.contains(".inspector-resize-capture"));
        assert!(CSS.contains("cursor: col-resize"));
        assert!(CSS.contains("input[type=\"number\"].prop-input"));
        assert!(CSS.contains("color-scheme: dark"));
        assert!(CSS.contains("::-webkit-inner-spin-button"));
    }

    #[test]
    fn topbar_exposes_editable_zoom_control() {
        assert!(CSS.contains(".tb-zoom-input"));
        assert_eq!(parse_zoom_percent("125%"), Some(1.25));
        assert_eq!(parse_zoom_percent(" 80 "), Some(0.8));
        assert_eq!(parse_zoom_percent("bad"), None);
        assert_eq!(parse_zoom_percent("0%"), None);
        assert_eq!(parse_zoom_percent("-50%"), None);
        assert_eq!(parse_zoom_percent("NaN"), None);
        assert_eq!(parse_zoom_percent("inf"), None);
        assert_eq!(parse_zoom_percent("99999%"), Some(ZOOM_MAX));
    }

    #[test]
    fn app_css_does_not_force_uppercase_or_letter_spacing() {
        assert!(!CSS.contains("text-transform"));
        assert!(!CSS.contains("uppercase"));
        for line in CSS.lines().filter(|line| line.contains("letter-spacing")) {
            assert!(
                line.contains("letter-spacing: 0;"),
                "non-zero letter spacing is not allowed: {line}"
            );
        }
    }

    #[test]
    fn layers_panel_styles_rows_and_blend_select() {
        assert!(CSS.contains(".layer-main-row"));
        assert!(CSS.contains(".layer-opacity-row"));
        assert!(CSS.contains(".layer-icon"));
        assert!(CSS.contains(".layer-blend-select"));
    }

    #[test]
    fn double_click_path_enters_node_edit_mode() {
        let mut engine = EditorEngine::new(240.0, 180.0);
        let mut path = sivgo_math::BezPath::new();
        path.move_to(Point::new(20.0, 20.0));
        path.line_to(Point::new(100.0, 20.0));
        path.line_to(Point::new(100.0, 90.0));
        path.close();

        let root = engine.document.root().expect("new document has a root");
        let path_id = engine
            .document
            .add_node(SvgElement::Path(sivgo_dom::PathAttrs {
                d: path,
                ..Default::default()
            }));
        engine.document.append_child(root, path_id);

        assert_eq!(
            enter_canvas_edit_mode_at(&mut engine, 60.0, 50.0),
            Some(ToolId::Node)
        );
        assert_eq!(engine.selection.primary(), Some(path_id));
        assert_eq!(engine.tools.active_id(), ToolId::Node);
    }

    #[test]
    fn wheel_delta_normalizes_to_zoom_steps() {
        assert_eq!(
            wheel_delta_to_zoom_steps(WheelDelta::pixels(0.0, -120.0, 0.0)),
            1.0
        );
        assert_eq!(
            wheel_delta_to_zoom_steps(WheelDelta::pixels(0.0, 120.0, 0.0)),
            -1.0
        );
        assert_eq!(
            wheel_delta_to_zoom_steps(WheelDelta::lines(0.0, -3.0, 0.0)),
            1.0
        );
        assert_eq!(
            wheel_delta_to_zoom_steps(WheelDelta::pages(0.0, -1.0, 0.0)),
            1.0
        );
    }

    #[test]
    fn setting_zoom_preserves_viewport_center_document_point() {
        let mut engine = EditorEngine::new(800.0, 600.0);
        engine.viewport.pan = sivgo_math::Vector::new(-120.0, -80.0);
        let center = sivgo_math::Point::new(400.0, 300.0);
        let before = engine.viewport.screen_to_document(center);
        set_viewport_zoom_at_screen_point(&mut engine, 2.0, center);
        let after = engine.viewport.screen_to_document(center);
        assert!((before.x - after.x).abs() < 0.001);
        assert!((before.y - after.y).abs() < 0.001);
        assert_eq!(engine.viewport.zoom, 2.0);
    }

    #[test]
    fn inspector_layout_settings_are_clamped_and_drag_math_is_directional() {
        assert_eq!(sanitize_inspector_width(f64::NAN), INSPECTOR_WIDTH_DEFAULT);
        assert_eq!(sanitize_inspector_width(10.0), INSPECTOR_WIDTH_MIN);
        assert_eq!(sanitize_inspector_width(10_000.0), INSPECTOR_WIDTH_MAX);

        let drag = InspectorResizeDrag {
            start_client_x: 900.0,
            start_width_px: 300.0,
        };
        assert_eq!(drag.width_at(860.0), 340.0);
        assert_eq!(drag.width_at(950.0), 250.0);
    }

    #[test]
    fn canvas_viewport_width_tracks_resized_inspector_space() {
        let default_width = canvas_viewport_width_for_window(1280.0, INSPECTOR_WIDTH_DEFAULT);
        assert_eq!(
            default_width,
            1280.0
                - LEFT_RAIL_WIDTH
                - INSPECTOR_RESIZE_HANDLE_WIDTH
                - INSPECTOR_WIDTH_DEFAULT
                - CANVAS_RULER_THICKNESS
                - CANVAS_SCROLLBAR_THICKNESS
        );

        let wide_inspector = canvas_viewport_width_for_window(1280.0, INSPECTOR_WIDTH_MAX);
        assert!(wide_inspector < default_width);
        assert_eq!(
            canvas_viewport_width_for_window(100.0, INSPECTOR_WIDTH_MAX),
            1.0
        );
    }

    #[test]
    fn desktop_settings_snapshot_preserves_persisted_layout_width() {
        let layout = LayoutSettings {
            inspector_width_px: 444.0,
        };
        let snapshot = desktop_settings_snapshot(
            &PreferencesState::default(),
            &DocumentDefaults::default(),
            &layout,
        );
        assert_eq!(snapshot.layout.inspector_width_px, 444.0);
        assert_eq!(snapshot.document_defaults.width_px, CANVAS_WIDTH as f64);
    }

    #[test]
    fn advanced_inspector_panels_have_dense_layout_css() {
        for selector in [
            ".text-align-buttons",
            ".lpe-add-buttons",
            ".filter-add-buttons",
            ".lpe-item",
            ".filter-prim-item",
            ".resource-item",
            ".resource-id",
        ] {
            assert!(CSS.contains(selector), "missing selector {selector}");
        }
    }

    #[test]
    fn canvas_scrollbar_slots_force_stretching() {
        assert!(CSS.contains(".canvas-scroll-x .scrollbar-horizontal"));
        assert!(CSS.contains("width: 100% !important"));
        assert!(CSS.contains(".canvas-scroll-y .scrollbar-vertical"));
        assert!(CSS.contains("height: 100% !important"));
    }

    #[test]
    fn semantic_menu_is_rendered_in_topbar() {
        assert!(in_window_menu_rendered());
        assert!(CSS.contains(".tool-svg-icon"));
        assert!(CSS.contains("pointer-events: none"));
    }

    #[test]
    fn tool_selection_routes_expected_inspector_panels() {
        assert_eq!(
            inspector_panel_for_tool(ToolId::Text),
            Some(InspectorPanel::Text)
        );
        assert_eq!(
            inspector_panel_for_tool(ToolId::Pages),
            Some(InspectorPanel::Pages)
        );
        assert_eq!(
            inspector_panel_for_tool(ToolId::Lpe),
            Some(InspectorPanel::Lpe)
        );
        assert_eq!(inspector_panel_for_tool(ToolId::Rect), None);
    }

    #[test]
    fn viewport_mouse_event_preserves_canvas_screen_coordinates() {
        let event = engine_mouse_event_from_viewport_coords(123.0, 45.0, true, false, true);
        assert_eq!(event.x, 123.0);
        assert_eq!(event.y, 45.0);
        assert!(event.modifiers.ctrl);
        assert!(!event.modifiers.shift);
        assert!(event.modifiers.alt);
    }

    #[test]
    fn cursor_position_updates_are_quantized_by_screen_delta() {
        assert!(!should_update_cursor_position(10.0, 20.0, 17.0, 20.0, 1.0));
        assert!(should_update_cursor_position(10.0, 20.0, 18.0, 20.0, 1.0));
        assert!(should_update_cursor_position(10.0, 20.0, 14.1, 20.0, 2.0));
    }

    #[test]
    fn canvas_mousemove_does_not_force_bitmap_render() {
        assert!(!should_invalidate_canvas_bitmap_for_mouse_move_tool(
            ToolId::Pencil
        ));
        assert!(should_invalidate_canvas_bitmap_for_mouse_move_tool(
            ToolId::Spray
        ));
    }

    #[test]
    fn canvas_cursor_tracks_active_tool() {
        assert_eq!(canvas_cursor_for_tool(ToolId::Select), "default");
        assert_eq!(canvas_cursor_for_tool(ToolId::Pages), "default");
        assert_eq!(canvas_cursor_for_tool(ToolId::Text), "text");
        assert_eq!(canvas_cursor_for_tool(ToolId::Zoom), "zoom-in");
        assert_eq!(canvas_cursor_for_tool(ToolId::Rect), "crosshair");
    }

    #[test]
    fn selection_overlay_css_and_preview_math_are_stable() {
        assert!(CSS.contains(".canvas-selection-overlay"));
        assert!(CSS.contains(".canvas-selection-handle"));
        assert!(CSS.contains(".canvas-rubber-band-preview"));

        let preview =
            CanvasSelectPreview::new(100.0, 120.0, Some(Rect::new(10.0, 20.0, 40.0, 60.0)))
                .with_current(116.0, 150.0);
        assert!(preview.is_active());
        assert_eq!(preview.screen_delta(), (16.0, 30.0));
        assert_eq!(
            canvas_selection_overlay_style(
                preview.selection_bounds.unwrap(),
                2.0,
                5.0,
                7.0,
                16.0,
                30.0
            ),
            "left: 41px; top: 77px; width: 60px; height: 80px;"
        );

        let rubber_band = CanvasSelectPreview::new(40.0, 50.0, None).with_current(10.0, 20.0);
        assert_eq!(
            canvas_rubber_band_preview_style(rubber_band),
            "left: 10px; top: 20px; width: 30px; height: 30px;"
        );
    }

    #[test]
    fn hover_mousemove_does_not_dispatch_to_engine() {
        assert!(!should_dispatch_canvas_mouse_move(false, ToolId::Select));
        assert!(!should_dispatch_canvas_mouse_move(true, ToolId::Rect));
        assert!(should_dispatch_canvas_mouse_move(true, ToolId::Select));
        assert!(should_capture_canvas_mouse_move(true, ToolId::Rect));
        assert!(!should_capture_canvas_mouse_move(false, ToolId::Rect));
        assert!(tool_needs_continuous_canvas_mouse_move(ToolId::Pencil));
        assert!(tool_needs_continuous_canvas_mouse_move(ToolId::Pen));
        assert!(!tool_needs_continuous_canvas_mouse_move(ToolId::Ellipse));
        assert!(tool_uses_canvas_drag_preview(ToolId::Ellipse));
        assert!(tool_uses_canvas_drag_preview(ToolId::Star));
        assert!(tool_uses_canvas_drag_preview(ToolId::Zoom));
        assert!(tool_uses_canvas_drag_preview(ToolId::Text));
        assert!(tool_uses_canvas_path_preview(ToolId::Pen));
        assert!(tool_uses_canvas_path_preview(ToolId::Pencil));
        assert!(tool_uses_canvas_path_preview(ToolId::Calligraphy));
        assert!(tool_uses_canvas_path_preview(ToolId::Measure));
        assert!(tool_uses_canvas_path_preview(ToolId::Gradient));
        assert!(tool_uses_canvas_path_preview(ToolId::Connector));
        assert!(tool_uses_canvas_drag_preview(ToolId::Mesh));
        assert!(tool_uses_canvas_drag_preview(ToolId::Pages));
        assert!(should_capture_canvas_mouse_move(true, ToolId::Pages));
        assert!(should_invalidate_canvas_bitmap_for_mouse_move_tool(
            ToolId::Spray
        ));
        assert!(should_invalidate_canvas_bitmap_for_mouse_move_tool(
            ToolId::Tweak
        ));
        assert!(!should_invalidate_canvas_bitmap_for_mouse_move_tool(
            ToolId::Pencil
        ));
    }

    #[test]
    fn star_drag_preview_uses_fitted_star_polygon() {
        assert!(CSS.contains("62.361% 37.390%"));
        assert!(CSS.contains("80.902% 100%"));
        assert!(CSS.contains("0% 38.197%"));
    }

    #[test]
    fn screenshot_tool_group_has_preview_mutation_or_target_feedback() {
        let screenshot_tools = [
            ToolId::Text,
            ToolId::Gradient,
            ToolId::Mesh,
            ToolId::Dropper,
            ToolId::BucketFill,
            ToolId::Connector,
            ToolId::Marker,
            ToolId::Pages,
            ToolId::Lpe,
            ToolId::Eraser,
            ToolId::Spray,
            ToolId::Tweak,
            ToolId::Measure,
            ToolId::Zoom,
        ];

        for tool in screenshot_tools {
            let has_feedback_strategy = tool_uses_canvas_drag_preview(tool)
                || tool_uses_canvas_path_preview(tool)
                || tool_commits_document_on_mouse_move(tool)
                || canvas_tool_feedback_on_mouse_down(tool, None, false, None, None, false, false)
                    .is_some();
            assert!(
                has_feedback_strategy,
                "missing visible feedback strategy for {tool:?}"
            );
        }

        assert!(CSS.contains(".canvas-tool-feedback"));
        assert!(CSS.contains(".canvas-drag-preview.mesh"));
        assert!(CSS.contains(".canvas-path-preview-line.connector"));
    }

    #[test]
    fn conditional_tools_report_no_target_feedback() {
        assert_eq!(
            canvas_tool_feedback_on_mouse_down(
                ToolId::Gradient,
                None,
                false,
                None,
                None,
                false,
                false
            ),
            Some("Select an object first")
        );
        assert_eq!(
            canvas_tool_feedback_on_mouse_down(
                ToolId::BucketFill,
                None,
                false,
                None,
                None,
                false,
                false
            ),
            Some("Click an object to fill")
        );
        assert_eq!(
            canvas_tool_feedback_on_mouse_down(
                ToolId::Marker,
                None,
                false,
                None,
                None,
                false,
                false
            ),
            Some("Select a path first")
        );
        assert_eq!(
            canvas_tool_feedback_on_mouse_down(ToolId::Lpe, None, false, None, None, true, false),
            Some("Create or select a path first")
        );
        assert_eq!(
            canvas_tool_feedback_on_mouse_up(ToolId::Connector, None, None),
            Some("Start on an object")
        );
    }

    #[test]
    fn pages_preview_skips_existing_page_borders() {
        let page = PageEntry {
            index: 1,
            id: NodeId::new(),
            label: "Page 1".to_string(),
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 80.0,
            active: true,
        };
        assert!(!pages_tool_should_preview_new_page(
            std::slice::from_ref(&page),
            10.0,
            40.0,
            0.0,
            0.0,
            1.0
        ));
        assert!(pages_tool_should_preview_new_page(
            &[page],
            40.0,
            50.0,
            0.0,
            0.0,
            1.0
        ));
    }

    #[test]
    fn tool_feedback_audit_classifies_every_tool() {
        let audit = tool_feedback_audit_covers_all_tools();
        let tools = [
            ToolId::Select,
            ToolId::Node,
            ToolId::Rect,
            ToolId::Ellipse,
            ToolId::Star,
            ToolId::Spiral,
            ToolId::Pen,
            ToolId::Pencil,
            ToolId::Calligraphy,
            ToolId::Text,
            ToolId::Gradient,
            ToolId::Dropper,
            ToolId::BucketFill,
            ToolId::Connector,
            ToolId::Eraser,
            ToolId::Spray,
            ToolId::Tweak,
            ToolId::Measure,
            ToolId::Zoom,
            ToolId::Mesh,
            ToolId::Box3d,
            ToolId::Marker,
            ToolId::Pages,
            ToolId::Lpe,
        ];
        assert_eq!(audit.len(), tools.len());
        for tool in tools {
            assert!(
                audit.iter().any(|(candidate, _)| *candidate == tool),
                "missing feedback audit entry for {tool:?}"
            );
        }
    }

    #[test]
    fn drag_preview_bounds_support_square_and_center_modes() {
        let preview = CanvasDragPreview::new(ToolId::Rect, 10.0, 20.0, false, false)
            .with_current(30.0, 35.0, false, false);
        assert_eq!(
            canvas_drag_preview_bounds(preview),
            (10.0, 20.0, 20.0, 15.0)
        );

        let square = preview.with_current(30.0, 35.0, true, false);
        assert_eq!(canvas_drag_preview_bounds(square), (10.0, 20.0, 20.0, 20.0));

        let centered = preview.with_current(30.0, 35.0, false, true);
        assert_eq!(
            canvas_drag_preview_bounds(centered),
            (-10.0, 5.0, 40.0, 30.0)
        );
    }

    #[test]
    fn ellipse_preview_uses_svg_geometry_matching_drag_bounds() {
        assert!(CSS.contains(".canvas-shape-preview-svg"));
        assert!(CSS.contains(".canvas-shape-preview-ellipse"));

        let preview = CanvasDragPreview::new(ToolId::Ellipse, 100.0, 120.0, false, false)
            .with_current(160.0, 150.0, false, false);
        assert_eq!(
            canvas_drag_preview_bounds(preview),
            (100.0, 120.0, 60.0, 30.0)
        );
        assert_eq!(
            canvas_ellipse_preview_geometry(preview),
            (130.0, 135.0, 30.0, 15.0)
        );

        let circle = preview.with_current(160.0, 150.0, true, false);
        let (_, _, rx, ry) = canvas_ellipse_preview_geometry(circle);
        assert_eq!(rx, ry);
    }

    #[test]
    fn e2e_dragging_rect_tool_creates_renderable_content() {
        let mut engine = EditorEngine::new(240.0, 180.0);
        let before = count_rect_nodes(&engine);
        drag_tool(&mut engine, ToolId::Rect, (30.0, 24.0), (150.0, 110.0));
        assert!(count_rect_nodes(&engine) > before);

        let data_uri = sivgo_ui::render_to_data_uri(&engine, 240, 180);
        assert!(data_uri.starts_with("data:image/png;base64,"));
        assert!(data_uri.len() > 128);
    }

    #[test]
    fn e2e_draw_then_select_shape_updates_selection() {
        let mut engine = EditorEngine::new(240.0, 180.0);
        drag_tool(&mut engine, ToolId::Rect, (30.0, 24.0), (150.0, 110.0));
        engine.tools.switch_to(ToolId::Select);
        engine.on_mouse_down(engine_mouse_event_from_viewport_coords(
            60.0, 60.0, false, false, true,
        ));
        assert_eq!(engine.selection.len(), 1);
    }

    fn drag_tool(engine: &mut EditorEngine, tool: ToolId, start: (f64, f64), end: (f64, f64)) {
        engine.tools.switch_to(tool);
        engine.on_mouse_down(engine_mouse_event_from_viewport_coords(
            start.0, start.1, false, false, true,
        ));
        engine.on_mouse_move(engine_mouse_event_from_viewport_coords(
            end.0, end.1, false, false, true,
        ));
        engine.on_mouse_up(engine_mouse_event_from_viewport_coords(
            end.0, end.1, false, false, true,
        ));
    }

    fn count_rect_nodes(engine: &EditorEngine) -> usize {
        engine
            .document
            .nodes()
            .filter(|(_, node)| matches!(node.element, SvgElement::Rect(_)))
            .count()
    }
}

// ---------------------------------------------------------------------------
// CLI (kept for backward compatibility)
// ---------------------------------------------------------------------------

fn run_cli() {
    use clap::{Parser, Subcommand};

    /// Sivgo -- SVG vector graphics editor
    #[derive(Parser)]
    #[command(name = "sivgo", version, about = "SVG vector graphics editor")]
    struct Cli {
        #[command(subcommand)]
        command: Option<CliCommands>,
    }

    #[derive(Subcommand)]
    enum CliCommands {
        /// Render an SVG file to a PNG image.
        Render {
            input: PathBuf,
            output: PathBuf,
            #[arg(long, default_value_t = 800)]
            width: u32,
            #[arg(long, default_value_t = 600)]
            height: u32,
        },
        /// Show structural information about an SVG file.
        Info { input: PathBuf },
    }

    let cli = Cli::parse();

    match cli.command {
        Some(CliCommands::Render {
            input,
            output,
            width,
            height,
        }) => {
            let content = fs::read_to_string(&input).unwrap_or_else(|e| {
                eprintln!("Error reading '{}': {}", input.display(), e);
                process::exit(1);
            });
            let doc = sivgo_dom::parse_svg(&content).unwrap_or_else(|e| {
                eprintln!("Error parsing '{}': {}", input.display(), e);
                process::exit(1);
            });
            let mut backend = TinySkiaBackend::new(width, height);
            DocumentRenderer::render(&doc, &mut backend);
            let png_data = backend.pixmap().encode_png().unwrap_or_else(|e| {
                eprintln!("PNG encoding failed: {}", e);
                process::exit(1);
            });
            fs::write(&output, &png_data).unwrap_or_else(|e| {
                eprintln!("Error writing '{}': {}", output.display(), e);
                process::exit(1);
            });
            println!(
                "Rendered '{}' -> '{}' ({}x{})",
                input.display(),
                output.display(),
                width,
                height
            );
        }
        Some(CliCommands::Info { input }) => {
            let content = fs::read_to_string(&input).unwrap_or_else(|e| {
                eprintln!("Error reading '{}': {}", input.display(), e);
                process::exit(1);
            });
            let doc = sivgo_dom::parse_svg(&content).unwrap_or_else(|e| {
                eprintln!("Error parsing '{}': {}", input.display(), e);
                process::exit(1);
            });
            if let Some(root_id) = doc.root() {
                if let Some(root_node) = doc.get_node(root_id) {
                    match &root_node.element {
                        SvgElement::Svg(attrs) => {
                            print!("Root <svg>");
                            if let Some(ref w) = attrs.width {
                                print!("  width=\"{}\"", w);
                            }
                            if let Some(ref h) = attrs.height {
                                print!("  height=\"{}\"", h);
                            }
                            println!();
                        }
                        other => println!("Root element: <{}>", other.tag_name()),
                    }
                }
            }
            println!("Total nodes: {}", doc.len());
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for (_id, node) in doc.nodes() {
                *counts.entry(node.element.tag_name()).or_insert(0) += 1;
            }
            let mut sorted: Vec<(&&str, &usize)> = counts.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
            println!("Element types:");
            for (tag, count) in &sorted {
                println!("  {:>4}  <{}>", count, tag);
            }
        }
        None => launch_gui(),
    }
}

// ---------------------------------------------------------------------------
// GUI launch
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn color_to_hex(c: &Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn hex_to_color(hex: &str) -> Option<Color> {
    Color::from_hex(hex)
}

fn bump_render(render_version: Signal<u64>) {
    *render_version.write() += 1;
}

fn cursor_update_threshold_doc(zoom: f64) -> f64 {
    (CURSOR_UPDATE_SCREEN_EPSILON / zoom.max(0.05)).max(0.25)
}

fn should_update_cursor_position(
    previous_x: f64,
    previous_y: f64,
    next_x: f64,
    next_y: f64,
    zoom: f64,
) -> bool {
    let threshold = cursor_update_threshold_doc(zoom);
    (previous_x - next_x).abs() >= threshold || (previous_y - next_y).abs() >= threshold
}

fn update_cursor_position(
    cursor_position: glory::Cage<(f64, f64)>,
    next_x: f64,
    next_y: f64,
    zoom: f64,
) {
    let (previous_x, previous_y) = cursor_position.get_untracked();
    if should_update_cursor_position(previous_x, previous_y, next_x, next_y, zoom) {
        cursor_position.revise(|value| *value = (next_x, next_y));
    }
}

fn tool_commits_document_on_mouse_move(tool: ToolId) -> bool {
    matches!(tool, ToolId::Spray | ToolId::Tweak)
}

fn canvas_cursor_for_tool(tool: ToolId) -> &'static str {
    match tool {
        ToolId::Select | ToolId::Pages | ToolId::Lpe => "default",
        ToolId::Text => "text",
        ToolId::Zoom => "zoom-in",
        _ => "crosshair",
    }
}

fn should_invalidate_canvas_bitmap_for_mouse_move_tool(tool: ToolId) -> bool {
    tool_commits_document_on_mouse_move(tool)
}

fn tool_uses_canvas_drag_preview(tool: ToolId) -> bool {
    matches!(
        tool,
        ToolId::Rect
            | ToolId::Ellipse
            | ToolId::Star
            | ToolId::Spiral
            | ToolId::Box3d
            | ToolId::Text
            | ToolId::Zoom
            | ToolId::Mesh
            | ToolId::Pages
    )
}

fn tool_uses_canvas_path_preview(tool: ToolId) -> bool {
    matches!(
        tool,
        ToolId::Pen
            | ToolId::Pencil
            | ToolId::Calligraphy
            | ToolId::Measure
            | ToolId::Gradient
            | ToolId::Connector
    )
}

fn tool_needs_continuous_canvas_mouse_move(tool: ToolId) -> bool {
    matches!(
        tool,
        ToolId::Select
            | ToolId::Node
            | ToolId::Pen
            | ToolId::Pencil
            | ToolId::Calligraphy
            | ToolId::Eraser
            | ToolId::Spray
            | ToolId::Tweak
            | ToolId::Measure
            | ToolId::Zoom
            | ToolId::Pages
    )
}

fn should_dispatch_canvas_mouse_move(pointer_is_down: bool, tool: ToolId) -> bool {
    pointer_is_down && tool_needs_continuous_canvas_mouse_move(tool)
}

fn should_capture_canvas_mouse_move(pointer_is_down: bool, tool: ToolId) -> bool {
    pointer_is_down
        && (tool_needs_continuous_canvas_mouse_move(tool)
            || tool_uses_canvas_drag_preview(tool)
            || tool_uses_canvas_path_preview(tool))
}

fn page_border_hit(page: &PageEntry, doc_x: f64, doc_y: f64, tolerance: f64) -> bool {
    let left = (doc_x - page.x).abs() < tolerance
        && doc_y >= page.y - tolerance
        && doc_y <= page.y + page.height + tolerance;
    let right = (doc_x - (page.x + page.width)).abs() < tolerance
        && doc_y >= page.y - tolerance
        && doc_y <= page.y + page.height + tolerance;
    let top = (doc_y - page.y).abs() < tolerance
        && doc_x >= page.x - tolerance
        && doc_x <= page.x + page.width + tolerance;
    let bottom = (doc_y - (page.y + page.height)).abs() < tolerance
        && doc_x >= page.x - tolerance
        && doc_x <= page.x + page.width + tolerance;
    left || right || top || bottom
}

fn pages_tool_should_preview_new_page(
    pages: &[PageEntry],
    viewport_x: f64,
    viewport_y: f64,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
) -> bool {
    let zoom = zoom.max(0.05);
    let doc_x = (viewport_x - pan_x) / zoom;
    let doc_y = (viewport_y - pan_y) / zoom;
    let tolerance = 5.0 / zoom;
    !pages
        .iter()
        .any(|page| page_border_hit(page, doc_x, doc_y, tolerance))
}

fn should_start_canvas_drag_preview(
    tool: ToolId,
    pages: &[PageEntry],
    viewport_x: f64,
    viewport_y: f64,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
) -> bool {
    if !tool_uses_canvas_drag_preview(tool) {
        return false;
    }
    tool != ToolId::Pages
        || pages_tool_should_preview_new_page(pages, viewport_x, viewport_y, pan_x, pan_y, zoom)
}

fn is_canvas_drawable_element(element: &SvgElement) -> bool {
    matches!(
        element,
        SvgElement::Rect(_)
            | SvgElement::Circle(_)
            | SvgElement::Ellipse(_)
            | SvgElement::Line(_)
            | SvgElement::Polyline(_)
            | SvgElement::Polygon(_)
            | SvgElement::Path(_)
            | SvgElement::Use(_)
            | SvgElement::Image(_)
            | SvgElement::Text(_)
            | SvgElement::FlowRoot(_)
            | SvgElement::Box3d(_)
    )
}

fn document_has_drawable_target(engine: &EditorEngine) -> bool {
    engine
        .document
        .nodes()
        .any(|(_, node)| is_canvas_drawable_element(&node.element))
}

fn document_has_path_target(engine: &EditorEngine) -> bool {
    engine
        .document
        .nodes()
        .any(|(_, node)| matches!(node.element, SvgElement::Path(_)))
}

fn canvas_hit_id_at(engine: &EditorEngine, viewport_x: f64, viewport_y: f64) -> Option<NodeId> {
    let doc_pt = engine
        .viewport
        .screen_to_document(Point::new(viewport_x, viewport_y));
    sivgo_core::tools::select::hit_test_at(&engine.document, doc_pt)
}

fn canvas_path_hit_id_at(
    engine: &EditorEngine,
    viewport_x: f64,
    viewport_y: f64,
) -> Option<NodeId> {
    let doc_pt = engine
        .viewport
        .screen_to_document(Point::new(viewport_x, viewport_y));
    let tolerance = 4.0 / engine.viewport.zoom.max(0.05);
    engine.document.nodes().find_map(|(id, node)| {
        if !matches!(node.element, SvgElement::Path(_)) {
            return None;
        }
        let bounds = element_bounding_box(&node.element)?;
        let padded = Rect::new(
            bounds.x0 - tolerance,
            bounds.y0 - tolerance,
            bounds.x1 + tolerance,
            bounds.y1 + tolerance,
        );
        padded.contains_point(doc_pt).then_some(*id)
    })
}

fn edit_tool_for_node(engine: &EditorEngine, id: NodeId) -> Option<ToolId> {
    let node = engine.document.get_node(id)?;
    match node.element {
        SvgElement::Path(_) => Some(ToolId::Node),
        SvgElement::Text(_) | SvgElement::FlowPara(_) | SvgElement::TextPath(_) => {
            Some(ToolId::Text)
        }
        _ => None,
    }
}

fn enter_object_edit_mode(engine: &mut EditorEngine, id: NodeId) -> Option<ToolId> {
    let tool = edit_tool_for_node(engine, id)?;
    engine.select_node(id);
    engine.tools.switch_to(tool);
    Some(engine.tools.active_id())
}

fn enter_canvas_edit_mode_at(
    engine: &mut EditorEngine,
    viewport_x: f64,
    viewport_y: f64,
) -> Option<ToolId> {
    let id = canvas_path_hit_id_at(engine, viewport_x, viewport_y)
        .or_else(|| canvas_hit_id_at(engine, viewport_x, viewport_y))?;
    enter_object_edit_mode(engine, id)
}

fn canvas_tool_feedback_on_mouse_down(
    tool: ToolId,
    primary_selection: Option<NodeId>,
    selected_path_target: bool,
    hit_id: Option<NodeId>,
    path_hit_id: Option<NodeId>,
    has_document_drawable_target: bool,
    has_document_path_target: bool,
) -> Option<&'static str> {
    match tool {
        ToolId::Gradient | ToolId::Mesh if primary_selection.is_none() => {
            Some("Select an object first")
        }
        ToolId::Dropper if hit_id.is_none() => Some("Click an object to sample"),
        ToolId::Dropper if primary_selection.is_none() => Some("Select a target first"),
        ToolId::Dropper if primary_selection == hit_id => Some("Sample another object"),
        ToolId::BucketFill if hit_id.is_none() => Some("Click an object to fill"),
        ToolId::Eraser if hit_id.is_none() => Some("Click an object to erase"),
        ToolId::Connector if hit_id.is_none() => Some("Start on an object"),
        ToolId::Marker if !selected_path_target => Some("Select a path first"),
        ToolId::Spray if primary_selection.is_none() => Some("Select an object to spray"),
        ToolId::Tweak if !has_document_drawable_target => Some("Create an object to tweak"),
        ToolId::Lpe if !has_document_path_target => Some("Create or select a path first"),
        ToolId::Lpe if path_hit_id.is_none() => Some("Click a path target"),
        _ => None,
    }
}

fn canvas_tool_feedback_on_mouse_up(
    tool: ToolId,
    source_hit_id: Option<NodeId>,
    release_hit_id: Option<NodeId>,
) -> Option<&'static str> {
    match tool {
        ToolId::Connector if source_hit_id.is_none() => Some("Start on an object"),
        ToolId::Connector if release_hit_id.is_none() => Some("Release on another object"),
        ToolId::Connector if source_hit_id == release_hit_id => Some("Release on another object"),
        _ => None,
    }
}

fn canvas_drag_preview_bounds(preview: CanvasDragPreview) -> (f64, f64, f64, f64) {
    let mut dx = preview.current_x - preview.start_x;
    let mut dy = preview.current_y - preview.start_y;

    if matches!(
        preview.tool,
        ToolId::Rect | ToolId::Ellipse | ToolId::Star | ToolId::Spiral | ToolId::Box3d
    ) && preview.ctrl
    {
        let side = dx.abs().max(dy.abs());
        dx = side * dx.signum();
        dy = side * dy.signum();
    }

    let (x0, y0, x1, y1) = if preview.shift {
        (
            preview.start_x - dx,
            preview.start_y - dy,
            preview.start_x + dx,
            preview.start_y + dy,
        )
    } else {
        (
            preview.start_x,
            preview.start_y,
            preview.start_x + dx,
            preview.start_y + dy,
        )
    };

    let left = x0.min(x1);
    let top = y0.min(y1);
    let width = (x1 - x0).abs().max(1.0);
    let height = (y1 - y0).abs().max(1.0);
    (left, top, width, height)
}

#[cfg(test)]
fn canvas_screen_rect_style(left: f64, top: f64, width: f64, height: f64) -> String {
    format!(
        "left: {left}px; top: {top}px; width: {}px; height: {}px;",
        width.max(1.0),
        height.max(1.0)
    )
}

#[cfg(test)]
fn canvas_selection_overlay_style(
    bounds: Rect,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    delta_x: f64,
    delta_y: f64,
) -> String {
    canvas_screen_rect_style(
        bounds.x0 * zoom + pan_x + delta_x,
        bounds.y0 * zoom + pan_y + delta_y,
        bounds.width() * zoom,
        bounds.height() * zoom,
    )
}

fn canvas_ellipse_preview_geometry(preview: CanvasDragPreview) -> (f64, f64, f64, f64) {
    let (left, top, width, height) = canvas_drag_preview_bounds(preview);
    (
        left + width / 2.0,
        top + height / 2.0,
        width / 2.0,
        height / 2.0,
    )
}

#[cfg(test)]
fn canvas_rubber_band_preview_style(preview: CanvasSelectPreview) -> String {
    let left = preview.start_x.min(preview.current_x);
    let top = preview.start_y.min(preview.current_y);
    let width = (preview.current_x - preview.start_x).abs();
    let height = (preview.current_y - preview.start_y).abs();
    canvas_screen_rect_style(left, top, width, height)
}

#[cfg(test)]
fn tool_feedback_audit_covers_all_tools() -> &'static [(ToolId, &'static str)] {
    &[
        (ToolId::Select, "selection/transform-on-release"),
        (ToolId::Node, "node-edit-on-release"),
        (ToolId::Rect, "drag-rect-preview"),
        (ToolId::Ellipse, "drag-rect-preview"),
        (ToolId::Star, "drag-rect-preview"),
        (ToolId::Spiral, "drag-rect-preview"),
        (ToolId::Pen, "path-preview-confirm"),
        (ToolId::Pencil, "path-preview-commit-on-up"),
        (ToolId::Calligraphy, "path-preview-commit-on-up"),
        (ToolId::Text, "drag-rect-preview"),
        (ToolId::Gradient, "path-preview-or-selection-feedback"),
        (ToolId::Dropper, "click-target-feedback"),
        (ToolId::BucketFill, "click-target-feedback"),
        (ToolId::Connector, "path-preview-or-target-feedback"),
        (ToolId::Eraser, "click-target-feedback"),
        (ToolId::Spray, "mousemove-mutates-or-selection-feedback"),
        (ToolId::Tweak, "mousemove-mutates-or-object-feedback"),
        (ToolId::Measure, "path-preview"),
        (ToolId::Zoom, "drag-rect-preview"),
        (ToolId::Mesh, "drag-rect-preview-or-selection-feedback"),
        (ToolId::Box3d, "drag-rect-preview"),
        (ToolId::Marker, "click-path-feedback"),
        (ToolId::Pages, "page-create-preview"),
        (ToolId::Lpe, "click-path-feedback"),
    ]
}

fn current_document_name(path: Option<&PathBuf>) -> String {
    path.and_then(|file| file.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled.svg".to_string())
}

fn set_window_title(desktop: &DesktopContext, path: Option<&PathBuf>) {
    desktop.set_title(&format!("Sivgo Studio - {}", current_document_name(path)));
}

fn px_to_mm(px: f64) -> f64 {
    px / PX_PER_MM
}

fn mm_to_px(mm: f64) -> f64 {
    mm * PX_PER_MM
}

fn sanitize_positive(value: f64, fallback: f64, min: f64, max: f64) -> f64 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

fn sanitize_inspector_width(value: f64) -> f64 {
    sanitize_positive(
        value,
        INSPECTOR_WIDTH_DEFAULT,
        INSPECTOR_WIDTH_MIN,
        INSPECTOR_WIDTH_MAX,
    )
}

/// Preserve enough canvas space when a saved wide inspector is opened in a smaller window.
fn effective_inspector_width(window_width: f64, requested_width: f64) -> f64 {
    let available = (window_width - LEFT_RAIL_WIDTH - INSPECTOR_RESIZE_HANDLE_WIDTH - 320.0)
        .max(INSPECTOR_WIDTH_MIN);
    sanitize_inspector_width(requested_width).min(available)
}

fn sanitize_zoom(value: f64) -> f64 {
    sanitize_positive(value, 1.0, ZOOM_MIN, ZOOM_MAX)
}

fn parse_zoom_percent(value: &str) -> Option<f64> {
    let normalized = value.trim().trim_end_matches('%').trim();
    let percent = normalized.parse::<f64>().ok()?;
    (percent.is_finite() && percent > 0.0).then(|| sanitize_zoom(percent / 100.0))
}

fn wheel_delta_to_zoom_steps(delta: WheelDelta) -> f64 {
    match delta {
        WheelDelta::Pixels(raw) => (-raw / 120.0).clamp(-6.0, 6.0),
        #[cfg(test)]
        WheelDelta::Lines(raw) => (-raw / 3.0).clamp(-6.0, 6.0),
        #[cfg(test)]
        WheelDelta::Pages(raw) => (-raw).clamp(-6.0, 6.0),
    }
}

fn set_viewport_zoom_at_screen_point(
    engine: &mut EditorEngine,
    target_zoom: f64,
    center: sivgo_math::Point,
) {
    let current_zoom = engine.viewport.zoom.max(ZOOM_MIN);
    let target_zoom = sanitize_zoom(target_zoom);
    let factor = target_zoom / current_zoom;
    engine.viewport.zoom_at(center, factor);
    engine.viewport.zoom = sanitize_zoom(engine.viewport.zoom);
}

fn canvas_viewport_width_for_window(window_width: f64, inspector_width: f64) -> f64 {
    sanitize_positive(
        window_width
            - LEFT_RAIL_WIDTH
            - INSPECTOR_RESIZE_HANDLE_WIDTH
            - inspector_width
            - CANVAS_RULER_THICKNESS
            - CANVAS_SCROLLBAR_THICKNESS,
        1.0,
        1.0,
        16_384.0,
    )
}

fn sanitize_hex_string(value: &str, fallback: &str) -> String {
    if Color::from_hex(value).is_some() {
        value.to_string()
    } else {
        fallback.to_string()
    }
}

fn safe_page_file_stem(label: &str, index: usize) -> String {
    let raw = if label.trim().is_empty() {
        format!("page_{index}")
    } else {
        label.trim().to_string()
    };

    let stem: String = raw
        .chars()
        .map(|ch| {
            if ch.is_control()
                || ch.is_whitespace()
                || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect();
    let stem = stem
        .trim_matches(|ch| ch == '.' || ch == '_' || ch == ' ')
        .to_string();
    if stem.is_empty() {
        format!("page_{index}")
    } else {
        stem
    }
}

fn export_pages_svg_to_folder(engine: &EditorEngine, folder: &Path) -> Result<usize, String> {
    let exports = engine.export_all_pages_svg();
    if exports.is_empty() {
        return Err("Document has no pages to export".to_string());
    }

    let mut written = 0usize;
    for (idx, (label, svg)) in exports.into_iter().enumerate() {
        let stem = safe_page_file_stem(&label, idx + 1);
        let path = folder.join(format!("{:03}_{stem}.svg", idx + 1));
        fs::write(&path, svg.as_bytes())
            .map_err(|err| format!("Failed to write {}: {err}", path.display()))?;
        written += 1;
    }
    Ok(written)
}

fn export_pages_pdf(engine: &EditorEngine, path: &Path) -> Result<usize, String> {
    let pages = engine.get_pages();
    if pages.is_empty() {
        return Err("Document has no pages to export".to_string());
    }

    let mut exporter = sivgo_io::MultiPagePdfExporter::new();
    let title = root_text(engine, "title");
    if !title.trim().is_empty() {
        exporter.metadata.title = Some(title);
    }
    let author = root_text(engine, "metadata");
    if !author.trim().is_empty() {
        exporter.metadata.author = Some(author);
    }

    for (page_id, _label, _x, _y, width, height) in pages {
        if let Some(svg) = engine.export_page_svg(page_id) {
            exporter.add_page(svg, width.max(1.0), height.max(1.0));
        }
    }

    let count = exporter.pages.len();
    if count == 0 {
        return Err("No valid pages could be exported".to_string());
    }
    exporter
        .export(path)
        .map_err(|err| format!("Failed to write {}: {err}", path.display()))?;
    Ok(count)
}

fn desktop_config_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("sivgo");
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("sivgo");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config").join("sivgo");
    }
    std::env::temp_dir().join("sivgo")
}

fn desktop_settings_path() -> PathBuf {
    desktop_config_dir().join("desktop_settings.json")
}

fn load_desktop_settings_from_disk() -> DesktopSettings {
    let path = desktop_settings_path();
    match fs::read_to_string(&path) {
        Ok(json) => match serde_json::from_str::<DesktopSettings>(&json) {
            Ok(settings) => settings.sanitized(),
            Err(err) => {
                log::warn!(
                    "Failed to parse desktop settings at {}: {}",
                    path.display(),
                    err
                );
                DesktopSettings::default()
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => DesktopSettings::default(),
        Err(err) => {
            log::warn!(
                "Failed to read desktop settings at {}: {}",
                path.display(),
                err
            );
            DesktopSettings::default()
        }
    }
}

#[cfg(not(test))]
fn initial_desktop_settings() -> DesktopSettings {
    INITIAL_DESKTOP_SETTINGS
        .get_or_init(load_desktop_settings_from_disk)
        .clone()
}

fn save_desktop_settings(settings: &DesktopSettings) {
    let path = desktop_settings_path();
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            log::warn!(
                "Failed to create desktop settings directory {}: {}",
                parent.display(),
                err
            );
            return;
        }
    }

    let settings = settings.clone().sanitized();
    let json = match serde_json::to_string_pretty(&settings) {
        Ok(json) => json,
        Err(err) => {
            log::warn!("Failed to serialize desktop settings: {}", err);
            return;
        }
    };

    if let Err(err) = fs::write(&path, json) {
        log::warn!(
            "Failed to write desktop settings at {}: {}",
            path.display(),
            err
        );
    }
}

fn persist_desktop_settings(preferences: &PreferencesState, document_defaults: &DocumentDefaults) {
    let layout = load_desktop_settings_from_disk().layout;
    persist_desktop_settings_with_layout(preferences, document_defaults, &layout);
}

fn desktop_settings_snapshot(
    preferences: &PreferencesState,
    document_defaults: &DocumentDefaults,
    layout: &LayoutSettings,
) -> DesktopSettings {
    DesktopSettings {
        preferences: preferences.clone(),
        document_defaults: document_defaults.clone(),
        layout: layout.clone(),
    }
    .sanitized()
}

fn persist_desktop_settings_with_layout(
    preferences: &PreferencesState,
    document_defaults: &DocumentDefaults,
    layout: &LayoutSettings,
) {
    let settings = desktop_settings_snapshot(preferences, document_defaults, layout);
    save_desktop_settings(&settings);
}

fn copy_svg_to_native_clipboard(svg: &str) {
    if svg.trim().is_empty() {
        return;
    }

    match ClipboardManager::new().and_then(|mut clipboard| clipboard.copy_svg(svg)) {
        Ok(()) => {
            let _ = copy_svg_to_stub_clipboard(svg);
        }
        Err(err) => {
            log::warn!("System clipboard unavailable, using in-process clipboard: {err}");
            let _ = copy_svg_to_stub_clipboard(svg);
        }
    }
}

fn copy_text_to_native_clipboard(text: &str) {
    if text.trim().is_empty() {
        return;
    }

    match ClipboardManager::new().and_then(|mut clipboard| clipboard.copy_text(text)) {
        Ok(()) => {
            let _ = copy_text_to_stub_clipboard(text);
        }
        Err(err) => {
            log::warn!("System clipboard unavailable, using in-process text clipboard: {err}");
            let _ = copy_text_to_stub_clipboard(text);
        }
    }
}

fn copy_bitmap_to_native_clipboard(pixels: &[u8], width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("Cannot copy an empty bitmap to the clipboard".to_string());
    }

    match ClipboardManager::new()
        .and_then(|mut clipboard| clipboard.copy_bitmap(pixels, width, height))
    {
        Ok(()) => {
            let _ = copy_image_to_stub_clipboard(pixels, width, height);
            Ok(())
        }
        Err(err) => {
            log::warn!("System clipboard unavailable, using in-process bitmap clipboard: {err}");
            copy_image_to_stub_clipboard(pixels, width, height)
        }
    }
}

fn copy_editor_bitmap_to_native_clipboard(engine: &EditorEngine) -> Result<(), String> {
    let (pixels, width, height) = clipboard_bitmap_for_engine(engine)?;
    copy_bitmap_to_native_clipboard(&pixels, width, height)
}

fn clipboard_bitmap_for_engine(engine: &EditorEngine) -> Result<(Vec<u8>, u32, u32), String> {
    if let Some(bounds) = engine.selection.bounding_box(&engine.document) {
        return render_document_region_bitmap(&engine.document, bounds.inflate(2.0, 2.0));
    }

    let (width, height) = document_pixel_size(engine);
    let pixels = sivgo_render::render_document_to_pixmap(&engine.document, width, height);
    Ok((pixels, width, height))
}

fn render_document_region_bitmap(
    document: &sivgo_dom::SvgDocument,
    bounds: Rect,
) -> Result<(Vec<u8>, u32, u32), String> {
    if bounds.is_empty() || !bounds.width().is_finite() || !bounds.height().is_finite() {
        return Err("Selection has no rasterizable area".to_string());
    }

    let width = bounds.width().ceil().clamp(1.0, 16_384.0) as u32;
    let height = bounds.height().ceil().clamp(1.0, 16_384.0) as u32;
    let mut cropped_document = document.clone();
    let root = cropped_document
        .root()
        .ok_or_else(|| "Document has no root SVG node".to_string())?;
    let root_node = cropped_document
        .get_node_mut(root)
        .ok_or_else(|| "Root SVG node is missing".to_string())?;

    let SvgElement::Svg(attrs) = &mut root_node.element else {
        return Err("Root node is not an <svg> element".to_string());
    };

    attrs.width = Some(SvgLength::px(width as f64));
    attrs.height = Some(SvgLength::px(height as f64));
    attrs.view_box = Some(ViewBox::new(
        bounds.x0,
        bounds.y0,
        bounds.width(),
        bounds.height(),
    ));
    attrs.preserve_aspect_ratio = Some(PreserveAspectRatio {
        align: Align::None,
        meet_or_slice: MeetOrSlice::Meet,
    });

    let pixels = sivgo_render::render_document_to_pixmap(&cropped_document, width, height);
    Ok((pixels, width, height))
}

fn read_svg_from_native_clipboard() -> Option<String> {
    match ClipboardManager::new().and_then(|mut clipboard| clipboard.paste_svg()) {
        Ok(Some(svg)) => Some(svg),
        Ok(None) => None,
        Err(err) => {
            log::warn!("Failed to read system clipboard, using in-process clipboard: {err}");
            paste_svg_from_stub_clipboard().ok().flatten()
        }
    }
}

fn read_bitmap_from_native_clipboard() -> Option<(Vec<u8>, u32, u32)> {
    match ClipboardManager::new().and_then(|mut clipboard| clipboard.paste_bitmap()) {
        Ok(Some(bitmap)) => Some(bitmap),
        Ok(None) => None,
        Err(err) => {
            log::warn!("Failed to read bitmap clipboard, using in-process clipboard: {err}");
            paste_image_from_stub_clipboard().ok().flatten()
        }
    }
}

fn read_text_from_native_clipboard() -> Option<String> {
    match ClipboardManager::new().and_then(|mut clipboard| clipboard.paste_text()) {
        Ok(Some(text)) if !text.trim().is_empty() => Some(text),
        Ok(_) => None,
        Err(err) => {
            log::warn!("Failed to read text clipboard, using in-process clipboard: {err}");
            paste_text_from_stub_clipboard()
                .ok()
                .flatten()
                .filter(|text| !text.trim().is_empty())
        }
    }
}

fn read_preferred_clipboard_payload() -> Option<ClipboardPayload> {
    ClipboardSnapshot {
        svg_text: read_svg_from_native_clipboard(),
        bitmap: read_bitmap_from_native_clipboard(),
        text: read_text_from_native_clipboard(),
    }
    .preferred_payload()
}

fn encode_rgba_png(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let expected = width as usize * height as usize * 4;
    if pixels.len() != expected {
        return Err(format!(
            "Pixel buffer size mismatch: expected {expected} bytes, got {}",
            pixels.len()
        ));
    }

    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("Failed to write PNG header: {e}"))?;
        writer
            .write_image_data(pixels)
            .map_err(|e| format!("Failed to encode clipboard bitmap as PNG: {e}"))?;
    }
    Ok(encoded)
}

fn execute_add_element(
    engine: &mut EditorEngine,
    element: SvgElement,
    presentation: PresentationAttrs,
) -> Option<NodeId> {
    let parent = current_layer_id(engine).or_else(|| engine.document.root())?;
    engine.execute(Box::new(AddNodeCommand::new(
        parent,
        element,
        presentation,
        None,
    )));
    let created_id = engine.document.children(parent).last().copied();
    if let Some(id) = created_id {
        engine.select_node(id);
    }
    created_id
}

fn paste_bitmap_into_document(
    engine: &mut EditorEngine,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> Result<Option<NodeId>, String> {
    use base64::Engine;

    if width == 0 || height == 0 {
        return Ok(None);
    }

    let png_bytes = encode_rgba_png(pixels, width, height)?;
    let data_uri = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png_bytes)
    );
    let center = engine.viewport.screen_to_document(sivgo_math::Point::new(
        engine.viewport.width * 0.5,
        engine.viewport.height * 0.5,
    ));
    let x = center.x - width as f64 * 0.5;
    let y = center.y - height as f64 * 0.5;
    let element = SvgElement::Image(ImageAttrs {
        href: Some(data_uri),
        x: Some(SvgLength::px(x)),
        y: Some(SvgLength::px(y)),
        width: Some(SvgLength::px(width as f64)),
        height: Some(SvgLength::px(height as f64)),
        preserve_aspect_ratio: None,
    });

    Ok(execute_add_element(
        engine,
        element,
        PresentationAttrs::default(),
    ))
}

fn paste_text_into_document(engine: &mut EditorEngine, text: &str) -> Option<NodeId> {
    let text = text.trim_matches('\0').trim();
    if text.is_empty() {
        return None;
    }

    let center = engine.viewport.screen_to_document(sivgo_math::Point::new(
        engine.viewport.width * 0.5,
        engine.viewport.height * 0.5,
    ));
    let element = SvgElement::Text(TextAttrs {
        x: vec![SvgLength::new(center.x, LengthUnit::None)],
        y: vec![SvgLength::new(center.y, LengthUnit::None)],
        text_content: text.to_string(),
        ..Default::default()
    });
    let presentation = PresentationAttrs {
        fill: Some(Paint::Color(Color::BLACK)),
        font_size: Some(SvgLength::new(16.0, LengthUnit::None)),
        ..Default::default()
    };

    execute_add_element(engine, element, presentation)
}

fn apply_preferences_to_engine(engine: &mut EditorEngine, preferences: &PreferencesState) {
    engine.snapping.prefs.snap_to_grid = preferences.snap_to_grid;
    engine.snapping.prefs.snap_to_guides = preferences.snap_to_guides;
    engine.snapping.prefs.snap_to_path_nodes = preferences.snap_to_nodes;
    engine.snapping.prefs.snap_to_path = preferences.snap_to_paths;
    engine.snapping.prefs.snap_to_bbox_edges = preferences.snap_to_objects;
    engine.snapping.prefs.snap_to_bbox_centers = preferences.snap_to_objects;
    engine.snapping.prefs.snap_to_bbox_corners = preferences.snap_to_objects;
    engine.snapping.prefs.snap_to_midpoints = preferences.snap_to_objects;
    engine.snapping.prefs.snap_to_intersections = preferences.snap_to_intersections;
    engine.snapping.prefs.snap_to_equal_spacing = preferences.snap_to_equal_spacing;
    engine.snapping.prefs.sources.snap_from_cursor = preferences.snap_from_cursor;
    engine.snapping.prefs.sources.snap_from_object_nodes = preferences.snap_from_object_nodes;
    engine.snapping.prefs.sources.snap_from_bbox_corners = preferences.snap_from_bbox_corners;
    engine.snapping.prefs.sources.snap_from_bbox_centers = preferences.snap_from_bbox_centers;
    engine.snapping.prefs.sources.snap_from_bbox_midpoints = preferences.snap_from_bbox_midpoints;
    engine.snapping.prefs.tolerance =
        sanitize_positive(preferences.snap_distance, 10.0, 1.0, 512.0);
}

fn apply_document_defaults_to_engine(engine: &mut EditorEngine, defaults: &DocumentDefaults) {
    let defaults = defaults.clone().sanitized();
    engine.set_document_size(defaults.width_px, defaults.height_px);
    engine.set_page_color(defaults.background_color);
    engine.set_grid_visible(defaults.grid_enabled);
    engine.snapping.prefs.grid_spacing = defaults.grid_spacing_x;
    engine.snapping.prefs.snap_to_grid = defaults.snap_to_grid;
    engine.snapping.prefs.snap_to_guides = defaults.snap_to_guides;

    if !defaults.title.is_empty() {
        engine.set_document_title(defaults.title);
    }
    if !defaults.author.is_empty() {
        engine.set_document_metadata(defaults.author);
    }
    if !defaults.description.is_empty() {
        engine.set_document_description(defaults.description);
    }
}

fn document_pixel_size(engine: &EditorEngine) -> (u32, u32) {
    let (width, height) = engine.document_size();
    (
        width.round().clamp(1.0, 16_384.0) as u32,
        height.round().clamp(1.0, 16_384.0) as u32,
    )
}

fn root_text(engine: &EditorEngine, tag_name: &str) -> String {
    let Some(root) = engine.document.root() else {
        return String::new();
    };
    engine
        .document
        .children(root)
        .iter()
        .find_map(|id| {
            let node = engine.document.get_node(*id)?;
            match &node.element {
                SvgElement::Title(attrs) if tag_name == "title" => Some(attrs.text_content.clone()),
                SvgElement::Desc(attrs) if tag_name == "desc" => Some(attrs.text_content.clone()),
                SvgElement::Metadata(attrs) if tag_name == "metadata" => {
                    Some(attrs.raw_content.clone())
                }
                _ => None,
            }
        })
        .unwrap_or_default()
}

fn page_color(engine: &EditorEngine) -> String {
    let Some(root) = engine.document.root() else {
        return "#ffffff".to_string();
    };
    engine
        .document
        .children(root)
        .iter()
        .find_map(|id| {
            let node = engine.document.get_node(*id)?;
            match &node.element {
                SvgElement::SodipodiNamedView(attrs) => attrs.page_color.clone(),
                _ => None,
            }
        })
        .unwrap_or_else(|| "#ffffff".to_string())
}

fn grid_visible(engine: &EditorEngine) -> bool {
    let Some(root) = engine.document.root() else {
        return false;
    };
    engine
        .document
        .children(root)
        .iter()
        .find_map(|id| {
            let node = engine.document.get_node(*id)?;
            match &node.element {
                SvgElement::SodipodiNamedView(attrs) => attrs.show_grid.clone(),
                _ => None,
            }
        })
        .map(|value| matches!(value.as_str(), "true" | "1" | "yes"))
        .unwrap_or(false)
}

fn object_tree_type_name(element: &SvgElement) -> &'static str {
    match element {
        SvgElement::Rect(_) => "rect",
        SvgElement::Circle(_) => "circle",
        SvgElement::Ellipse(_) => "ellipse",
        SvgElement::Line(_) => "line",
        SvgElement::Polyline(_) => "polyline",
        SvgElement::Polygon(_) => "polygon",
        SvgElement::Path(_) => "path",
        SvgElement::Group(_) => "g",
        SvgElement::Text(_) | SvgElement::TSpan(_) | SvgElement::TextPath(_) => "text",
        SvgElement::FlowRoot(_) | SvgElement::FlowPara(_) | SvgElement::FlowSpan(_) => "flowRoot",
        SvgElement::Image(_) => "image",
        SvgElement::Use(_) => "use",
        SvgElement::Symbol(_) => "symbol",
        SvgElement::Svg(_) => "svg",
        _ => "object",
    }
}

fn object_tree_visible(element: &SvgElement) -> bool {
    matches!(
        element,
        SvgElement::Rect(_)
            | SvgElement::Circle(_)
            | SvgElement::Ellipse(_)
            | SvgElement::Line(_)
            | SvgElement::Polyline(_)
            | SvgElement::Polygon(_)
            | SvgElement::Path(_)
            | SvgElement::Group(_)
            | SvgElement::Text(_)
            | SvgElement::TSpan(_)
            | SvgElement::TextPath(_)
            | SvgElement::FlowRoot(_)
            | SvgElement::FlowPara(_)
            | SvgElement::FlowSpan(_)
            | SvgElement::Image(_)
            | SvgElement::Use(_)
            | SvgElement::Symbol(_)
    )
}

fn object_tree_label(engine: &EditorEngine, id: NodeId) -> String {
    let Some(node) = engine.document.get_node(id) else {
        return "Object".to_string();
    };
    if let Some(label) = node.attributes.get("inkscape:label") {
        if !label.trim().is_empty() {
            return label.clone();
        }
    }
    if let Some(xml_id) = &node.xml_id {
        if !xml_id.trim().is_empty() {
            return xml_id.clone();
        }
    }

    let id_text = id.to_string();
    let short_id = id_text.get(..8).unwrap_or(&id_text);
    format!("{} {}", object_tree_type_name(&node.element), short_id)
}

fn push_object_tree_entries(
    engine: &EditorEngine,
    id: NodeId,
    depth: usize,
    entries: &mut Vec<ObjectEntry>,
) {
    let Some(node) = engine.document.get_node(id) else {
        return;
    };
    if !object_tree_visible(&node.element) {
        return;
    }

    entries.push(ObjectEntry {
        id,
        label: object_tree_label(engine, id),
        type_name: object_tree_type_name(&node.element).to_string(),
        depth,
        visible: !node.hidden,
        locked: node.locked,
        selected: engine.selection.is_selected(id),
    });

    for &child in engine.document.children(id) {
        push_object_tree_entries(engine, child, depth + 1, entries);
    }
}

fn object_tree_entries(engine: &EditorEngine) -> Vec<ObjectEntry> {
    let mut entries = Vec::new();
    if let Some(root) = engine.document.root() {
        for &child in engine.document.children(root) {
            push_object_tree_entries(engine, child, 0, &mut entries);
        }
    }
    entries
}

fn selected_object_details(engine: &EditorEngine) -> Option<ObjectDetails> {
    let id = engine.selection.primary()?;
    let node = engine.document.get_node(id)?;
    Some(ObjectDetails {
        id,
        label: object_tree_label(engine, id),
        type_name: object_tree_type_name(&node.element).to_string(),
        xml_id: node.xml_id.clone().unwrap_or_default(),
    })
}

fn selected_path_has_lpe_target(engine: &EditorEngine) -> bool {
    let Some(id) = engine.selection.primary() else {
        return false;
    };
    engine
        .document
        .get_node(id)
        .is_some_and(|node| matches!(node.element, SvgElement::Path(_)))
}

fn format_lpe_value(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        format!("{value:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn selected_lpe_entries(engine: &EditorEngine) -> Vec<UiLpeEntry> {
    engine
        .selected_lpe_entries()
        .into_iter()
        .filter_map(|entry| {
            let kind = UiLpeKind::from_key(&entry.kind)?;
            Some(UiLpeEntry {
                id: entry.id,
                kind,
                params: entry
                    .params
                    .into_iter()
                    .map(|param| UiLpeParam {
                        name: param.name,
                        value: format_lpe_value(param.value),
                    })
                    .collect(),
                enabled: entry.enabled,
            })
        })
        .collect()
}

fn selected_filter_primitives(engine: &EditorEngine) -> Vec<UiFilterPrimitive> {
    engine
        .selected_filter_primitives()
        .into_iter()
        .filter_map(|entry| {
            let kind = UiFilterPrimitiveKind::from_key(&entry.kind)?;
            Some(UiFilterPrimitive {
                id: entry.id,
                kind,
                params: entry
                    .params
                    .into_iter()
                    .map(|param| UiFilterParam {
                        name: param.name,
                        value: param.value,
                    })
                    .collect(),
                enabled: entry.enabled,
            })
        })
        .collect()
}

fn resource_kind_from_core(kind: &str) -> Option<UiResourceKind> {
    match kind {
        "gradient" => Some(UiResourceKind::Gradient),
        "pattern" => Some(UiResourceKind::Pattern),
        "font" => Some(UiResourceKind::Font),
        "image" => Some(UiResourceKind::Image),
        "filter" => Some(UiResourceKind::Filter),
        "marker" => Some(UiResourceKind::Marker),
        "symbol" => Some(UiResourceKind::Symbol),
        "clip_path" => Some(UiResourceKind::ClipPath),
        "mask" => Some(UiResourceKind::Mask),
        "template" => Some(UiResourceKind::Template),
        "tutorial" => Some(UiResourceKind::Tutorial),
        "extension_asset" => Some(UiResourceKind::ExtensionAsset),
        _ => None,
    }
}

fn document_resource_entries(engine: &EditorEngine) -> Vec<UiResourceEntry> {
    engine
        .document_resources()
        .into_iter()
        .filter_map(|entry| {
            Some(UiResourceEntry::new(
                entry.id,
                resource_kind_from_core(&entry.kind)?,
                entry.name,
                entry.usage_count,
            ))
        })
        .collect()
}

fn page_entries(engine: &EditorEngine) -> Vec<PageEntry> {
    let pages = engine.get_pages();
    let active_id = if pages.is_empty() {
        None
    } else {
        let viewport_center = engine.viewport.screen_to_document(sivgo_math::Point::new(
            engine.viewport.width / 2.0,
            engine.viewport.height / 2.0,
        ));
        pages
            .iter()
            .min_by(|a, b| {
                let a_center_x = a.2 + a.4 / 2.0;
                let a_center_y = a.3 + a.5 / 2.0;
                let b_center_x = b.2 + b.4 / 2.0;
                let b_center_y = b.3 + b.5 / 2.0;
                let a_dist = (a_center_x - viewport_center.x).powi(2)
                    + (a_center_y - viewport_center.y).powi(2);
                let b_dist = (b_center_x - viewport_center.x).powi(2)
                    + (b_center_y - viewport_center.y).powi(2);
                a_dist
                    .partial_cmp(&b_dist)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|page| page.0)
    };

    pages
        .into_iter()
        .enumerate()
        .map(|(idx, (id, label, x, y, width, height))| PageEntry {
            index: idx + 1,
            id,
            label: if label.trim().is_empty() {
                format!("Page {}", idx + 1)
            } else {
                label
            },
            x,
            y,
            width,
            height,
            active: active_id == Some(id),
        })
        .collect()
}

fn next_page_origin(pages: &[(NodeId, String, f64, f64, f64, f64)]) -> (f64, f64) {
    pages
        .last()
        .map(|(_, _, x, y, width, _)| (*x + width.max(1.0) + 20.0, *y))
        .unwrap_or((0.0, 0.0))
}

fn add_page_like_previous(engine: &mut EditorEngine) -> Option<NodeId> {
    let existing_pages = engine.get_pages();
    let (doc_w, doc_h) = engine.document_size();
    let (x, y) = next_page_origin(&existing_pages);
    let (width, height) = existing_pages
        .last()
        .map(|(_, _, _, _, width, height)| (width.max(1.0), height.max(1.0)))
        .unwrap_or((doc_w.max(1.0), doc_h.max(1.0)));
    let label = format!("Page {}", existing_pages.len() + 1);
    let page_id = engine.add_page(x, y, width, height, Some(&label));
    if let Some(id) = page_id {
        engine.navigate_to_page(id);
    }
    page_id
}

fn add_page_from_template(engine: &mut EditorEngine, template: PageTemplate) -> Option<NodeId> {
    let existing_pages = engine.get_pages();
    let (doc_w, doc_h) = engine.document_size();
    let (width, height) = template.dimensions_px(doc_w, doc_h);
    let (x, y) = next_page_origin(&existing_pages);
    let label = format!("{} {}", template.label(), existing_pages.len() + 1);
    let page_id = engine.add_page(x, y, width, height, Some(&label));
    if let Some(id) = page_id {
        engine.navigate_to_page(id);
    }
    page_id
}

fn selection_bounds_values(engine: &EditorEngine) -> (f64, f64, f64, f64) {
    engine
        .selection
        .bounding_box(&engine.document)
        .map(|bbox| (bbox.x0, bbox.y0, bbox.width(), bbox.height()))
        .unwrap_or((0.0, 0.0, 0.0, 0.0))
}

fn update_preferences_state(state: &mut PreferencesState, action: PreferencesAction) {
    match action {
        PreferencesAction::SetSection(section) => state.section = section,
        PreferencesAction::SetDefaultShapeTool(value) => state.default_shape_tool = value,
        PreferencesAction::SetBoundingBoxDisplay(value) => state.bounding_box_display = value,
        PreferencesAction::SetTheme(value) => state.theme = value,
        PreferencesAction::SetIconSize(value) => state.icon_size = value,
        PreferencesAction::SetShowTooltips(value) => state.show_tooltips = value,
        PreferencesAction::SetAutoSave(value) => state.auto_save = value,
        PreferencesAction::SetAutoSaveInterval(value) => state.auto_save_interval_secs = value,
        PreferencesAction::SetRecentFilesCount(value) => state.recent_files_count = value,
        PreferencesAction::SetAntiAliasing(value) => state.anti_aliasing = value,
        PreferencesAction::SetDisplayFilterQuality(value) => state.display_filter_quality = value,
        PreferencesAction::SetCanvasBorder(value) => state.canvas_border = value,
        PreferencesAction::SetSnapToNodes(value) => state.snap_to_nodes = value,
        PreferencesAction::SetSnapToPaths(value) => state.snap_to_paths = value,
        PreferencesAction::SetSnapToGrid(value) => state.snap_to_grid = value,
        PreferencesAction::SetSnapToGuides(value) => state.snap_to_guides = value,
        PreferencesAction::SetSnapToObjects(value) => state.snap_to_objects = value,
        PreferencesAction::SetSnapToIntersections(value) => state.snap_to_intersections = value,
        PreferencesAction::SetSnapToEqualSpacing(value) => state.snap_to_equal_spacing = value,
        PreferencesAction::SetSnapFromCursor(value) => state.snap_from_cursor = value,
        PreferencesAction::SetSnapFromObjectNodes(value) => state.snap_from_object_nodes = value,
        PreferencesAction::SetSnapFromBBoxCorners(value) => state.snap_from_bbox_corners = value,
        PreferencesAction::SetSnapFromBBoxCenters(value) => state.snap_from_bbox_centers = value,
        PreferencesAction::SetSnapFromBBoxMidpoints(value) => {
            state.snap_from_bbox_midpoints = value
        }
        PreferencesAction::SetSnapDistance(value) => state.snap_distance = value,
        PreferencesAction::Close => {}
    }
}

fn tool_label(tool: ToolId) -> &'static str {
    match tool {
        ToolId::Select => "Select",
        ToolId::Node => "Nodes",
        ToolId::Rect => "Rectangle",
        ToolId::Ellipse => "Ellipse",
        ToolId::Star => "Star",
        ToolId::Spiral => "Spiral",
        ToolId::Pen => "Pen",
        ToolId::Pencil => "Pencil",
        ToolId::Calligraphy => "Calligraphy",
        ToolId::Text => "Text",
        ToolId::Gradient => "Gradient",
        ToolId::Dropper => "Dropper",
        ToolId::BucketFill => "Bucket Fill",
        ToolId::Connector => "Connector",
        ToolId::Eraser => "Eraser",
        ToolId::Spray => "Spray",
        ToolId::Tweak => "Tweak",
        ToolId::Measure => "Measure",
        ToolId::Zoom => "Zoom",
        ToolId::Mesh => "Mesh",
        ToolId::Box3d => "3D Box",
        ToolId::Marker => "Marker",
        ToolId::Pages => "Pages",
        ToolId::Lpe => "Live Path",
    }
}

/// Parse a menu item's shortcut hint into a real accelerator.
///
/// The hint is what the menu displays, e.g. `"Ctrl+Shift+S"`. Passing `None`
/// to the menu item leaves the text showing a shortcut the window never
/// actually listens for.
fn parse_accelerator(hint: &str) -> Option<(AcceleratorModifiers, Code)> {
    let mut modifiers = AcceleratorModifiers::empty();
    let mut key: Option<Code> = None;

    // `+` is both the separator and a key name, so a hint ending in `+` names
    // the plus key: "Ctrl++" is Ctrl plus `+`, not a trailing empty segment.
    let (hint, trailing_plus) = match hint.strip_suffix('+') {
        Some(rest) => (rest.trim_end_matches('+'), true),
        None => (hint, false),
    };
    if trailing_plus {
        key = Some(Code::Equal);
    }

    for part in hint.split('+') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "cmd" | "command" => {
                // The native host maps the platform command modifier.
                modifiers |= AcceleratorModifiers::CONTROL;
            }
            "shift" => modifiers |= AcceleratorModifiers::SHIFT,
            "alt" | "option" => modifiers |= AcceleratorModifiers::ALT,
            "super" | "meta" | "win" => modifiers |= AcceleratorModifiers::META,
            other => key = code_for(other),
        }
    }

    key.map(|code| (modifiers, code))
}

/// Map the key part of a shortcut hint onto a physical key code.
fn code_for(name: &str) -> Option<Code> {
    Some(match name {
        "a" => Code::KeyA,
        "b" => Code::KeyB,
        "c" => Code::KeyC,
        "d" => Code::KeyD,
        "e" => Code::KeyE,
        "f" => Code::KeyF,
        "g" => Code::KeyG,
        "h" => Code::KeyH,
        "i" => Code::KeyI,
        "j" => Code::KeyJ,
        "k" => Code::KeyK,
        "l" => Code::KeyL,
        "m" => Code::KeyM,
        "n" => Code::KeyN,
        "o" => Code::KeyO,
        "p" => Code::KeyP,
        "q" => Code::KeyQ,
        "r" => Code::KeyR,
        "s" => Code::KeyS,
        "t" => Code::KeyT,
        "u" => Code::KeyU,
        "v" => Code::KeyV,
        "w" => Code::KeyW,
        "x" => Code::KeyX,
        "y" => Code::KeyY,
        "z" => Code::KeyZ,
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "delete" | "del" => Code::Delete,
        "backspace" => Code::Backspace,
        "escape" | "esc" => Code::Escape,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "space" => Code::Space,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        "pgup" | "pageup" => Code::PageUp,
        "pgdn" | "pagedown" => Code::PageDown,
        "home" => Code::Home,
        "end" => Code::End,
        "f11" => Code::F11,
        // Shifted punctuation is reported by its physical key, so these map to
        // the digit or symbol key the character sits on.
        "+" | "plus" | "=" => Code::Equal,
        "-" | "minus" | "_" => Code::Minus,
        "!" => Code::Digit1,
        "@" => Code::Digit2,
        "#" => Code::Digit3,
        "$" => Code::Digit4,
        "%" => Code::Digit5,
        "^" => Code::Digit6,
        "&" => Code::Digit7,
        "*" => Code::Digit8,
        "(" => Code::Digit9,
        ")" => Code::Digit0,
        "/" | "?" => Code::Slash,
        "\\" | "|" => Code::Backslash,
        "," | "<" => Code::Comma,
        "." | ">" => Code::Period,
        "[" | "{" => Code::BracketLeft,
        "]" | "}" => Code::BracketRight,
        ";" | ":" => Code::Semicolon,
        "'" | "\"" => Code::Quote,
        "`" | "~" => Code::Backquote,
        _ => return None,
    })
}

/// Build the layer entry list for the layers panel.
fn build_layer_entries(engine: &EditorEngine) -> Vec<LayerEntry> {
    let mut layer_entries = Vec::new();
    if let Some(root) = engine.document.root() {
        for &child in engine.document.children(root) {
            push_layer_entries(engine, child, 0, &mut layer_entries);
        }
    }
    if !layer_entries.is_empty() {
        return layer_entries;
    }

    let doc = &engine.document;
    let sel = &engine.selection;
    let Some(root) = doc.root() else {
        return Vec::new();
    };

    doc.children(root)
        .iter()
        .rev()
        .filter_map(|&child_id| {
            let node = doc.get_node(child_id)?;
            if !object_tree_visible(&node.element) {
                return None;
            }
            let label = if let Some(ref xml_id) = node.xml_id {
                format!("<{}> #{}", node.element.tag_name(), xml_id)
            } else {
                format!("<{}>", node.element.tag_name())
            };
            Some(LayerEntry {
                id: child_id,
                label,
                depth: 0,
                selected: sel.is_selected(child_id),
                active: false,
                visible: !node.hidden,
                locked: node.locked,
                opacity: node.presentation.opacity.unwrap_or(1.0),
                blend_mode: node
                    .presentation
                    .mix_blend_mode
                    .unwrap_or_default()
                    .to_svg()
                    .to_string(),
            })
        })
        .collect()
}

fn layer_count(engine: &EditorEngine) -> usize {
    let Some(root) = engine.document.root() else {
        return 0;
    };
    let mut count = 0;
    for &child in engine.document.children(root) {
        count_layers(engine, child, &mut count);
    }
    count
}

fn count_layers(engine: &EditorEngine, id: NodeId, count: &mut usize) {
    if is_layer_node(engine, id) {
        *count += 1;
    }
    for &child in engine.document.children(id) {
        count_layers(engine, child, count);
    }
}

fn push_layer_entries(
    engine: &EditorEngine,
    id: NodeId,
    depth: usize,
    entries: &mut Vec<LayerEntry>,
) {
    let Some(node) = engine.document.get_node(id) else {
        return;
    };

    let next_depth = if is_layer_node(engine, id) {
        let label = match &node.element {
            SvgElement::Group(attrs) => attrs
                .inkscape_label
                .clone()
                .unwrap_or_else(|| "Unnamed layer".to_string()),
            _ => "Unnamed layer".to_string(),
        };
        entries.push(LayerEntry {
            id,
            label,
            depth,
            selected: engine.selection.is_selected(id),
            active: engine.active_layer == Some(id),
            visible: !node.hidden,
            locked: node.locked,
            opacity: node.presentation.opacity.unwrap_or(1.0),
            blend_mode: node
                .presentation
                .mix_blend_mode
                .unwrap_or_default()
                .to_svg()
                .to_string(),
        });
        depth + 1
    } else {
        depth
    };

    for &child in engine.document.children(id) {
        push_layer_entries(engine, child, next_depth, entries);
    }
}

/// Resolve the primary selection using the same stylesheet and inheritance
/// rules as the renderer, including SVG's black fill / no stroke defaults.
fn selected_presentation(engine: &EditorEngine) -> PresentationAttrs {
    let Some(id) = engine.selection.primary() else {
        return PresentationAttrs::default();
    };
    let overrides = sivgo_style::stylesheet_overrides(&engine.document);
    let mut ancestors = Vec::new();
    let mut current = Some(id);
    while let Some(id) = current {
        let Some(node) = engine.document.get_node(id) else {
            break;
        };
        ancestors.push(id);
        current = node.parent;
    }
    ancestors
        .into_iter()
        .rev()
        .fold(PresentationAttrs::default(), |parent, id| {
            let own = &engine
                .document
                .get_node(id)
                .expect("existing ancestor")
                .presentation;
            overrides
                .get(&id)
                .map(|overlay| own.overridden_by(overlay))
                .unwrap_or_else(|| own.clone())
                .inheriting_from(&parent)
        })
}

fn paint_color_label(paint: &Paint, current_color: Option<Color>) -> (String, bool) {
    match paint {
        Paint::None => (String::new(), true),
        Paint::Color(color) => (color_to_hex(color), false),
        Paint::CurrentColor => (color_to_hex(&current_color.unwrap_or(Color::BLACK)), false),
        Paint::Url(reference) => (format!("url({reference})"), false),
    }
}

fn get_selected_fill(p: &PresentationAttrs) -> (String, bool) {
    paint_color_label(
        p.fill.as_ref().unwrap_or(&Paint::Color(Color::BLACK)),
        p.color,
    )
}

fn get_selected_stroke(p: &PresentationAttrs) -> (String, bool) {
    paint_color_label(p.stroke.as_ref().unwrap_or(&Paint::None), p.color)
}

fn get_selected_stroke_width(p: &PresentationAttrs) -> f64 {
    p.stroke_width.map(|width| width.value).unwrap_or(1.0)
}

/// Style snapshot of the primary selection for the Fill & Stroke panel.
struct SelectedStyleState {
    fill_paint_type: &'static str,
    fill_opacity: f64,
    stroke_paint_type: &'static str,
    stroke_opacity: f64,
    line_cap: &'static str,
    line_join: &'static str,
    dash_pattern: String,
    dash_offset: f64,
    marker_start: String,
    marker_mid: String,
    marker_end: String,
}

impl Default for SelectedStyleState {
    fn default() -> Self {
        SelectedStyleState {
            fill_paint_type: "Flat Color",
            fill_opacity: 1.0,
            stroke_paint_type: "Flat Color",
            stroke_opacity: 1.0,
            line_cap: "Butt",
            line_join: "Miter",
            dash_pattern: String::new(),
            dash_offset: 0.0,
            marker_start: String::new(),
            marker_mid: String::new(),
            marker_end: String::new(),
        }
    }
}

/// Resolve a paint to the matching Fill & Stroke panel paint-type label,
/// following `url(#...)` references to the paint server they point at.
fn paint_type_label(engine: &EditorEngine, paint: &Option<Paint>) -> &'static str {
    match paint {
        Some(Paint::None) => "None",
        Some(Paint::Url(reference)) => {
            let id = reference
                .trim_start_matches("url(")
                .trim_end_matches(')')
                .trim_matches('#');
            match engine
                .document
                .find_by_xml_id(id)
                .and_then(|node_id| engine.document.get_node(node_id))
                .map(|node| &node.element)
            {
                Some(SvgElement::LinearGradient(_)) => "Linear Gradient",
                Some(SvgElement::RadialGradient(_)) => "Radial Gradient",
                Some(SvgElement::Pattern(_)) => "Pattern",
                _ => "Flat Color",
            }
        }
        _ => "Flat Color",
    }
}

fn selected_style_state(engine: &EditorEngine, p: &PresentationAttrs) -> SelectedStyleState {
    let node = engine
        .selection
        .primary()
        .and_then(|id| engine.document.get_node(id));
    if node.is_none() {
        return SelectedStyleState::default();
    }
    SelectedStyleState {
        fill_paint_type: paint_type_label(engine, &p.fill),
        fill_opacity: p.fill_opacity.unwrap_or(1.0),
        stroke_paint_type: if p.stroke.is_none() {
            "None"
        } else {
            paint_type_label(engine, &p.stroke)
        },
        stroke_opacity: p.stroke_opacity.unwrap_or(1.0),
        line_cap: match p.stroke_linecap.unwrap_or_default() {
            LineCap::Butt => "Butt",
            LineCap::Round => "Round",
            LineCap::Square => "Square",
        },
        line_join: match p.stroke_linejoin.unwrap_or_default() {
            LineJoin::Miter => "Miter",
            LineJoin::Round => "Round",
            LineJoin::Bevel => "Bevel",
        },
        dash_pattern: p
            .stroke_dasharray
            .as_ref()
            .map(|dashes| {
                dashes
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default(),
        dash_offset: p.stroke_dashoffset.unwrap_or(0.0),
        marker_start: p.marker_start.clone().unwrap_or_default(),
        marker_mid: p.marker_mid.clone().unwrap_or_default(),
        marker_end: p.marker_end.clone().unwrap_or_default(),
    }
}

/// Apply an undoable presentation-attribute change to the primary selection.
/// Returns `true` if a command was executed.
fn modify_primary_presentation(
    engine: Signal<EditorEngine>,
    mutate: impl FnOnce(&mut PresentationAttrs),
) -> bool {
    let mut eng = engine.write();
    let Some(id) = eng.selection.primary() else {
        return false;
    };
    let Some(node) = eng.document.get_node(id) else {
        return false;
    };
    let old_attrs = node.presentation.clone();
    let mut new_attrs = old_attrs.clone();
    mutate(&mut new_attrs);
    let cmd = ModifyAttributeCommand::new(id, old_attrs, new_attrs);
    eng.execute(Box::new(cmd));
    true
}

/// Serialize the current document and write it to `path`.
fn save_svg_document(
    engine: Signal<EditorEngine>,
    path: &std::path::Path,
    mark_saved: bool,
) -> bool {
    let xml = {
        let eng = engine.read();
        sivgo_dom::serialize_svg(&eng.document)
    };
    match fs::write(path, xml.as_bytes()) {
        Ok(()) => {
            if mark_saved {
                engine.write().mark_saved();
            }
            true
        }
        Err(e) => {
            report_error(
                "Could not save the file",
                &format!("{}: {e}", path.display()),
            );
            false
        }
    }
}

/// Show a failure to the user instead of only writing it to the log.
///
/// A dialog that never appears is indistinguishable from the operation having
/// worked: opening a corrupt file, saving to a read-only path and a failed
/// export all used to leave the interface completely unchanged.
fn report_error(title: &str, detail: &str) {
    log::error!("{title}: {detail}");
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(title)
        .set_description(detail)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// Ask whether to discard unsaved changes.
///
/// Returns `true` when it is safe to continue. A document with no unsaved
/// changes never prompts.
fn confirm_discard_changes(engine: Signal<EditorEngine>, what: &str) -> bool {
    if !engine.read().is_dirty() {
        return true;
    }
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title("Unsaved changes")
        .set_description(format!("This drawing has unsaved changes. {what} anyway?"))
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        == rfd::MessageDialogResult::Yes
}

/// Empty or whitespace-only marker references clear the marker.
fn normalize_marker_ref(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Parse a dash pattern like "5,3,2" or "5 3 2" into lengths.
fn parse_dash_pattern(value: &str) -> Option<Vec<f64>> {
    let dashes: Vec<f64> = value
        .split([',', ' '])
        .filter(|token| !token.trim().is_empty())
        .filter_map(|token| token.trim().parse::<f64>().ok())
        .filter(|v| *v >= 0.0)
        .collect();
    (!dashes.is_empty()).then_some(dashes)
}

fn engine_mouse_event_from_viewport_coords(
    x: f64,
    y: f64,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> sivgo_core::MouseEvent {
    sivgo_core::MouseEvent {
        x,
        y,
        button: sivgo_core::MouseButton::Left,
        modifiers: sivgo_core::Modifiers { ctrl, shift, alt },
    }
}

fn is_layer_node(engine: &EditorEngine, id: NodeId) -> bool {
    engine
        .document
        .get_node(id)
        .map(|node| {
            matches!(
                &node.element,
                SvgElement::Group(attrs)
                    if attrs.inkscape_groupmode.as_deref() == Some("layer")
            )
        })
        .unwrap_or(false)
}

fn current_layer_id(engine: &EditorEngine) -> Option<NodeId> {
    engine
        .active_layer
        .filter(|id| is_layer_node(engine, *id))
        .or_else(|| {
            engine
                .selection
                .primary()
                .filter(|id| is_layer_node(engine, *id))
        })
        .or_else(|| engine.get_layers().last().map(|(id, _)| *id))
}

/// Translate a native key press into the editor's key type.
///
/// Returns `None` for keys the editor has no binding vocabulary for, which the
/// caller passes through to the active tool instead.
fn to_editor_key(key: &Key) -> Option<sivgo_core::Key> {
    use sivgo_core::Key as EditorKey;
    Some(match key {
        Key::Named(keyboard_types::NamedKey::Enter) => EditorKey::Enter,
        Key::Named(keyboard_types::NamedKey::Escape) => EditorKey::Escape,
        Key::Named(keyboard_types::NamedKey::Tab) => EditorKey::Tab,
        Key::Named(keyboard_types::NamedKey::Delete) => EditorKey::Delete,
        Key::Named(keyboard_types::NamedKey::Backspace) => EditorKey::Backspace,
        Key::Named(keyboard_types::NamedKey::ArrowUp) => EditorKey::ArrowUp,
        Key::Named(keyboard_types::NamedKey::ArrowDown) => EditorKey::ArrowDown,
        Key::Named(keyboard_types::NamedKey::ArrowLeft) => EditorKey::ArrowLeft,
        Key::Named(keyboard_types::NamedKey::ArrowRight) => EditorKey::ArrowRight,
        Key::Named(keyboard_types::NamedKey::PageUp) => EditorKey::PageUp,
        Key::Named(keyboard_types::NamedKey::PageDown) => EditorKey::PageDown,
        Key::Named(keyboard_types::NamedKey::Home) => EditorKey::Home,
        Key::Named(keyboard_types::NamedKey::End) => EditorKey::End,
        Key::Named(keyboard_types::NamedKey::F1) => EditorKey::F1,
        Key::Named(keyboard_types::NamedKey::F2) => EditorKey::F2,
        Key::Named(keyboard_types::NamedKey::F3) => EditorKey::F3,
        Key::Named(keyboard_types::NamedKey::F4) => EditorKey::F4,
        Key::Named(keyboard_types::NamedKey::F5) => EditorKey::F5,
        Key::Named(keyboard_types::NamedKey::F6) => EditorKey::F6,
        Key::Named(keyboard_types::NamedKey::F7) => EditorKey::F7,
        Key::Named(keyboard_types::NamedKey::F8) => EditorKey::F8,
        Key::Named(keyboard_types::NamedKey::F9) => EditorKey::F9,
        Key::Named(keyboard_types::NamedKey::F10) => EditorKey::F10,
        Key::Named(keyboard_types::NamedKey::F11) => EditorKey::F11,
        Key::Named(keyboard_types::NamedKey::F12) => EditorKey::F12,
        Key::Character(text) => {
            let mut chars = text.chars();
            let ch = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            // The binding table uses the named letter and digit variants, so a
            // character has to be translated into one of those to match.
            // `Char` is the fallback for punctuation.
            match ch.to_ascii_lowercase() {
                'a' => EditorKey::A,
                'b' => EditorKey::B,
                'c' => EditorKey::C,
                'd' => EditorKey::D,
                'e' => EditorKey::E,
                'f' => EditorKey::F,
                'g' => EditorKey::G,
                'h' => EditorKey::H,
                'i' => EditorKey::I,
                'j' => EditorKey::J,
                'k' => EditorKey::K,
                'l' => EditorKey::L,
                'm' => EditorKey::M,
                'n' => EditorKey::N,
                'o' => EditorKey::O,
                'p' => EditorKey::P,
                'q' => EditorKey::Q,
                'r' => EditorKey::R,
                's' => EditorKey::S,
                't' => EditorKey::T,
                'u' => EditorKey::U,
                'v' => EditorKey::V,
                'w' => EditorKey::W,
                'x' => EditorKey::X,
                'y' => EditorKey::Y,
                'z' => EditorKey::Z,
                '0' => EditorKey::Digit0,
                '1' => EditorKey::Digit1,
                '2' => EditorKey::Digit2,
                '3' => EditorKey::Digit3,
                '4' => EditorKey::Digit4,
                '5' => EditorKey::Digit5,
                '6' => EditorKey::Digit6,
                '7' => EditorKey::Digit7,
                '8' => EditorKey::Digit8,
                '9' => EditorKey::Digit9,
                '+' | '=' => EditorKey::Plus,
                '-' => EditorKey::Minus,
                ' ' => EditorKey::Space,
                other => EditorKey::Char(other),
            }
        }
        _ => return None,
    })
}

/// The menu action a keyboard shortcut maps onto.
///
/// Shortcuts and menu items trigger the same work, so the binding table is
/// resolved to the menu's action id and dispatched through the one handler
/// rather than duplicating the logic. `None` means the action has no menu
/// equivalent yet.
fn menu_action_for_shortcut(action: &sivgo_core::Action) -> Option<&'static str> {
    use sivgo_core::Action as A;
    Some(match action {
        A::NewDocument => "file.new",
        A::Open => "file.open",
        A::Save => "file.save",
        A::SaveAs => "file.save_as",
        A::Export => "file.export_png",
        A::Quit => "file.quit",

        A::Undo => "edit.undo",
        A::Redo => "edit.redo",
        A::Cut => "edit.cut",
        A::Copy => "edit.copy",
        A::Paste => "edit.paste",
        A::PasteInPlace => "edit.paste_in_place",
        A::PasteStyle => "edit.paste_style",
        A::Duplicate => "edit.duplicate",
        A::Clone => "edit.clone",
        A::Delete => "edit.delete",
        A::SelectAll => "edit.select_all",
        A::SelectNone => "edit.select_none",

        A::ZoomIn => "view.zoom_in",
        A::ZoomOut => "view.zoom_out",
        A::Zoom100 => "view.zoom_100",
        A::ZoomFitPage => "view.zoom_fit_page",
        A::ZoomFitSelection => "view.zoom_selection",
        A::ZoomFitDrawing => "view.zoom_fit",
        A::ToggleGrid => "view.toggle_grid",
        A::ToggleGuides => "view.toggle_guides",
        A::ToggleFullscreen => "view.fullscreen",

        A::Group => "object.group",
        A::Ungroup => "object.ungroup",
        A::Raise => "object.raise",
        A::Lower => "object.lower",
        A::RaiseToTop => "object.to_front",
        A::LowerToBottom => "object.to_back",
        A::FlipHorizontal => "object.flip_h",
        A::FlipVertical => "object.flip_v",

        A::ObjectToPath => "path.object_to_path",
        A::SimplifyPath => "path.simplify",
        A::BoolUnion => "path.union",
        A::BoolDifference => "path.difference",
        A::BoolIntersection => "path.intersection",
        A::BoolExclusion => "path.exclusion",

        // Handled elsewhere or not wired to a menu entry.
        _ => return None,
    })
}

/// Insert the contents of `path` into the open document.
///
/// SVG files are merged element by element. A bitmap is either embedded as an
/// `<image>` or, when `trace` is set, converted to paths first. None of this
/// was reachable before: the Import menu item had no handler, so the bitmap
/// tracer and the bitmap importer had no way in at all.
fn import_file_into_document(engine: Signal<EditorEngine>, path: &Path, trace: bool) {
    let Ok(bytes) = std::fs::read(path) else {
        log::error!("Failed to read {}", path.display());
        return;
    };

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if extension == "svg" {
        match String::from_utf8(bytes)
            .ok()
            .and_then(|text| sivgo_dom::parse_svg(&text).ok())
        {
            Some(imported) => {
                let count = engine.write().import_document(&imported);
                info!("Imported {count} elements from {}", path.display());
            }
            None => log::error!("Failed to parse {}", path.display()),
        }
        return;
    }

    if trace {
        match sivgo_io::trace_bitmap_file(&bytes, &sivgo_io::TraceOptions::default()) {
            Ok(traced) => {
                let count = engine.write().import_document(&traced);
                info!("Traced {} into {count} paths", path.display());
            }
            Err(e) => log::error!("Failed to trace {}: {e}", path.display()),
        }
        return;
    }

    let mime = match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "image/png",
    };
    let mut eng = engine.write();
    match sivgo_io::import_bitmap(&mut eng.document, &bytes, mime) {
        Ok(_) => info!("Imported bitmap {}", path.display()),
        Err(e) => log::error!("Failed to import bitmap: {e}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_editor_action(
    action: &str,
    engine: Signal<EditorEngine>,
    render_version: Signal<u64>,
    file_path: Signal<Option<PathBuf>>,
    is_fullscreen: Signal<bool>,
    inspector_panel: Signal<InspectorPanel>,
    document_defaults: Signal<DocumentDefaults>,
    preferences_state: Signal<PreferencesState>,
    desktop: DesktopContext,
) {
    match action {
        "file.new" => {
            if !confirm_discard_changes(engine, "Start a new drawing") {
                return;
            }
            let defaults = document_defaults.read().clone();
            let preferences = preferences_state.read().clone();
            let mut eng = engine.write();
            eng.new_document(defaults.width_px, defaults.height_px);
            apply_document_defaults_to_engine(&mut eng, &defaults);
            apply_preferences_to_engine(&mut eng, &preferences);
            eng.mark_saved();
            drop(eng);
            file_path.set(None);
            set_window_title(&desktop, None);
            bump_render(render_version);
        }
        "file.open" => {
            if !confirm_discard_changes(engine, "Open another file") {
                return;
            }
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SVG files", &["svg"])
                .pick_file()
            {
                match fs::read_to_string(&path) {
                    Ok(content) => match sivgo_dom::parse_svg(&content) {
                        Ok(doc) => {
                            let preferences = preferences_state.read().clone();
                            let mut eng = engine.write();
                            eng.load_document(doc);
                            apply_preferences_to_engine(&mut eng, &preferences);
                            // A freshly loaded document has no unsaved edits.
                            eng.mark_saved();
                            drop(eng);
                            file_path.set(Some(path.clone()));
                            set_window_title(&desktop, Some(&path));
                            bump_render(render_version);
                        }
                        Err(e) => report_error(
                            "Could not open the file",
                            &format!("{} is not valid SVG: {e}", path.display()),
                        ),
                    },
                    Err(e) => report_error(
                        "Could not read the file",
                        &format!("{}: {e}", path.display()),
                    ),
                }
            }
        }
        "file.save" => {
            let existing_path = file_path.read().clone();
            let path = if let Some(path) = existing_path {
                path
            } else {
                match rfd::FileDialog::new()
                    .add_filter("SVG files", &["svg"])
                    .save_file()
                {
                    Some(path) => path,
                    None => return,
                }
            };

            if save_svg_document(engine, &path, true) {
                file_path.set(Some(path.clone()));
                set_window_title(&desktop, Some(&path));
                info!("Saved to {}", path.display());
            }
        }
        "file.save_as" => {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SVG files", &["svg"])
                .save_file()
            {
                if save_svg_document(engine, &path, true) {
                    file_path.set(Some(path.clone()));
                    set_window_title(&desktop, Some(&path));
                    info!("Saved as {}", path.display());
                }
            }
        }
        "file.import" => {
            let Some(path) = rfd::FileDialog::new()
                .add_filter(
                    "Images and drawings",
                    &["svg", "png", "jpg", "jpeg", "gif", "bmp"],
                )
                .pick_file()
            else {
                return;
            };
            import_file_into_document(engine, &path, false);
            bump_render(render_version);
        }
        "file.import_traced" => {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Bitmap images", &["png", "jpg", "jpeg", "gif", "bmp"])
                .pick_file()
            else {
                return;
            };
            import_file_into_document(engine, &path, true);
            bump_render(render_version);
        }
        "file.export_plain_svg" => {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SVG files", &["svg"])
                .save_file()
            {
                // Unlike a normal save, this strips the editor-specific
                // `inkscape:` and `sodipodi:` markup.
                let plain = sivgo_io::export_plain_svg(&engine.read().document);
                match std::fs::write(&path, plain) {
                    Ok(()) => info!("Wrote plain SVG to {}", path.display()),
                    Err(e) => log::error!("Failed to write plain SVG: {e}"),
                }
            }
        }
        "file.save_copy" | "file.export_svg" => {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SVG files", &["svg"])
                .save_file()
            {
                if save_svg_document(engine, &path, false) {
                    info!("Wrote SVG copy to {}", path.display());
                }
            }
        }
        "file.export_png" => {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG images", &["png"])
                .save_file()
            {
                let eng = engine.read();
                let (width, height) = document_pixel_size(&eng);
                if let Err(e) = sivgo_io::export_png(&eng.document, &path, width, height) {
                    log::error!("Failed to export PNG: {}", e);
                } else {
                    info!("Exported PNG to {}", path.display());
                }
            }
        }
        "file.export_pages_svg" => {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                let result = {
                    let eng = engine.read();
                    export_pages_svg_to_folder(&eng, &folder)
                };
                match result {
                    Ok(count) => info!("Exported {count} SVG page(s) to {}", folder.display()),
                    Err(err) => log::error!("{err}"),
                }
            }
        }
        "file.export_pages_pdf" => {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PDF documents", &["pdf"])
                .save_file()
            {
                let result = {
                    let eng = engine.read();
                    export_pages_pdf(&eng, &path)
                };
                match result {
                    Ok(count) => info!("Exported {count} page(s) to {}", path.display()),
                    Err(err) => log::error!("{err}"),
                }
            }
        }
        "file.print" => {
            if let Err(error) = print_document(&engine.read()) {
                report_error("Could not print", &error);
            }
        }
        "file.close" | "file.quit" => {
            if confirm_discard_changes(engine, "Close") {
                desktop.close();
            }
        }
        "file.document_properties" => inspector_panel.set(InspectorPanel::Document),
        "edit.undo" => {
            engine.write().undo();
            bump_render(render_version);
        }
        "edit.redo" => {
            engine.write().redo();
            bump_render(render_version);
        }
        "edit.cut" => {
            let (svg, plain_text) = {
                let mut eng = engine.write();
                let plain_text = eng.selected_plain_text();
                eng.cut();
                (eng.clipboard_svg_text(), plain_text)
            };
            if let Some(text) = plain_text {
                copy_text_to_native_clipboard(&text);
            } else if let Some(svg) = svg {
                copy_svg_to_native_clipboard(&svg);
            }
            bump_render(render_version);
        }
        "edit.copy" => {
            let (svg, plain_text) = {
                let mut eng = engine.write();
                eng.copy();
                (eng.clipboard_svg_text(), eng.selected_plain_text())
            };
            if let Some(text) = plain_text {
                copy_text_to_native_clipboard(&text);
            } else if let Some(svg) = svg {
                copy_svg_to_native_clipboard(&svg);
            }
        }
        "edit.copy_bitmap" => {
            let result = {
                let eng = engine.read();
                copy_editor_bitmap_to_native_clipboard(&eng)
            };
            if let Err(err) = result {
                log::warn!("Failed to copy bitmap to clipboard: {err}");
            }
        }
        "edit.paste" => {
            match read_preferred_clipboard_payload() {
                Some(ClipboardPayload::Svg(svg)) => {
                    engine.write().set_clipboard_svg_text(svg);
                    engine.write().paste();
                }
                Some(ClipboardPayload::Bitmap {
                    pixels,
                    width,
                    height,
                }) => {
                    let mut eng = engine.write();
                    if let Err(err) = paste_bitmap_into_document(&mut eng, &pixels, width, height) {
                        log::warn!("Failed to paste clipboard bitmap: {err}");
                    }
                }
                Some(ClipboardPayload::Text(text)) => {
                    let mut eng = engine.write();
                    paste_text_into_document(&mut eng, &text);
                }
                None => {
                    engine.write().paste();
                }
            }
            bump_render(render_version);
        }
        "edit.paste_in_place" => {
            if let Some(svg) = read_svg_from_native_clipboard() {
                engine.write().set_clipboard_svg_text(svg);
            }
            engine.write().paste_in_place();
            bump_render(render_version);
        }
        "edit.paste_style" => {
            if let Some(svg) = read_svg_from_native_clipboard() {
                engine.write().set_clipboard_svg_text(svg);
            }
            engine.write().paste_style();
            bump_render(render_version);
        }
        "edit.delete" => {
            engine.write().delete_selected();
            bump_render(render_version);
        }
        "edit.select_all" => {
            engine.write().select_all();
            bump_render(render_version);
        }
        "edit.select_all_layers" => {
            engine.write().select_all_in_all_layers();
            bump_render(render_version);
        }
        "edit.invert_selection" => {
            engine.write().invert_selection();
            bump_render(render_version);
        }
        "edit.select_none" => {
            engine.write().select_none();
            bump_render(render_version);
        }
        "edit.duplicate" => {
            engine.write().duplicate_selected();
            bump_render(render_version);
        }
        "edit.clone" => {
            engine.write().clone_selected();
            bump_render(render_version);
        }
        "edit.unlink_clone" => {
            engine.write().unlink_selected_clones();
            bump_render(render_version);
        }
        "edit.preferences" => inspector_panel.set(InspectorPanel::Preferences),
        "view.zoom_in" => {
            let mut eng = engine.write();
            let center =
                sivgo_math::Point::new(eng.viewport.width / 2.0, eng.viewport.height / 2.0);
            let target_zoom = eng.viewport.zoom * 1.15;
            set_viewport_zoom_at_screen_point(&mut eng, target_zoom, center);
            drop(eng);
            bump_render(render_version);
        }
        "view.zoom_out" => {
            let mut eng = engine.write();
            let center =
                sivgo_math::Point::new(eng.viewport.width / 2.0, eng.viewport.height / 2.0);
            let target_zoom = eng.viewport.zoom / 1.15;
            set_viewport_zoom_at_screen_point(&mut eng, target_zoom, center);
            drop(eng);
            bump_render(render_version);
        }
        "view.zoom_100" => {
            let mut eng = engine.write();
            let center =
                sivgo_math::Point::new(eng.viewport.width / 2.0, eng.viewport.height / 2.0);
            set_viewport_zoom_at_screen_point(&mut eng, 1.0, center);
            drop(eng);
            bump_render(render_version);
        }
        "view.zoom_50" => {
            let mut eng = engine.write();
            let center =
                sivgo_math::Point::new(eng.viewport.width / 2.0, eng.viewport.height / 2.0);
            set_viewport_zoom_at_screen_point(&mut eng, 0.5, center);
            drop(eng);
            bump_render(render_version);
        }
        "view.zoom_fit" | "view.zoom_fit_page" => {
            let mut eng = engine.write();
            let (width, height) = eng.document_size();
            eng.viewport
                .fit_rect(sivgo_math::Rect::new(0.0, 0.0, width, height));
            drop(eng);
            bump_render(render_version);
        }
        "view.zoom_selection" => {
            let mut eng = engine.write();
            if let Some(bounds) = eng.selection.bounding_box(&eng.document) {
                eng.viewport.fit_rect(bounds);
                drop(eng);
                bump_render(render_version);
            }
        }
        "view.toggle_grid" => {
            let mut eng = engine.write();
            let next = !grid_visible(&eng);
            eng.set_grid_visible(next);
            drop(eng);
            bump_render(render_version);
        }
        "view.toggle_guides" => {
            let next = {
                let mut eng = engine.write();
                eng.snapping.prefs.snap_to_guides = !eng.snapping.prefs.snap_to_guides;
                eng.snapping.prefs.snap_to_guides
            };
            preferences_state.write().snap_to_guides = next;
            let preferences_snapshot = preferences_state.read().clone();
            let defaults_snapshot = document_defaults.read().clone();
            persist_desktop_settings(&preferences_snapshot, &defaults_snapshot);
            bump_render(render_version);
        }
        "view.toggle_snapping" => {
            let mut eng = engine.write();
            let next = !eng.snapping.prefs.any_enabled();
            eng.snapping.prefs.snap_to_grid = next;
            eng.snapping.prefs.snap_to_guides = next;
            eng.snapping.prefs.snap_to_bbox_edges = next;
            eng.snapping.prefs.snap_to_bbox_centers = next;
            eng.snapping.prefs.snap_to_bbox_corners = next;
            eng.snapping.prefs.snap_to_path_nodes = next;
            eng.snapping.prefs.snap_to_path = next;
            eng.snapping.prefs.snap_to_intersections = next;
            eng.snapping.prefs.snap_to_midpoints = next;
            eng.snapping.prefs.snap_to_equal_spacing = next;
            eng.snapping.prefs.sources.snap_from_cursor = next;
            eng.snapping.prefs.sources.snap_from_object_nodes = next;
            eng.snapping.prefs.sources.snap_from_bbox_corners = next;
            eng.snapping.prefs.sources.snap_from_bbox_centers = next;
            eng.snapping.prefs.sources.snap_from_bbox_midpoints = next;
            drop(eng);
            {
                let mut prefs = preferences_state.write();
                prefs.snap_to_grid = next;
                prefs.snap_to_guides = next;
                prefs.snap_to_objects = next;
                prefs.snap_to_nodes = next;
                prefs.snap_to_paths = next;
                prefs.snap_to_intersections = next;
                prefs.snap_to_equal_spacing = next;
                prefs.snap_from_cursor = next;
                prefs.snap_from_object_nodes = next;
                prefs.snap_from_bbox_corners = next;
                prefs.snap_from_bbox_centers = next;
                prefs.snap_from_bbox_midpoints = next;
            }
            let preferences_snapshot = preferences_state.read().clone();
            let defaults_snapshot = document_defaults.read().clone();
            persist_desktop_settings(&preferences_snapshot, &defaults_snapshot);
            bump_render(render_version);
        }
        "view.display_mode_normal" => {
            engine.write().set_display_mode(DisplayMode::Normal);
            bump_render(render_version);
        }
        "view.display_mode_outline" => {
            engine.write().set_display_mode(DisplayMode::Outline);
            bump_render(render_version);
        }
        "view.display_mode_no_filters" => {
            engine.write().set_display_mode(DisplayMode::NoFilters);
            bump_render(render_version);
        }
        "view.display_mode_hairlines" => {
            engine
                .write()
                .set_display_mode(DisplayMode::VisibleHairlines);
            bump_render(render_version);
        }
        "view.display_mode_print" => {
            engine.write().set_display_mode(DisplayMode::PrintPreview);
            bump_render(render_version);
        }
        "view.display_mode_proof" => {
            engine.write().set_display_mode(DisplayMode::ProofColors);
            bump_render(render_version);
        }
        "view.fullscreen" => {
            let next = !*is_fullscreen.read();
            is_fullscreen.set(next);
            desktop.set_fullscreen(next);
        }
        "filter.editor" => {
            inspector_panel.set(InspectorPanel::Filters);
        }
        "filter.blur" => {
            engine.write().apply_filter_preset_to_selected("blur");
            inspector_panel.set(InspectorPanel::Filters);
            bump_render(render_version);
        }
        "filter.drop_shadow" => {
            engine
                .write()
                .apply_filter_preset_to_selected("drop_shadow");
            inspector_panel.set(InspectorPanel::Filters);
            bump_render(render_version);
        }
        "object.group" => {
            engine.write().group_selected();
            bump_render(render_version);
        }
        "object.ungroup" => {
            engine.write().ungroup_selected();
            bump_render(render_version);
        }
        "object.raise" => {
            engine.write().raise_selected();
            bump_render(render_version);
        }
        "object.lower" => {
            engine.write().lower_selected();
            bump_render(render_version);
        }
        "object.to_front" => {
            let steps = {
                let eng = engine.read();
                eng.document
                    .root()
                    .map(|root| eng.document.children(root).len())
                    .unwrap_or(0)
            };
            let mut eng = engine.write();
            for _ in 0..steps {
                eng.raise_selected();
            }
            drop(eng);
            bump_render(render_version);
        }
        "object.to_back" => {
            let steps = {
                let eng = engine.read();
                eng.document
                    .root()
                    .map(|root| eng.document.children(root).len())
                    .unwrap_or(0)
            };
            let mut eng = engine.write();
            for _ in 0..steps {
                eng.lower_selected();
            }
            drop(eng);
            bump_render(render_version);
        }
        "object.flip_h" => {
            engine.write().flip_horizontal();
            bump_render(render_version);
        }
        "object.flip_v" => {
            engine.write().flip_vertical();
            bump_render(render_version);
        }
        "object.rotate_90_cw" => {
            engine.write().rotate_selected(90.0);
            bump_render(render_version);
        }
        "object.rotate_90_ccw" => {
            engine.write().rotate_selected(-90.0);
            bump_render(render_version);
        }
        "object.align" => {
            inspector_panel.set(InspectorPanel::Align);
        }
        "object.transform" => {
            inspector_panel.set(InspectorPanel::Transform);
        }
        "object.clip_set" => {
            engine.write().set_clip_selected();
            bump_render(render_version);
        }
        "object.clip_release" => {
            engine.write().release_clip_selected();
            bump_render(render_version);
        }
        "object.mask_set" => {
            engine.write().set_mask_selected();
            bump_render(render_version);
        }
        "object.mask_release" => {
            engine.write().release_mask_selected();
            bump_render(render_version);
        }
        "object.object_properties" => {
            inspector_panel.set(InspectorPanel::Objects);
        }
        "path.object_to_path" => {
            engine.write().object_to_path_selected();
            bump_render(render_version);
        }
        "path.stroke_to_path" => {
            engine.write().stroke_to_path_selected();
            bump_render(render_version);
        }
        "text.to_path" => {
            engine.write().text_to_path_selected();
            bump_render(render_version);
        }
        "text.text_properties" => {
            inspector_panel.set(InspectorPanel::Text);
        }
        "text.font_size_inc" => {
            let mut eng = engine.write();
            if let Some(state) = eng.selected_text_state() {
                eng.set_selected_text_font_size(state.font_size + 1.0);
            }
            drop(eng);
            bump_render(render_version);
        }
        "text.font_size_dec" => {
            let mut eng = engine.write();
            if let Some(state) = eng.selected_text_state() {
                eng.set_selected_text_font_size((state.font_size - 1.0).max(1.0));
            }
            drop(eng);
            bump_render(render_version);
        }
        "text.bold" => {
            let mut eng = engine.write();
            if let Some(state) = eng.selected_text_state() {
                let next = if state.font_weight == "Bold" {
                    "Normal"
                } else {
                    "Bold"
                };
                eng.set_selected_text_font_weight(next);
            }
            drop(eng);
            bump_render(render_version);
        }
        "text.italic" => {
            let mut eng = engine.write();
            if let Some(state) = eng.selected_text_state() {
                let next = if state.font_style == "Italic" {
                    "Normal"
                } else {
                    "Italic"
                };
                eng.set_selected_text_font_style(next);
            }
            drop(eng);
            bump_render(render_version);
        }
        "path.union" => {
            engine.write().union_selected_paths();
            bump_render(render_version);
        }
        "path.difference" => {
            engine.write().difference_selected_paths();
            bump_render(render_version);
        }
        "path.intersection" => {
            engine.write().intersection_selected_paths();
            bump_render(render_version);
        }
        "path.exclusion" => {
            engine.write().exclusion_selected_paths();
            bump_render(render_version);
        }
        "path.division" => {
            engine.write().division_selected_paths();
            bump_render(render_version);
        }
        "path.cut_path" => {
            engine.write().cut_selected_paths();
            bump_render(render_version);
        }
        "path.combine" => {
            engine.write().combine_selected_paths();
            bump_render(render_version);
        }
        "path.break_apart" => {
            engine.write().break_apart_selected_paths();
            bump_render(render_version);
        }
        "path.simplify" => {
            engine.write().simplify_selected_paths(1.0);
            bump_render(render_version);
        }
        "path.reverse" => {
            engine.write().reverse_selected_paths();
            bump_render(render_version);
        }
        "path.inset" => {
            engine.write().offset_selected_paths(-2.0);
            bump_render(render_version);
        }
        "path.outset" => {
            engine.write().offset_selected_paths(2.0);
            bump_render(render_version);
        }
        "path.dynamic_offset" => {
            engine.write().dynamic_offset_selected_paths(2.0);
            bump_render(render_version);
        }
        "path.linked_offset" => {
            engine.write().linked_offset_selected_paths(2.0);
            bump_render(render_version);
        }
        "layer.add" => {
            let next_index = {
                let eng = engine.read();
                layer_count(&eng) + 1
            };
            engine
                .write()
                .add_sublayer(&format!("Layer {}", next_index));
            bump_render(render_version);
        }
        "layer.delete" => {
            let mut eng = engine.write();
            if let Some(id) = current_layer_id(&eng) {
                eng.delete_layer(id);
            }
            drop(eng);
            bump_render(render_version);
        }
        "layer.raise" => {
            let mut eng = engine.write();
            if let Some(id) = current_layer_id(&eng) {
                eng.raise_layer(id);
            }
            drop(eng);
            bump_render(render_version);
        }
        "layer.lower" => {
            let mut eng = engine.write();
            if let Some(id) = current_layer_id(&eng) {
                eng.lower_layer(id);
            }
            drop(eng);
            bump_render(render_version);
        }
        "layer.to_top" => {
            let steps = engine.read().get_layers().len();
            let mut eng = engine.write();
            if let Some(id) = current_layer_id(&eng) {
                for _ in 0..steps {
                    eng.raise_layer(id);
                }
            }
            drop(eng);
            bump_render(render_version);
        }
        "layer.to_bottom" => {
            let steps = engine.read().get_layers().len();
            let mut eng = engine.write();
            if let Some(id) = current_layer_id(&eng) {
                for _ in 0..steps {
                    eng.lower_layer(id);
                }
            }
            drop(eng);
            bump_render(render_version);
        }
        "layer.toggle_visible" => {
            let mut eng = engine.write();
            if let Some(id) = current_layer_id(&eng) {
                eng.toggle_visibility(id);
            }
            drop(eng);
            bump_render(render_version);
        }
        "layer.toggle_lock" => {
            let mut eng = engine.write();
            if let Some(id) = current_layer_id(&eng) {
                eng.toggle_lock(id);
            }
            drop(eng);
            bump_render(render_version);
        }
        "layer.move_selection_to_active" => {
            engine.write().move_selection_to_active_layer();
            bump_render(render_version);
        }
        "layer.merge_down" => {
            let mut eng = engine.write();
            if let Some(id) = current_layer_id(&eng) {
                eng.merge_layer_down(id);
            }
            drop(eng);
            bump_render(render_version);
        }
        "layer.flatten" => {
            engine.write().flatten_layers();
            bump_render(render_version);
        }
        "help.about" => {
            let _ = MessageDialog::new()
                .set_level(MessageLevel::Info)
                .set_title("About Sivgo")
                .set_description(
                    "Sivgo Studio\nA desktop SVG editor built with Rust, Glory, and Sapid.",
                )
                .set_buttons(MessageButtons::Ok)
                .show();
        }
        other => log::info!("Unhandled menu action: {}", other),
    }
}

// ---------------------------------------------------------------------------
// Main App component
// ---------------------------------------------------------------------------

/// Render the document itself, then delegate printing to the system PDF handler.
fn print_document(engine: &EditorEngine) -> Result<(), String> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let path = std::env::temp_dir().join(format!("sivgo-print-{}-{nonce}.pdf", std::process::id()));
    export_pages_pdf(engine, &path)?;
    #[cfg(target_os = "windows")]
    let status = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Process -FilePath $env:SIVGO_PRINT_FILE -Verb Print -ErrorAction Stop",
            ])
            .env("SIVGO_PRINT_FILE", &path)
            .creation_flags(0x08000000)
            .status()
    };
    #[cfg(not(target_os = "windows"))]
    let status = std::process::Command::new("lp").arg(&path).status();
    let status = status.map_err(|error| format!("Printing {} failed: {error}", path.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "The system PDF print handler failed ({status}). The PDF is available at {}.",
            path.display()
        ))
    }
}

#[cfg(test)]
#[test]
fn native_navigation_and_zoom_keys_reach_the_editor_vocabulary() {
    use keyboard_types::NamedKey;
    for (native, editor) in [
        (NamedKey::ArrowLeft, sivgo_core::Key::ArrowLeft),
        (NamedKey::PageUp, sivgo_core::Key::PageUp),
        (NamedKey::Home, sivgo_core::Key::Home),
        (NamedKey::F11, sivgo_core::Key::F11),
    ] {
        assert_eq!(to_editor_key(&Key::Named(native)), Some(editor));
    }
    assert_eq!(
        to_editor_key(&Key::Character("-".into())),
        Some(sivgo_core::Key::Minus)
    );
}
