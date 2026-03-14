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
:root {
    --bg-0: #091318;
    --bg-1: #102027;
    --bg-2: #162d36;
    --panel: rgba(14, 28, 35, 0.8);
    --panel-strong: rgba(11, 24, 30, 0.94);
    --panel-edge: rgba(255, 255, 255, 0.08);
    --ink-0: #eff6f3;
    --ink-1: #bfd1cb;
    --ink-2: #87a39f;
    --accent: #ff8a5b;
    --accent-strong: #ff6d3f;
    --accent-soft: rgba(255, 138, 91, 0.16);
    --teal-soft: rgba(112, 198, 187, 0.18);
    --canvas-surface: #f3eadb;
    --shadow-lg: 0 18px 44px rgba(0, 0, 0, 0.32);
    --shadow-xl: 0 28px 80px rgba(0, 0, 0, 0.42);
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
    font-family: "Aptos", "Bahnschrift", "Trebuchet MS", "Segoe UI Variable Text", sans-serif;
    font-size: 13px;
    color: var(--ink-0);
    user-select: none;
    background:
        radial-gradient(circle at 10% 10%, rgba(255, 138, 91, 0.14), transparent 24%),
        radial-gradient(circle at 92% 18%, rgba(112, 198, 187, 0.16), transparent 18%),
        linear-gradient(145deg, #081116 0%, #10232a 44%, #0f1d24 100%);
}

button,
input,
select {
    font: inherit;
}

button {
    border: none;
    background: none;
    color: inherit;
}

.app {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    padding: 18px 18px 14px;
    gap: 14px;
}

.topbar,
.left-rail,
.canvas-stage,
.inspector-shell,
.status-bar {
    border: 1px solid var(--panel-edge);
    background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent),
        linear-gradient(180deg, rgba(17, 40, 48, 0.96), rgba(11, 24, 31, 0.92));
    box-shadow: var(--shadow-lg);
    backdrop-filter: blur(18px);
}

.topbar {
    display: grid;
    grid-template-columns: minmax(280px, 1.2fr) minmax(360px, 1.5fr) minmax(220px, 0.9fr);
    align-items: center;
    gap: 18px;
    min-height: 104px;
    padding: 18px 22px;
    border-radius: 28px;
}

.brand-block {
    display: flex;
    align-items: center;
    gap: 16px;
}

.brand-mark {
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    border-radius: 18px;
    background: linear-gradient(145deg, var(--accent), var(--accent-strong));
    color: #fff8f4;
    font-size: 24px;
    font-weight: 800;
    letter-spacing: 0.08em;
    box-shadow: 0 16px 28px rgba(255, 109, 63, 0.3);
}

.brand-copy,
.rail-head,
.inspector-head,
.meta-stack,
.tool-meta {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.brand-kicker,
.rail-eyebrow,
.inspector-kicker,
.meta-kicker,
.toolbar-group-label,
.panel-header,
.layer-control-label {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--ink-2);
}

.brand-title,
.rail-title,
.inspector-title {
    letter-spacing: 0.04em;
}

.brand-title {
    font-size: 24px;
    line-height: 1;
    text-transform: uppercase;
}

.doc-title,
.rail-copy,
.inspector-copy,
.status-item {
    color: var(--ink-1);
}

.topbar-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 10px;
}

.action-cluster {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 6px;
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.05);
}

.quick-btn {
    min-width: 74px;
    height: 38px;
    padding: 0 14px;
    border-radius: 14px;
    border: 1px solid transparent;
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-0);
    cursor: pointer;
    transition: transform 160ms ease, background 160ms ease, border-color 160ms ease;
}

.quick-btn:hover:not(:disabled) {
    transform: translateY(-1px);
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.12);
}

.quick-btn:disabled {
    opacity: 0.38;
    cursor: default;
}

.quick-btn.primary {
    background: linear-gradient(135deg, var(--accent), var(--accent-strong));
    color: #fff8f4;
}

.topbar-meta,
.stage-chip-row {
    display: flex;
    align-items: center;
    gap: 8px;
}

.topbar-meta {
    justify-content: flex-end;
}

.top-chip,
.stage-chip,
.paint-tab {
    display: inline-flex;
    align-items: center;
    height: 30px;
    padding: 0 12px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-1);
}

