//! `sivgo` -- Sivgo SVG vector graphics editor (desktop application).
//!
//! Supports two modes:
//!
//! * **GUI mode** (default, no arguments): launches the Dioxus desktop editor.
//! * **CLI mode** (`render` / `info` subcommands): batch operations.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process;

use dioxus::prelude::*;
use dioxus_desktop::{
    muda::{CheckMenuItem, Menu, MenuItem as NativeMenuItem, PredefinedMenuItem, Submenu},
    use_muda_event_handler, use_window, Config, DesktopContext, WindowBuilder,
};
use log::info;
use rfd::{MessageButtons, MessageDialog, MessageLevel};

use sivgo_core::{DisplayMode, EditorEngine, ModifyAttributeCommand, ToolId};
use sivgo_dom::{Color, Paint, SvgElement};
use sivgo_render::{DocumentRenderer, TinySkiaBackend};
use sivgo_ui::{
    layers_panel::LayerEntry,
    menu_model::{default_menu_bar_with_state, MenuItem as MenuNode},
    LayersPanel, PropertiesPanel, StatusBar, Toolbar,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Canvas dimensions in pixels.
const CANVAS_WIDTH: u32 = 800;
const CANVAS_HEIGHT: u32 = 600;

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
    gap: 2px;
    height: 38px;
    padding: 0 6px;
    background: var(--bg-1);
    border-bottom: 1px solid var(--panel-border);
    flex-shrink: 0;
}

.topbar-brand {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px 0 6px;
    margin-right: 4px;
    height: 100%;
    border-right: 1px solid var(--panel-border);
}

.brand-mark {
    width: 22px;
    height: 22px;
    border-radius: var(--radius-sm);
    background: linear-gradient(135deg, var(--accent), #7b6cf6);
    display: grid;
    place-items: center;
    color: #fff;
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.05em;
}

.brand-title {
    font-size: 12px;
    font-weight: 700;
    color: var(--ink-0);
    letter-spacing: 0.02em;
}

.topbar-actions {
    display: flex;
    align-items: center;
    gap: 1px;
}

.tb-btn {
    height: 28px;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    color: var(--ink-1);
    font-size: 11px;
    font-weight: 500;
    transition: background var(--transition), color var(--transition);
}
.tb-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.06);
    color: var(--ink-0);
}
.tb-btn:disabled {
    opacity: 0.35;
    cursor: default;
}
.tb-btn.active {
    background: var(--accent-soft);
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
    flex: 1;
}

.topbar-info {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 10px;
    color: var(--ink-2);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
}

.topbar-info span {
    white-space: nowrap;
}

.doc-name {
    color: var(--ink-1);
    font-weight: 600;
}

/* ------- Workspace Grid ------- */
.workspace {
    display: grid;
    grid-template-columns: 42px minmax(0, 1fr) 280px;
    flex: 1;
    min-height: 0;
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
    height: 32px;
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
    font-size: 15px;
    line-height: 1;
}

.tool-label,
.tool-shortcut,
.tool-meta {
    display: none; /* hidden in compact rail */
}

/* ------- Canvas Stage ------- */
.canvas-stage {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--canvas-bg);
    overflow: hidden;
}