.top-chip.active,
.stage-chip.active,
.paint-tab.active {
    background: var(--accent-soft);
    border-color: rgba(255, 138, 91, 0.32);
    color: #ffd3c3;
}

.workspace {
    display: grid;
    grid-template-columns: 118px minmax(0, 1fr) 336px;
    gap: 14px;
    min-height: 0;
    flex: 1;
}

.left-rail,
.inspector-shell {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 16px 14px 14px;
    border-radius: 28px;
}

.toolbar,
.right-panels {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
}

.toolbar-group {
    margin-bottom: 16px;
    padding-top: 10px;
    border-top: 1px solid rgba(255, 255, 255, 0.04);
}

.toolbar-group:first-child {
    padding-top: 0;
    border-top: none;
}

.toolbar-group-label {
    margin: 0 0 10px 6px;
}

.tool-btn {
    width: 100%;
    margin-bottom: 8px;
    padding: 10px;
    border-radius: 18px;
    border: 1px solid rgba(255, 255, 255, 0.05);
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.04), rgba(255, 255, 255, 0.015));
    cursor: pointer;
}

.tool-btn:hover {
    border-color: rgba(255, 255, 255, 0.12);
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0.025));
}

.tool-btn.active {
    border-color: rgba(255, 138, 91, 0.34);
    background: linear-gradient(180deg, rgba(255, 138, 91, 0.16), rgba(255, 138, 91, 0.08));
}

.tool-btn-main {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr);
    align-items: center;
    gap: 10px;
}

.tool-icon {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    font-size: 16px;
    color: #fff7f1;
}

.tool-label {
    font-size: 12px;
    font-weight: 700;
    color: var(--ink-0);
}

.tool-shortcut {
    color: var(--ink-2);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
}

.canvas-stage {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border-radius: 34px;
    overflow: hidden;
    box-shadow: var(--shadow-xl);
}

.stage-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 14px;
    padding: 16px 20px 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.canvas-frame {
    position: relative;
    flex: 1;
    min-height: 0;
    padding: 20px;
    background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.02), transparent),
        linear-gradient(180deg, rgba(6, 15, 19, 0.38), rgba(6, 15, 19, 0.64));
}

.canvas-frame::before {
    content: "";
    position: absolute;
    inset: 22px;
    border-radius: 28px;
    background:
        repeating-linear-gradient(0deg, rgba(255, 255, 255, 0.03) 0, rgba(255, 255, 255, 0.03) 1px, transparent 1px, transparent 26px),
        repeating-linear-gradient(90deg, rgba(255, 255, 255, 0.03) 0, rgba(255, 255, 255, 0.03) 1px, transparent 1px, transparent 26px);
    opacity: 0.32;
    pointer-events: none;
}

.canvas-container {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    border-radius: 26px;
}

.canvas-container::before {
    content: "";
    position: absolute;
    width: min(72vw, 820px);
    height: min(72vw, 820px);
    background: radial-gradient(circle, rgba(255, 138, 91, 0.12), transparent 62%);
    filter: blur(18px);
    pointer-events: none;
}

.canvas-container img {
    position: relative;
    z-index: 1;
    max-width: 100%;
    max-height: 100%;
    border-radius: 18px;
    border: 1px solid rgba(23, 37, 47, 0.22);
    background: var(--canvas-surface);
    box-shadow: 0 24px 46px rgba(0, 0, 0, 0.32), 0 0 0 14px rgba(243, 234, 219, 0.04);
    cursor: crosshair;
}

.inspector-shell {
    border-radius: 28px;
}

.right-panels {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-right: 2px;
}

.properties-panel,
.layers-panel {
    border-radius: 22px;
    border: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(8, 18, 23, 0.56);
    overflow: hidden;
}

.panel-header {
    padding: 14px 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(255, 255, 255, 0.03);
}

.prop-section {
    padding: 14px 16px 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
}

.prop-section:last-child {
    border-bottom: none;
}

.prop-section-header {
    margin-bottom: 12px;
    color: #f6faf7;
    font-size: 13px;
    font-weight: 700;
}

.paint-type-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 12px;
}

.paint-tab {
    cursor: pointer;
}

.prop-row,
.layer-controls {
    display: grid;
    align-items: center;
    gap: 10px;
}

.prop-row {
    grid-template-columns: 72px minmax(0, 1fr) auto;
    margin-bottom: 10px;
}

.prop-row:last-child {
    margin-bottom: 0;
}

.layer-controls {
    grid-column: 1 / -1;
    grid-template-columns: 54px minmax(0, 1fr) auto;
}

.prop-label,
.prop-empty,
.layer-empty {
    color: var(--ink-2);
}

.prop-color,
.prop-input,
.prop-hex-input,
.prop-select,
.layer-blend-select {
    height: 34px;
    width: 100%;
    padding: 0 12px;
    border-radius: 12px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-0);
    outline: none;
}

.prop-color {
    width: 44px;
    padding: 0;
}

.prop-slider,
.layer-opacity-slider {
    width: 100%;
    accent-color: var(--accent);
}

.prop-value,
.layer-opacity-value {
    color: var(--ink-1);
    font-variant-numeric: tabular-nums;
}

.prop-empty,
.layer-empty {
    padding: 18px 16px 20px;
    line-height: 1.6;
}

.layers-list {
    padding: 10px;
}

.layer-item {
    display: grid;
    grid-template-columns: 28px 28px minmax(0, 1fr) 24px 24px;
    gap: 8px;
    padding: 12px;
    margin-bottom: 10px;
    border-radius: 18px;
    border: 1px solid rgba(255, 255, 255, 0.04);
    background: rgba(255, 255, 255, 0.02);
}

.layer-item:hover {
    border-color: rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.04);
}

.layer-item.selected {
    border-color: rgba(255, 138, 91, 0.28);
    background: linear-gradient(180deg, rgba(255, 138, 91, 0.12), rgba(255, 138, 91, 0.06));
}

.layer-btn {
    width: 28px;
    height: 28px;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(255, 255, 255, 0.03);
    color: var(--ink-1);
    cursor: pointer;
}

.layer-btn:disabled {
    opacity: 0.35;
    cursor: default;
}

.layer-label {
    display: flex;
    align-items: center;
    min-width: 0;
    font-weight: 700;
    cursor: pointer;
}