.canvas-frame {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    /* Checkerboard transparency pattern */
    background-image:
        linear-gradient(45deg, #33333a 25%, transparent 25%),
        linear-gradient(-45deg, #33333a 25%, transparent 25%),
        linear-gradient(45deg, transparent 75%, #33333a 75%),
        linear-gradient(-45deg, transparent 75%, #33333a 75%);
    background-size: 16px 16px;
    background-position: 0 0, 0 8px, 8px -8px, -8px 0px;
}

.canvas-container {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
}

.canvas-container img {
    max-width: 100%;
    max-height: 100%;
    background: white;
    box-shadow: 0 2px 16px rgba(0, 0, 0, 0.3);
    cursor: crosshair;
    image-rendering: auto;
}

/* ------- Inspector (Right Panel) ------- */
.inspector-shell {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--bg-1);
    border-left: 1px solid var(--panel-border);
}

.right-panels {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
}

.properties-panel,
.layers-panel {
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
    text-transform: uppercase;
    letter-spacing: 0.06em;
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
    grid-template-columns: 60px minmax(0, 1fr) auto;
    margin-bottom: 6px;
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
}

.prop-color,
.prop-input,
.prop-hex-input,
.prop-select,
.layer-blend-select {
    height: 28px;
    width: 100%;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-0);
    font-size: 11px;
}
.prop-color {
    padding: 0;
    overflow: hidden;
    cursor: pointer;
}
.prop-input:focus,
.prop-hex-input:focus,
.prop-select:focus,
.layer-blend-select:focus {
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
    text-transform: uppercase;
    letter-spacing: 0.06em;
}

/* ------- Layers ------- */
.layers-list {
    padding: 4px;
}

.layer-item {
    display: grid;
    grid-template-columns: 26px 26px minmax(0, 1fr) 22px 22px;
    gap: 4px;
    padding: 6px 8px;
    margin-bottom: 1px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: transparent;
    align-items: center;
    transition: background var(--transition), border-color var(--transition);
}
.layer-item:last-child {
    margin-bottom: 0;
}
.layer-item:hover {
    background: rgba(255, 255, 255, 0.04);
}
.layer-item.selected {
    background: var(--accent-soft);
    border-color: var(--accent-border);
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

/* ------- Status Bar ------- */
.status-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 26px;
    padding: 0 10px;
    background: var(--bg-1);
    border-top: 1px solid var(--panel-border);
    flex-shrink: 0;
}

.status-section {
    display: flex;
    align-items: center;
    gap: 4px;
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
}
.status-cluster.emphasis {
    background: var(--accent-soft);
}

.status-label {
    color: var(--ink-2);
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
}

.status-item {
    color: var(--ink-1);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
}

/* ------- Responsive ------- */
@media (max-width: 1200px) {
    .workspace {
        grid-template-columns: 42px minmax(0, 1fr) 240px;
    }
}
"#;

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

fn launch_gui() {
    dioxus_desktop::launch::launch_virtual_dom(
        VirtualDom::new(App),
        Config::new()
            .with_background_color((9, 19, 24, 255))
            .with_menu(build_native_menu())
            .with_window(
                WindowBuilder::new()
                    .with_title("Sivgo Studio - Untitled.svg")
                    .with_inner_size(dioxus_desktop::LogicalSize::new(1280.0, 800.0))
                    .with_min_inner_size(dioxus_desktop::LogicalSize::new(1120.0, 720.0)),
            ),
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn color_to_hex(c: &Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn hex_to_color(hex: &str) -> Option<Color> {
    Color::from_hex(hex)
}

fn bump_render(mut render_version: Signal<u64>) {
    *render_version.write() += 1;
}

fn current_document_name(path: Option<&PathBuf>) -> String {
    path.and_then(|file| file.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled.svg".to_string())
}

fn set_window_title(desktop: &DesktopContext, path: Option<&PathBuf>) {
    desktop.set_title(&format!("Sivgo Studio - {}", current_document_name(path)));
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


fn action_text(item: &MenuNode) -> String {
    match item.shortcut.as_deref() {
        Some(shortcut) if !shortcut.is_empty() => format!("{}\t{}", item.label, shortcut),
        _ => item.label.clone(),
    }
}

fn append_menu_items(submenu: &Submenu, items: &[MenuNode]) {
    for item in items {
        if item.separator {
            let separator = PredefinedMenuItem::separator();
            let _ = submenu.append(&separator);
            continue;
        }

        if !item.children.is_empty() {
            let child_menu = Submenu::new(&item.label, item.enabled);
            append_menu_items(&child_menu, &item.children);
            let _ = submenu.append(&child_menu);
            continue;
        }

        let Some(action) = item.action.as_deref() else {
            continue;
        };

        let text = action_text(item);
        if let Some(checked) = item.checked {
            let native_item = CheckMenuItem::with_id(action, text, item.enabled, checked, None);
            let _ = submenu.append(&native_item);
        } else {
            let native_item = NativeMenuItem::with_id(action, text, item.enabled, None);
            let _ = submenu.append(&native_item);
        }
    }
}

fn build_native_menu() -> Menu {
    let menu = Menu::new();
    let nodes = default_menu_bar_with_state(true, true, true);

    for node in nodes {
        if node.separator || node.children.is_empty() {
            continue;
        }

        let submenu = Submenu::new(&node.label, node.enabled);
        append_menu_items(&submenu, &node.children);
        let _ = menu.append(&submenu);
    }

    menu
}

/// Build the layer entry list for the layers panel.
fn build_layer_entries(engine: &EditorEngine) -> Vec<LayerEntry> {
    let explicit_layers = engine.get_layers();
    if !explicit_layers.is_empty() {
        return explicit_layers
            .into_iter()
            .filter_map(|(id, label)| {
                let node = engine.document.get_node(id)?;
                Some(LayerEntry {
                    id,
                    label,
                    selected: engine.selection.is_selected(id),
                    visible: !node.hidden,
                    locked: node.locked,
                    opacity: node.presentation.opacity.unwrap_or(1.0),
                    blend_mode: "Normal".to_string(),
                })
            })
            .collect();
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
            if node.element.tag_name() == "defs" {
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
                selected: sel.is_selected(child_id),
                visible: !node.hidden,
                locked: node.locked,
                opacity: node.presentation.opacity.unwrap_or(1.0),
                blend_mode: "Normal".to_string(),
            })
        })
        .collect()
}

fn get_selected_fill(engine: &EditorEngine) -> String {
    if let Some(id) = engine.selection.primary() {
        if let Some(node) = engine.document.get_node(id) {
            if let Some(Paint::Color(c)) = &node.presentation.fill {
                return color_to_hex(c);
            }
        }
    }
    "#ff8a5b".to_string()
}

fn get_selected_stroke(engine: &EditorEngine) -> String {
    if let Some(id) = engine.selection.primary() {
        if let Some(node) = engine.document.get_node(id) {
            if let Some(Paint::Color(c)) = &node.presentation.stroke {
                return color_to_hex(c);
            }
        }
    }
    "#10232a".to_string()
}

fn get_selected_stroke_width(engine: &EditorEngine) -> f64 {
    if let Some(id) = engine.selection.primary() {
        if let Some(node) = engine.document.get_node(id) {
            if let Some(ref sw) = node.presentation.stroke_width {
                return sw.value;
            }
        }
    }
    1.0
}

/// Convert dioxus mouse event to our engine MouseEvent.
fn make_engine_mouse_event(
    evt: &dioxus::prelude::Event<dioxus::events::MouseData>,
) -> sivgo_core::MouseEvent {
    let coords = evt.data().element_coordinates();
    let mods = evt.data().modifiers();
    sivgo_core::MouseEvent {
        x: coords.x,
        y: coords.y,
        button: sivgo_core::MouseButton::Left,
        modifiers: sivgo_core::Modifiers {
            ctrl: mods.contains(keyboard_types::Modifiers::CONTROL),
            shift: mods.contains(keyboard_types::Modifiers::SHIFT),
            alt: mods.contains(keyboard_types::Modifiers::ALT),
        },
    }
}

fn show_stub_dialog(title: &str, description: &str) {
    let _ = MessageDialog::new()
        .set_level(MessageLevel::Info)
        .set_title(title)
        .set_description(description)
        .set_buttons(MessageButtons::Ok)
        .show();
}

fn handle_editor_action(
    action: &str,
    mut engine: Signal<EditorEngine>,
    render_version: Signal<u64>,
    mut file_path: Signal<Option<PathBuf>>,
    mut is_fullscreen: Signal<bool>,
    desktop: DesktopContext,
) {
    match action {
        "file.new" => {
            engine
                .write()
                .new_document(CANVAS_WIDTH as f64, CANVAS_HEIGHT as f64);
            file_path.set(None);
            set_window_title(&desktop, None);
            bump_render(render_version);
        }
        "file.open" => {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("SVG files", &["svg"])
                .pick_file()
            {
                match fs::read_to_string(&path) {
                    Ok(content) => match sivgo_dom::parse_svg(&content) {
                        Ok(doc) => {
                            engine.write().load_document(doc);
                            file_path.set(Some(path.clone()));
                            set_window_title(&desktop, Some(&path));
                            bump_render(render_version);
                        }
                        Err(e) => log::error!("Failed to parse SVG: {}", e),
                    },
                    Err(e) => log::error!("Failed to read file: {}", e),
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

            let xml = {
                let eng = engine.read();
                sivgo_dom::serialize_svg(&eng.document)
            };

            if let Err(e) = fs::write(&path, xml.as_bytes()) {
                log::error!("Failed to save: {}", e);
            } else {
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
                let xml = {
                    let eng = engine.read();
                    sivgo_dom::serialize_svg(&eng.document)
                };
                if let Err(e) = fs::write(&path, xml.as_bytes()) {
                    log::error!("Failed to save: {}", e);
                } else {
                    file_path.set(Some(path.clone()));
                    set_window_title(&desktop, Some(&path));
                    info!("Saved as {}", path.display());
                }
            }
        }
        "file.export_png" => {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG images", &["png"])
                .save_file()
            {
                let eng = engine.read();
                if let Err(e) =
                    sivgo_io::export_png(&eng.document, &path, CANVAS_WIDTH, CANVAS_HEIGHT)
                {
                    log::error!("Failed to export PNG: {}", e);
                } else {
                    info!("Exported PNG to {}", path.display());
                }
            }
        }
        "file.print" => desktop.print(),
        "file.close" | "file.quit" => desktop.close(),
        "file.document_properties" => show_stub_dialog(
            "Document Properties",
            "Document properties panel is not wired into the desktop shell yet.",
        ),
        "edit.undo" => {
            engine.write().undo();
            bump_render(render_version);
        }
        "edit.redo" => {
            engine.write().redo();
            bump_render(render_version);
        }
        "edit.cut" => {
            engine.write().cut();
            bump_render(render_version);
        }
        "edit.copy" => engine.write().copy(),
        "edit.paste" => {
            engine.write().paste();
            bump_render(render_version);
        }
        "edit.paste_in_place" => {
            engine.write().paste_in_place();
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
        "edit.duplicate" => {
            engine.write().duplicate_selected();
            bump_render(render_version);
        }
        "edit.preferences" => show_stub_dialog(
            "Preferences",
            "Desktop preferences dialog is not implemented yet.",
        ),
        "view.zoom_in" => {
            let mut eng = engine.write();
            let center =
                sivgo_math::Point::new(eng.viewport.width / 2.0, eng.viewport.height / 2.0);
            eng.viewport.zoom_at(center, 1.15);
            drop(eng);
            bump_render(render_version);
        }
        "view.zoom_out" => {
            let mut eng = engine.write();
            let center =
                sivgo_math::Point::new(eng.viewport.width / 2.0, eng.viewport.height / 2.0);
            eng.viewport.zoom_at(center, 1.0 / 1.15);
            drop(eng);
            bump_render(render_version);
        }
        "view.zoom_100" => {
            let mut eng = engine.write();
            eng.viewport.zoom = 1.0;
            eng.viewport.pan = sivgo_math::Vector::ZERO;
            drop(eng);
            bump_render(render_version);
        }
        "view.zoom_fit" | "view.zoom_fit_page" => {
            let mut eng = engine.write();
            eng.viewport.fit_rect(sivgo_math::Rect::new(
                0.0,
                0.0,
                CANVAS_WIDTH as f64,
                CANVAS_HEIGHT as f64,
            ));
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
        "view.fullscreen" => {
            let next = !*is_fullscreen.read();
            is_fullscreen.set(next);
            desktop.set_fullscreen(next);
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
        "layer.add" => {
            let next_index = engine.read().get_layers().len() + 1;
            engine.write().add_layer(&format!("Layer {}", next_index));
            bump_render(render_version);
        }
        "help.about" => {
            let _ = MessageDialog::new()
                .set_level(MessageLevel::Info)
                .set_title("About Sivgo")
                .set_description(
                    "Sivgo Studio\nA desktop SVG editor built with Rust, Dioxus, and native desktop menus.",
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

#[allow(non_snake_case)]
fn App() -> Element {
    let desktop = use_window();

    let mut engine = use_signal(|| EditorEngine::new(CANVAS_WIDTH as f64, CANVAS_HEIGHT as f64));
    let render_version = use_signal(|| 0u64);
    let mut cursor_x = use_signal(|| 0.0f64);
    let mut cursor_y = use_signal(|| 0.0f64);
    let file_path: Signal<Option<PathBuf>> = use_signal(|| None);
    let mut active_tool = use_signal(|| ToolId::Select);
    let is_fullscreen = use_signal(|| false);

    use_muda_event_handler({
        let desktop = desktop.clone();
        move |event| {
            handle_editor_action(
                event.id().as_ref(),
                engine,
                render_version,
                file_path,
                is_fullscreen,
                desktop.clone(),
            );
        }
    });

    let _ = *render_version.read();

    let canvas_data_uri = {
        let eng = engine.read();
        sivgo_ui::render_to_data_uri(&eng, CANVAS_WIDTH, CANVAS_HEIGHT)
    };

    let eng = engine.read();
    let can_undo = eng.history.can_undo();
    let can_redo = eng.history.can_redo();
    let selection_count = eng.selection.len();
    let object_count = eng.document.len();
    let layers = build_layer_entries(&eng);
    let fill_hex = get_selected_fill(&eng);
    let stroke_hex = get_selected_stroke(&eng);
    let stroke_width_val = get_selected_stroke_width(&eng);
    let zoom = eng.viewport.zoom;
    let display_mode = eng.viewport.display_mode;
    drop(eng);

    let current_file = file_path.read().clone();
    let document_name = current_document_name(current_file.as_ref());
    let active_tool_id = *active_tool.read();
    let active_tool_name = tool_label(active_tool_id);
    let zoom_pct = format!("{:.0}%", zoom * 100.0);
    let selection_summary = if selection_count == 0 {
        "No selection".to_string()
    } else if selection_count == 1 {
        "1 selected".to_string()
    } else {
        format!("{} selected", selection_count)
    };

    rsx! {
        style { {CSS} }
        div { class: "app",
            // ---- Compact Top Bar ----
            div { class: "topbar",
                div { class: "topbar-brand",
                    div { class: "brand-mark", "S" }
                    span { class: "brand-title", "Sivgo" }
                }
                div { class: "topbar-actions",
                    button {
                        class: "tb-btn",
                        onclick: {
                            let desktop = desktop.clone();
                            move |_| handle_editor_action("file.new", engine, render_version, file_path, is_fullscreen, desktop.clone())
                        },
                        "New"
                    }
                    button {
                        class: "tb-btn",
                        onclick: {
                            let desktop = desktop.clone();
                            move |_| handle_editor_action("file.open", engine, render_version, file_path, is_fullscreen, desktop.clone())
                        },
                        "Open"
                    }
                    button {
                        class: "tb-btn",
                        onclick: {
                            let desktop = desktop.clone();
                            move |_| handle_editor_action("file.save", engine, render_version, file_path, is_fullscreen, desktop.clone())
                        },
                        "Save"
                    }
                    button {
                        class: "tb-btn",
                        onclick: {
                            let desktop = desktop.clone();
                            move |_| handle_editor_action("file.export_png", engine, render_version, file_path, is_fullscreen, desktop.clone())
                        },
                        "Export"
                    }
                    div { class: "tb-sep" }
                    button {
                        class: "tb-btn",
                        disabled: !can_undo,
                        onclick: {
                            let desktop = desktop.clone();
                            move |_| handle_editor_action("edit.undo", engine, render_version, file_path, is_fullscreen, desktop.clone())
                        },
                        "Undo"
                    }
                    button {
                        class: "tb-btn",
                        disabled: !can_redo,
                        onclick: {
                            let desktop = desktop.clone();
                            move |_| handle_editor_action("edit.redo", engine, render_version, file_path, is_fullscreen, desktop.clone())
                        },
                        "Redo"
                    }
                    div { class: "tb-sep" }
                    button {
                        class: "tb-btn",
                        onclick: {
                            let desktop = desktop.clone();
                            move |_| handle_editor_action("view.zoom_out", engine, render_version, file_path, is_fullscreen, desktop.clone())
                        },
                        "-"
                    }
                    button {
                        class: "tb-btn",
                        onclick: {
                            let desktop = desktop.clone();
                            move |_| handle_editor_action("view.zoom_100", engine, render_version, file_path, is_fullscreen, desktop.clone())
                        },
                        "{zoom_pct}"
                    }
                    button {
                        class: "tb-btn",
                        onclick: {
                            let desktop = desktop.clone();
                            move |_| handle_editor_action("view.zoom_in", engine, render_version, file_path, is_fullscreen, desktop.clone())
                        },
                        "+"
                    }
                    button {
                        class: "tb-btn",
                        onclick: {
                            let desktop = desktop.clone();
                            move |_| handle_editor_action("view.zoom_fit", engine, render_version, file_path, is_fullscreen, desktop.clone())
                        },
                        "Fit"
                    }
                    div { class: "tb-sep" }
                    button {
                        class: if display_mode == DisplayMode::Normal { "tb-btn active" } else { "tb-btn" },
                        onclick: {
                            let desktop = desktop.clone();
                            move |_| handle_editor_action("view.display_mode_normal", engine, render_version, file_path, is_fullscreen, desktop.clone())
                        },
                        "Normal"
                    }
                    button {
                        class: if display_mode == DisplayMode::Outline { "tb-btn active" } else { "tb-btn" },
                        onclick: {
                            let desktop = desktop.clone();
                            move |_| handle_editor_action("view.display_mode_outline", engine, render_version, file_path, is_fullscreen, desktop.clone())
                        },
                        "Outline"
                    }
                }
                div { class: "topbar-spacer" }
                div { class: "topbar-info",
                    span { class: "doc-name", "{document_name}" }
                    span { "{active_tool_name}" }
                    span { "{selection_summary}" }
                }
            }

            // ---- Main Workspace ----
            div { class: "workspace",
                // Tool Rail (compact, icon-only)
                aside { class: "left-rail",
                    Toolbar {
                        active_tool: active_tool_id,
                        on_tool_change: move |id: ToolId| {
                            let mut eng = engine.write();
                            eng.tools.switch_to(id);
                            let actual = eng.tools.active_id();
                            drop(eng);
                            active_tool.set(actual);
                        },
                    }
                }

                // Canvas
                section { class: "canvas-stage",
                    div { class: "canvas-frame",
                        div { class: "canvas-container",
                            img {
                                src: "{canvas_data_uri}",
                                draggable: "false",
                                onmousedown: move |evt: Event<dioxus::events::MouseData>| {
                                    let me = make_engine_mouse_event(&evt);
                                    engine.write().on_mouse_down(me);
                                    bump_render(render_version);
                                },
                                onmousemove: move |evt: Event<dioxus::events::MouseData>| {
                                    let coords = evt.data().element_coordinates();
                                    cursor_x.set(coords.x);
                                    cursor_y.set(coords.y);
                                    let me = make_engine_mouse_event(&evt);
                                    engine.write().on_mouse_move(me);
                                    bump_render(render_version);
                                },
                                onmouseup: move |evt: Event<dioxus::events::MouseData>| {
                                    let me = make_engine_mouse_event(&evt);
                                    engine.write().on_mouse_up(me);
                                    bump_render(render_version);
                                },
                            }
                        }
                    }
                }

                // Inspector Panel (right)
                aside { class: "inspector-shell",
                    div { class: "right-panels",
                        PropertiesPanel {
                            fill: fill_hex.clone(),
                            fill_paint_type: "Flat Color".to_string(),
                            fill_opacity: 1.0,
                            stroke: stroke_hex.clone(),
                            stroke_paint_type: "Flat Color".to_string(),
                            stroke_opacity: 1.0,
                            stroke_width: stroke_width_val,
                            line_cap: "Butt".to_string(),
                            line_join: "Miter".to_string(),
                            dash_pattern: String::new(),
                            dash_offset: 0.0,
                            marker_start: String::new(),
                            marker_mid: String::new(),
                            marker_end: String::new(),
                            has_selection: selection_count > 0,
                            on_action: move |action: sivgo_ui::properties_panel::PropertiesAction| {
                                use sivgo_ui::properties_panel::PropertiesAction;
                                match action {
                                    PropertiesAction::SetFillColor(hex) => {
                                        let mut eng = engine.write();
                                        if let Some(id) = eng.selection.primary() {
                                            if let Some(color) = hex_to_color(&hex) {
                                                if let Some(node) = eng.document.get_node(id) {
                                                    let old_attrs = node.presentation.clone();
                                                    let mut new_attrs = old_attrs.clone();
                                                    new_attrs.fill = Some(Paint::Color(color));
                                                    let cmd = ModifyAttributeCommand::new(id, old_attrs, new_attrs);
                                                    eng.execute(Box::new(cmd));
                                                }
                                            }
                                        }
                                        drop(eng);
                                        bump_render(render_version);
                                    }
                                    PropertiesAction::SetStrokeColor(hex) => {
                                        let mut eng = engine.write();
                                        if let Some(id) = eng.selection.primary() {
                                            if let Some(color) = hex_to_color(&hex) {
                                                if let Some(node) = eng.document.get_node(id) {
                                                    let old_attrs = node.presentation.clone();
                                                    let mut new_attrs = old_attrs.clone();
                                                    new_attrs.stroke = Some(Paint::Color(color));
                                                    let cmd = ModifyAttributeCommand::new(id, old_attrs, new_attrs);
                                                    eng.execute(Box::new(cmd));
                                                }
                                            }
                                        }
                                        drop(eng);
                                        bump_render(render_version);
                                    }
                                    PropertiesAction::SetStrokeWidth(val) => {
                                        let mut eng = engine.write();
                                        if let Some(id) = eng.selection.primary() {
                                            if let Some(node) = eng.document.get_node(id) {
                                                let old_attrs = node.presentation.clone();
                                                let mut new_attrs = old_attrs.clone();
                                                new_attrs.stroke_width = Some(sivgo_dom::SvgLength::new(val, sivgo_dom::LengthUnit::None));
                                                let cmd = ModifyAttributeCommand::new(id, old_attrs, new_attrs);
                                                eng.execute(Box::new(cmd));
                                            }
                                        }
                                        drop(eng);
                                        bump_render(render_version);
                                    }
                                    _ => log::info!("Unhandled properties action: {:?}", action),
                                }
                            },
                        }
                        LayersPanel {
                            layers: layers,
                            on_action: move |action: sivgo_ui::layers_panel::LayerAction| {
                                use sivgo_ui::layers_panel::LayerAction;
                                match action {
                                    LayerAction::Select(id) => {
                                        engine.write().selection.select(id);
                                        bump_render(render_version);
                                    }
                                    LayerAction::ToggleVisibility(id) => {
                                        engine.write().toggle_visibility(id);
                                        bump_render(render_version);
                                    }
                                    LayerAction::ToggleLock(id) => {
                                        engine.write().toggle_lock(id);
                                        bump_render(render_version);
                                    }
                                    LayerAction::MoveUp(id) => {
                                        let mut eng = engine.write();
                                        eng.selection.select(id);
                                        eng.raise_selected();
                                        drop(eng);
                                        bump_render(render_version);
                                    }
                                    LayerAction::MoveDown(id) => {
                                        let mut eng = engine.write();
                                        eng.selection.select(id);
                                        eng.lower_selected();
                                        drop(eng);
                                        bump_render(render_version);
                                    }
                                    LayerAction::SetOpacity(id, value) => {
                                        let mut eng = engine.write();
                                        if let Some(node) = eng.document.get_node(id) {
                                            let old_attrs = node.presentation.clone();
                                            let mut new_attrs = old_attrs.clone();
                                            new_attrs.opacity = Some(value);
                                            let cmd = ModifyAttributeCommand::new(id, old_attrs, new_attrs);
                                            eng.execute(Box::new(cmd));
                                        }
                                        drop(eng);
                                        bump_render(render_version);
                                    }
                                    LayerAction::SetBlendMode(_, mode) => {
                                        log::info!("Blend mode editing is not wired yet: {}", mode);
                                    }
                                }
                            },
                        }
                    }
                }
            }

            // ---- Status Bar ----
            StatusBar {
                cursor_x: *cursor_x.read(),
                cursor_y: *cursor_y.read(),
                zoom: zoom,
                selection_count: selection_count,
                object_count: object_count,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        run_cli();
    } else {
        launch_gui();
    }
}