.status-bar {
    display: flex;
    align-items: center;
    gap: 18px;
    min-height: 52px;
    padding: 0 18px;
    border-radius: 18px;
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
            let center = sivgo_math::Point::new(eng.viewport.width / 2.0, eng.viewport.height / 2.0);
            eng.viewport.zoom_at(center, 1.15);
            drop(eng);
            bump_render(render_version);
        }
        "view.zoom_out" => {
            let mut eng = engine.write();
            let center = sivgo_math::Point::new(eng.viewport.width / 2.0, eng.viewport.height / 2.0);
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
    drop(eng);

    let document_name = current_document_name(file_path.read().as_ref());
    let active_tool_name = tool_label(*active_tool.read());
    let zoom_pct = format!("{:.0}%", zoom * 100.0);
    let selection_summary = if selection_count == 0 {
        "No selection".to_string()
    } else if selection_count == 1 {
        "1 object selected".to_string()
    } else {
        format!("{} objects selected", selection_count)
    };

    rsx! {
        style { {CSS} }
        div { class: "app",
            div { class: "topbar",
                div { class: "brand-block",
                    div { class: "brand-mark", "X" }
                    div { class: "brand-copy",
                        span { class: "brand-kicker", "Desktop Studio" }
                        h1 { class: "brand-title", "Sivgo" }
                        p { class: "doc-title", "{document_name}" }
                    }
                }
                div { class: "topbar-actions",
                    div { class: "action-cluster",
                        button {
                            class: "quick-btn primary",
                            onclick: {
                                let desktop = desktop.clone();
                                move |_| handle_editor_action("file.new", engine, render_version, file_path, is_fullscreen, desktop.clone())
                            },
                            "New"
                        }
                        button {
                            class: "quick-btn",
                            onclick: {
                                let desktop = desktop.clone();
                                move |_| handle_editor_action("file.open", engine, render_version, file_path, is_fullscreen, desktop.clone())
                            },
                            "Open"
                        }
                        button {
                            class: "quick-btn",
                            onclick: {
                                let desktop = desktop.clone();
                                move |_| handle_editor_action("file.save", engine, render_version, file_path, is_fullscreen, desktop.clone())
                            },
                            "Save"
                        }
                    }
                    div { class: "action-cluster",
                        button {
                            class: "quick-btn",
                            disabled: !can_undo,
                            onclick: {
                                let desktop = desktop.clone();
                                move |_| handle_editor_action("edit.undo", engine, render_version, file_path, is_fullscreen, desktop.clone())
                            },
                            "Undo"
                        }
                        button {
                            class: "quick-btn",
                            disabled: !can_redo,
                            onclick: {
                                let desktop = desktop.clone();
                                move |_| handle_editor_action("edit.redo", engine, render_version, file_path, is_fullscreen, desktop.clone())
                            },
                            "Redo"
                        }
                    }
                    div { class: "action-cluster",
                        button {
                            class: "quick-btn",
                            onclick: {
                                let desktop = desktop.clone();
                                move |_| handle_editor_action("view.zoom_out", engine, render_version, file_path, is_fullscreen, desktop.clone())
                            },
                            "Zoom -"
                        }
                        button {
                            class: "quick-btn",
                            onclick: {
                                let desktop = desktop.clone();
                                move |_| handle_editor_action("view.zoom_fit", engine, render_version, file_path, is_fullscreen, desktop.clone())
                            },
                            "Fit"
                        }
                        button {
                            class: "quick-btn",
                            onclick: {
                                let desktop = desktop.clone();
                                move |_| handle_editor_action("view.zoom_in", engine, render_version, file_path, is_fullscreen, desktop.clone())
                            },
                            "Zoom +"
                        }
                    }
                }
                div { class: "topbar-meta",
                    div { class: "meta-stack",
                        span { class: "meta-kicker", "Workspace" }
                        span { class: "doc-title", "{selection_summary}" }
                    }
                    span { class: "top-chip active", "{active_tool_name}" }
                    span { class: "top-chip", "{zoom_pct}" }
                }
            }

            div { class: "workspace",
                aside { class: "left-rail",
                    div { class: "rail-head",
                        span { class: "rail-eyebrow", "Tool Rail" }
                        h2 { class: "rail-title", "{active_tool_name}" }
                        p { class: "rail-copy", "Compact tool access tuned for desktop editing." }
                    }
                    Toolbar {
                        active_tool: *active_tool.read(),
                        on_tool_change: move |id: ToolId| {
                            engine.write().tools.switch_to(id);
                            active_tool.set(id);
                        },
                    }
                }

                section { class: "canvas-stage",
                    div { class: "stage-bar",
                        div { class: "stage-chip-row",
                            span { class: "stage-chip active", "Desktop" }
                            span { class: "stage-chip", "{selection_summary}" }
                            span { class: "stage-chip", "Canvas {CANVAS_WIDTH} × {CANVAS_HEIGHT}" }
                        }
                        div { class: "stage-chip-row",
                            button {
                                class: "quick-btn",
                                onclick: {
                                    let desktop = desktop.clone();
                                    move |_| handle_editor_action("view.display_mode_outline", engine, render_version, file_path, is_fullscreen, desktop.clone())
                                },
                                "Outline"
                            }
                            button {
                                class: "quick-btn",
                                onclick: {
                                    let desktop = desktop.clone();
                                    move |_| handle_editor_action("view.display_mode_normal", engine, render_version, file_path, is_fullscreen, desktop.clone())
                                },
                                "Normal"
                            }
                            span { class: "stage-chip active", "Zoom {zoom_pct}" }
                        }
                    }
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

                aside { class: "inspector-shell",
                    div { class: "inspector-head",
                        span { class: "inspector-kicker", "Inspector" }
                        h2 { class: "inspector-title", "Appearance & Layers" }
                        p { class: "inspector-copy", "Fill, stroke, and layer controls live here." }
                    }
                    div { class: "right-panels",
                        PropertiesPanel {
                            fill: fill_hex,
                            fill_paint_type: "Flat Color".to_string(),
                            fill_opacity: 1.0,
                            stroke: stroke_hex,
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
