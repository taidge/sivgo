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
    --bg-0: #061218;
    --bg-1: #0b1c23;
    --bg-2: #122730;
    --panel: rgba(9, 22, 28, 0.78);
    --panel-strong: rgba(7, 18, 24, 0.92);
    --panel-edge: rgba(255, 255, 255, 0.08);
    --panel-edge-strong: rgba(255, 255, 255, 0.14);
    --ink-0: #edf7f3;
    --ink-1: #c0d4cd;
    --ink-2: #8ca8a0;
    --ink-3: #6b8680;
    --accent: #ff8a5b;
    --accent-strong: #ff6b3d;
    --accent-soft: rgba(255, 138, 91, 0.16);
    --accent-outline: rgba(255, 138, 91, 0.34);
    --teal: #74d0c4;
    --teal-soft: rgba(116, 208, 196, 0.14);
    --canvas-surface: #f3eadb;
    --canvas-edge: rgba(12, 24, 28, 0.22);
    --shadow-lg: 0 20px 48px rgba(0, 0, 0, 0.34);
    --shadow-xl: 0 28px 84px rgba(0, 0, 0, 0.44);
    --transition-fast: 160ms ease;
    --transition-med: 220ms ease;
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
        radial-gradient(circle at 10% 12%, rgba(255, 138, 91, 0.14), transparent 24%),
        radial-gradient(circle at 88% 14%, rgba(116, 208, 196, 0.18), transparent 20%),
        radial-gradient(circle at 50% 100%, rgba(255, 255, 255, 0.04), transparent 32%),
        linear-gradient(145deg, #051015 0%, #0b1b22 42%, #10232b 100%);
}

button,
input,
select {
    font: inherit;
    color: inherit;
}

button {
    border: none;
    background: none;
}

input,
select {
    outline: none;
}

option {
    background: #10242d;
    color: var(--ink-0);
}

*::-webkit-scrollbar {
    width: 10px;
    height: 10px;
}

*::-webkit-scrollbar-track {
    background: rgba(255, 255, 255, 0.03);
}

*::-webkit-scrollbar-thumb {
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.12);
}

*::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.22);
}

.app {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    padding: 18px 18px 14px;
    gap: 14px;
    min-width: 0;
}

.topbar,
.left-rail,
.canvas-stage,
.inspector-shell,
.status-bar {
    border: 1px solid var(--panel-edge);
    background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent),
        linear-gradient(180deg, rgba(15, 33, 40, 0.96), rgba(8, 20, 26, 0.94));
    box-shadow: var(--shadow-lg);
    backdrop-filter: blur(18px);
}

.topbar {
    display: grid;
    grid-template-columns: minmax(300px, 1.05fr) minmax(420px, 1.35fr) minmax(260px, 0.9fr);
    gap: 18px;
    min-height: 138px;
    padding: 20px 22px;
    border-radius: 30px;
}

.topbar-main,
.topbar-side,
.brand-copy,
.rail-head,
.inspector-head,
.meta-stack,
.tool-meta,
.panel-title-wrap {
    display: flex;
    flex-direction: column;
    gap: 6px;
}

.brand-kicker,
.rail-eyebrow,
.inspector-kicker,
.meta-kicker,
.metric-label,
.stage-kicker,
.canvas-hud-kicker,
.panel-kicker,
.layer-control-label,
.status-label,
.rail-spotlight-label,
.rail-footer-label,
.inspector-card-label,
.toolbar-group-label,
.doc-badge {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--ink-2);
}

.brand-block {
    display: flex;
    align-items: center;
    gap: 16px;
}

.brand-mark {
    display: grid;
    place-items: center;
    width: 62px;
    height: 62px;
    border-radius: 20px;
    background:
        radial-gradient(circle at 28% 20%, rgba(255, 255, 255, 0.18), transparent 36%),
        linear-gradient(145deg, var(--accent), var(--accent-strong));
    color: #fff8f4;
    font-size: 22px;
    font-weight: 800;
    letter-spacing: 0.1em;
    box-shadow: 0 18px 30px rgba(255, 109, 63, 0.28);
}

.brand-title,
.rail-title,
.inspector-title,
.stage-title {
    letter-spacing: 0.04em;
}

.brand-title {
    font-size: 28px;
    line-height: 1;
    text-transform: uppercase;
}

.doc-strip {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
}

.doc-title {
    color: var(--ink-0);
    font-size: 15px;
    font-weight: 600;
}

.doc-badge,
.top-chip,
.stage-chip,
.paint-tab,
.panel-badge {
    display: inline-flex;
    align-items: center;
    height: 30px;
    padding: 0 12px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-1);
}

.doc-badge.warning {
    background: rgba(255, 138, 91, 0.12);
    border-color: rgba(255, 138, 91, 0.28);
    color: #ffd0c0;
}

.brand-description,
.rail-copy,
.inspector-copy,
.session-copy,
.metric-copy,
.panel-meta,
.canvas-hud-copy,
.inspector-card-copy,
.status-item {
    color: var(--ink-1);
}

.brand-description,
.rail-copy,
.inspector-copy,
.session-copy,
.panel-meta,
.canvas-hud-copy,
.inspector-card-copy {
    line-height: 1.55;
}

.topbar-actions {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 12px;
}

.action-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
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
    min-width: 78px;
    height: 38px;
    padding: 0 14px;
    border-radius: 14px;
    border: 1px solid transparent;
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-0);
    cursor: pointer;
    transition:
        transform var(--transition-fast),
        background var(--transition-fast),
        border-color var(--transition-fast),
        box-shadow var(--transition-fast);
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
    box-shadow: 0 14px 24px rgba(255, 107, 61, 0.22);
}

.quick-btn.subtle {
    background: rgba(116, 208, 196, 0.08);
    border-color: rgba(116, 208, 196, 0.18);
}

.quick-btn.active,
.top-chip.active,
.stage-chip.active,
.paint-tab.active {
    background: var(--accent-soft);
    border-color: var(--accent-outline);
    color: #ffd3c3;
}

.metric-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
}

.metric-card,
.session-card,
.rail-spotlight,
.inspector-card,
.canvas-stat,
.stage-footer-item,
.status-cluster {
    border: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(255, 255, 255, 0.035);
}

.metric-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px 16px;
    border-radius: 22px;
}

.metric-card.accent {
    background: linear-gradient(180deg, rgba(255, 138, 91, 0.12), rgba(255, 138, 91, 0.04));
    border-color: var(--accent-outline);
}

.metric-value {
    font-size: 24px;
    line-height: 1;
    font-weight: 700;
    color: var(--ink-0);
}

.metric-copy {
    line-height: 1.45;
}

.topbar-side {
    justify-content: center;
    gap: 12px;
}

.session-card {
    display: grid;
    grid-template-columns: 54px minmax(0, 1fr);
    gap: 14px;
    align-items: center;
    padding: 14px 16px;
    border-radius: 24px;
}

.session-tool-mark,
.rail-spotlight-mark {
    display: grid;
    place-items: center;
    border-radius: 18px;
    background:
        radial-gradient(circle at 30% 20%, rgba(255, 255, 255, 0.18), transparent 38%),
        linear-gradient(145deg, rgba(255, 138, 91, 0.2), rgba(116, 208, 196, 0.18));
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #fff8f4;
    font-weight: 800;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
}

.session-tool-mark {
    width: 54px;
    height: 54px;
    font-size: 16px;
}

.session-title {
    font-size: 18px;
    font-weight: 700;
    color: var(--ink-0);
}

.topbar-meta,
.stage-chip-row,
.stage-mode-buttons,
.canvas-swatch-row,
.inspector-swatch-row,
.status-section {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
}

.workspace {
    display: grid;
    grid-template-columns: 142px minmax(0, 1fr) 360px;
    gap: 16px;
    min-height: 0;
    flex: 1;
}

.left-rail,
.inspector-shell {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 18px 16px 16px;
    border-radius: 30px;
}

.toolbar,
.right-panels {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding-right: 2px;
}

.rail-title,
.inspector-title {
    font-size: 21px;
    color: var(--ink-0);
}

.rail-spotlight {
    display: grid;
    grid-template-columns: 56px minmax(0, 1fr);
    gap: 12px;
    align-items: center;
    margin: 14px 0 16px;
    padding: 14px;
    border-radius: 24px;
}

.rail-spotlight-mark {
    width: 56px;
    height: 56px;
    font-size: 16px;
}

.rail-spotlight-value {
    font-size: 18px;
    font-weight: 700;
    color: var(--ink-0);
}

.rail-footer {
    margin-top: 14px;
    padding: 14px;
    border-radius: 22px;
    border: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(255, 255, 255, 0.025);
}

.toolbar-group {
    margin-bottom: 16px;
    padding-top: 12px;
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
    padding: 11px;
    border-radius: 18px;
    border: 1px solid rgba(255, 255, 255, 0.05);
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.04), rgba(255, 255, 255, 0.015));
    cursor: pointer;
    transition:
        transform var(--transition-fast),
        border-color var(--transition-fast),
        background var(--transition-fast),
        box-shadow var(--transition-fast);
}

.tool-btn:hover {
    transform: translateY(-1px);
    border-color: rgba(255, 255, 255, 0.12);
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0.025));
}

.tool-btn.active {
    border-color: var(--accent-outline);
    background: linear-gradient(180deg, rgba(255, 138, 91, 0.16), rgba(255, 138, 91, 0.08));
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
}

.tool-btn-main {
    display: grid;
    grid-template-columns: 30px minmax(0, 1fr);
    align-items: center;
    gap: 10px;
}

.tool-icon {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.04);
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
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 16px;
    padding: 18px 22px 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.stage-title {
    font-size: 22px;
    color: var(--ink-0);
}

.stage-copy {
    margin-top: 4px;
    max-width: 58ch;
    color: var(--ink-1);
    line-height: 1.55;
}

.stage-actions {
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: flex-end;
    gap: 12px;
}

.canvas-frame {
    position: relative;
    flex: 1;
    min-height: 0;
    padding: 28px;
    overflow: hidden;
    background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.02), transparent),
        linear-gradient(180deg, rgba(6, 15, 19, 0.44), rgba(6, 15, 19, 0.72));
}

.canvas-frame::before {
    content: "";
    position: absolute;
    inset: 24px;
    border-radius: 30px;
    background:
        repeating-linear-gradient(0deg, rgba(255, 255, 255, 0.03) 0, rgba(255, 255, 255, 0.03) 1px, transparent 1px, transparent 28px),
        repeating-linear-gradient(90deg, rgba(255, 255, 255, 0.03) 0, rgba(255, 255, 255, 0.03) 1px, transparent 1px, transparent 28px);
    opacity: 0.32;
    pointer-events: none;
}

.canvas-frame::after {
    content: "";
    position: absolute;
    inset: 0;
    background:
        radial-gradient(circle at 50% 42%, rgba(255, 138, 91, 0.08), transparent 32%),
        radial-gradient(circle at 14% 88%, rgba(116, 208, 196, 0.08), transparent 26%);
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
    padding: 42px;
    border-radius: 30px;
    border: 1px solid rgba(255, 255, 255, 0.04);
    background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent),
        linear-gradient(180deg, rgba(12, 23, 29, 0.56), rgba(9, 20, 24, 0.74));
    overflow: hidden;
}

.canvas-container::before {
    content: "";
    position: absolute;
    width: min(70vw, 840px);
    height: min(70vw, 840px);
    background: radial-gradient(circle, rgba(255, 138, 91, 0.12), transparent 62%);
    filter: blur(22px);
    pointer-events: none;
}

.canvas-container img {
    position: relative;
    z-index: 1;
    max-width: 100%;
    max-height: 100%;
    border-radius: 20px;
    border: 1px solid var(--canvas-edge);
    background: var(--canvas-surface);
    box-shadow: 0 28px 50px rgba(0, 0, 0, 0.34), 0 0 0 14px rgba(243, 234, 219, 0.05);
    cursor: crosshair;
    transition:
        transform var(--transition-med),
        box-shadow var(--transition-med);
}

.canvas-container img:hover {
    transform: translateY(-1px);
    box-shadow: 0 32px 58px rgba(0, 0, 0, 0.38), 0 0 0 14px rgba(243, 234, 219, 0.05);
}

.canvas-hud {
    position: absolute;
    z-index: 2;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 220px;
    max-width: 280px;
    padding: 16px 18px;
    border-radius: 22px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(6, 17, 22, 0.62);
    box-shadow: 0 16px 32px rgba(0, 0, 0, 0.22);
    backdrop-filter: blur(14px);
    pointer-events: none;
}

.canvas-hud.top-left {
    top: 34px;
    left: 34px;
}

.canvas-hud.top-right {
    top: 34px;
    right: 34px;
}

.canvas-hud.bottom-left {
    bottom: 34px;
    left: 34px;
}

.canvas-hud.bottom-right {
    bottom: 34px;
    right: 34px;
}

.canvas-hud-title {
    font-size: 16px;
    font-weight: 700;
    color: var(--ink-0);
}

.canvas-stat-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
}

.canvas-stat {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 12px;
    border-radius: 16px;
}

.canvas-stat-label {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--ink-2);
}

.canvas-stat-value {
    font-size: 18px;
    font-weight: 700;
    color: var(--ink-0);
}

.canvas-swatch {
    width: 20px;
    height: 20px;
    border-radius: 7px;
    border: 1px solid rgba(255, 255, 255, 0.18);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
}

.stage-footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    padding: 14px 22px 18px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(255, 255, 255, 0.02);
}

.stage-footer-item {
    min-height: 32px;
    padding: 0 12px;
    border-radius: 999px;
    display: inline-flex;
    align-items: center;
    color: var(--ink-1);
}

.inspector-shell {
    border-radius: 30px;
}

.inspector-overview {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
    margin: 14px 0;
}

.inspector-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px 16px;
    border-radius: 22px;
}

.inspector-card.wide {
    grid-column: 1 / -1;
}

.inspector-card-value {
    font-size: 18px;
    font-weight: 700;
    color: var(--ink-0);
}

.inspector-swatch {
    width: 18px;
    height: 18px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.16);
}

.right-panels {
    display: flex;
    flex-direction: column;
    gap: 14px;
}

.properties-panel,
.layers-panel {
    border-radius: 24px;
    border: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(8, 18, 23, 0.56);
    overflow: hidden;
}

.panel-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 16px 18px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    background: rgba(255, 255, 255, 0.03);
}

.prop-section {
    padding: 16px 18px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
}

.prop-section:last-child {
    border-bottom: none;
}

.panel-title {
    font-size: 15px;
    font-weight: 700;
    color: var(--ink-0);
    letter-spacing: 0.02em;
}

.panel-swatch-pair {
    display: flex;
    gap: 8px;
    align-items: center;
}

.panel-swatch {
    width: 18px;
    height: 18px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
}

.prop-section-header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 12px;
}

.prop-section-header {
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
    transition:
        border-color var(--transition-fast),
        background var(--transition-fast),
        color var(--transition-fast);
}

.paint-tab:hover {
    border-color: rgba(255, 255, 255, 0.14);
    background: rgba(255, 255, 255, 0.08);
}

.prop-row,
.layer-controls {
    display: grid;
    align-items: center;
    gap: 10px;
}

.prop-row {
    grid-template-columns: 88px minmax(0, 1fr) auto;
    margin-bottom: 12px;
}

.prop-row:last-child,
.prop-color-row:last-child {
    margin-bottom: 0;
}

.prop-color-row {
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr);
    gap: 10px;
    margin-bottom: 12px;
}

.layer-controls {
    grid-column: 1 / -1;
    grid-template-columns: 64px minmax(0, 1fr) auto;
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
    height: 38px;
    width: 100%;
    padding: 0 12px;
    border-radius: 12px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.04);
    color: var(--ink-0);
}

.prop-color {
    padding: 0;
    overflow: hidden;
}

.prop-input:focus,
.prop-hex-input:focus,
.prop-select:focus,
.layer-blend-select:focus {
    border-color: var(--accent-outline);
    box-shadow: 0 0 0 3px rgba(255, 138, 91, 0.12);
}

.prop-slider,
.layer-opacity-slider {
    width: 100%;
    accent-color: var(--accent);
}

.prop-value,
.layer-opacity-value {
    min-width: 42px;
    text-align: right;
    color: var(--ink-1);
    font-variant-numeric: tabular-nums;
}

.prop-empty,
.layer-empty {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 20px 18px 22px;
    line-height: 1.6;
}

.empty-title {
    color: var(--ink-0);
    font-weight: 700;
}

.layers-list {
    padding: 10px;
}

.layer-item {
    display: grid;
    grid-template-columns: 32px 32px minmax(0, 1fr) 28px 28px;
    gap: 10px;
    padding: 14px;
    margin-bottom: 10px;
    border-radius: 20px;
    border: 1px solid rgba(255, 255, 255, 0.04);
    background: rgba(255, 255, 255, 0.02);
    transition:
        border-color var(--transition-fast),
        background var(--transition-fast),
        transform var(--transition-fast);
}

.layer-item:last-child {
    margin-bottom: 0;
}

.layer-item:hover {
    transform: translateY(-1px);
    border-color: rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.04);
}

.layer-item.selected {
    border-color: rgba(255, 138, 91, 0.28);
    background: linear-gradient(180deg, rgba(255, 138, 91, 0.12), rgba(255, 138, 91, 0.06));
}

.layer-btn {
    width: 32px;
    height: 32px;
    border-radius: 11px;
    border: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(255, 255, 255, 0.03);
    color: var(--ink-1);
    cursor: pointer;
    transition:
        border-color var(--transition-fast),
        background var(--transition-fast);
}

.layer-btn:hover:not(:disabled) {
    border-color: rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.08);
}

.layer-btn:disabled {
    opacity: 0.35;
    cursor: default;
}

.layer-label-wrap {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 4px;
    min-width: 0;
    cursor: pointer;
}

.layer-label {
    min-width: 0;
    font-weight: 700;
    color: var(--ink-0);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.layer-meta {
    color: var(--ink-2);
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.status-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 58px;
    padding: 0 18px;
    border-radius: 20px;
}

.status-section {
    gap: 10px;
}

.status-cluster {
    min-height: 36px;
    padding: 0 12px;
    border-radius: 16px;
}

.status-cluster.emphasis {
    background: var(--accent-soft);
    border-color: var(--accent-outline);
}

@media (max-width: 1420px) {
    .topbar {
        grid-template-columns: minmax(280px, 1fr) minmax(420px, 1.2fr);
    }

    .topbar-side {
        grid-column: 1 / -1;
        flex-direction: row;
        align-items: stretch;
        justify-content: space-between;
    }

    .workspace {
        grid-template-columns: 132px minmax(0, 1fr) 328px;
    }
}

@media (max-width: 1240px) {
    .topbar {
        grid-template-columns: 1fr;
    }

    .metric-grid,
    .inspector-overview {
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .workspace {
        grid-template-columns: 122px minmax(0, 1fr) 300px;
    }

    .canvas-hud {
        min-width: 200px;
        max-width: 240px;
    }

    .canvas-container {
        padding: 34px;
    }

    .stage-bar {
        grid-template-columns: 1fr;
    }

    .stage-actions {
        align-items: flex-start;
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

fn tool_glyph(tool: ToolId) -> &'static str {
    match tool {
        ToolId::Select => "S",
        ToolId::Node => "N",
        ToolId::Rect => "R",
        ToolId::Ellipse => "E",
        ToolId::Star => "*",
        ToolId::Spiral => "@",
        ToolId::Pen => "P",
        ToolId::Pencil => "Pb",
        ToolId::Calligraphy => "Ink",
        ToolId::Text => "T",
        ToolId::Gradient => "G",
        ToolId::Dropper => "D",
        ToolId::BucketFill => "Bk",
        ToolId::Connector => "Cn",
        ToolId::Eraser => "Er",
        ToolId::Spray => "Sp",
        ToolId::Tweak => "Tw",
        ToolId::Measure => "Me",
        ToolId::Zoom => "Z",
        ToolId::Mesh => "Ms",
        ToolId::Box3d => "3D",
        ToolId::Marker => "Mk",
        ToolId::Pages => "Pg",
        ToolId::Lpe => "Fx",
    }
}

fn tool_shortcut(tool: ToolId) -> &'static str {
    match tool {
        ToolId::Select => "S",
        ToolId::Node => "N",
        ToolId::Rect => "R",
        ToolId::Ellipse => "E",
        ToolId::Star => "*",
        ToolId::Spiral => "I",
        ToolId::Pen => "B",
        ToolId::Pencil => "P",
        ToolId::Calligraphy => "C",
        ToolId::Text => "T",
        ToolId::Gradient => "G",
        ToolId::Dropper => "D",
        ToolId::BucketFill => "U",
        ToolId::Connector => "O",
        ToolId::Eraser => "Shift+E",
        ToolId::Spray => "A",
        ToolId::Tweak => "W",
        ToolId::Measure => "F5",
        ToolId::Zoom => "Z",
        ToolId::Mesh => "M",
        ToolId::Box3d => "X",
        ToolId::Marker => "K",
        ToolId::Pages | ToolId::Lpe => "Menu",
    }
}

fn tool_copy(tool: ToolId) -> &'static str {
    match tool {
        ToolId::Select => "Transform, move, and inspect objects directly on the stage.",
        ToolId::Node => "Refine path handles and clean up geometry with direct node editing.",
        ToolId::Rect => "Create crisp rectangular shapes with predictable dimensions.",
        ToolId::Ellipse => "Lay out circles and ellipses for icons, diagrams, and framing.",
        ToolId::Star => "Generate star and polygon primitives with adjustable symmetry.",
        ToolId::Spiral => "Sketch spirals quickly for decorative or technical shapes.",
        ToolId::Pen => "Draw precise Bezier paths for logos, illustrations, and vector curves.",
        ToolId::Pencil => "Capture quick freehand strokes before refining them.",
        ToolId::Calligraphy => "Simulate expressive ink strokes with pressure-inspired shapes.",
        ToolId::Text => "Place and edit live text directly on the canvas.",
        ToolId::Gradient => "Author linear and radial color transitions on selected objects.",
        ToolId::Dropper => "Sample color directly from the canvas and reuse it instantly.",
        ToolId::BucketFill => "Flood closed regions with a new fill treatment.",
        ToolId::Connector => "Build diagram-style connectors between objects.",
        ToolId::Eraser => "Remove portions of shapes to edit silhouettes quickly.",
        ToolId::Spray => "Distribute repeated shapes for texture and generative layouts.",
        ToolId::Tweak => "Push, pull, and sculpt vector forms with direct gestures.",
        ToolId::Measure => "Inspect distance and alignment before committing changes.",
        ToolId::Zoom => "Navigate dense artwork without losing precision.",
        ToolId::Mesh => "Set up multi-stop mesh color transitions.",
        ToolId::Box3d => "Construct perspective boxes for spatial illustration.",
        ToolId::Marker => "Place reusable marker treatments for arrows and path ends.",
        ToolId::Pages => "Manage multi-page layouts for print and presentation output.",
        ToolId::Lpe => "Apply live path effects without destructively editing source geometry.",
    }
}

fn display_mode_label(mode: DisplayMode) -> &'static str {
    match mode {
        DisplayMode::Normal => "Rendered",
        DisplayMode::Outline => "Outline",
        DisplayMode::NoFilters => "No Filters",
        DisplayMode::VisibleHairlines => "Hairlines",
    }
}

fn display_mode_copy(mode: DisplayMode) -> &'static str {
    match mode {
        DisplayMode::Normal => "Full render with fills, strokes, and effects enabled.",
        DisplayMode::Outline => "Geometry-only view for path cleanup and structure review.",
        DisplayMode::NoFilters => "Rendered output with filter effects temporarily disabled.",
        DisplayMode::VisibleHairlines => "Hairline strokes boosted for precision inspection.",
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
    let layer_count = layers.len();
    let fill_hex = get_selected_fill(&eng);
    let stroke_hex = get_selected_stroke(&eng);
    let stroke_width_val = get_selected_stroke_width(&eng);
    let zoom = eng.viewport.zoom;
    let display_mode = eng.viewport.display_mode;
    let page_count = eng.page_count();
    drop(eng);

    let current_file = file_path.read().clone();
    let document_name = current_document_name(current_file.as_ref());
    let active_tool_id = *active_tool.read();
    let active_tool_name = tool_label(active_tool_id);
    let active_tool_glyph = tool_glyph(active_tool_id);
    let active_tool_shortcut = tool_shortcut(active_tool_id);
    let active_tool_copy = tool_copy(active_tool_id);
    let zoom_pct = format!("{:.0}%", zoom * 100.0);
    let display_mode_name = display_mode_label(display_mode);
    let display_mode_note = display_mode_copy(display_mode);
    let selection_summary = if selection_count == 0 {
        "No selection".to_string()
    } else if selection_count == 1 {
        "1 object selected".to_string()
    } else {
        format!("{} objects selected", selection_count)
    };
    let selection_metric = if selection_count == 0 {
        "Ready".to_string()
    } else {
        selection_count.to_string()
    };
    let surface_count = page_count.max(1);
    let layer_summary = if layer_count == 1 {
        "1 layer".to_string()
    } else {
        format!("{} layers", layer_count)
    };
    let page_summary = if page_count == 0 {
        "Single canvas".to_string()
    } else if page_count == 1 {
        "1 page".to_string()
    } else {
        format!("{} pages", page_count)
    };
    let structure_summary = format!("{layer_summary} / {page_summary}");
    let document_state = if current_file.is_some() {
        "Linked file"
    } else {
        "Unsaved draft"
    };
    let document_state_copy = if current_file.is_some() {
        "This canvas is attached to a file on disk and ready for repeated saves."
    } else {
        "Work is live in memory. Open or save the document to anchor it to disk."
    };
    let history_state = if can_undo || can_redo {
        "History available"
    } else {
        "Fresh session"
    };
    let fullscreen_enabled = *is_fullscreen.read();
    let fullscreen_label = if fullscreen_enabled {
        "Fullscreen"
    } else {
        "Windowed"
    };
    let fullscreen_button_label = if fullscreen_enabled {
        "Exit Fullscreen"
    } else {
        "Fullscreen"
    };
    let selection_state = if selection_count > 0 {
        "Selection live"
    } else {
        "Canvas ready"
    };
    let stage_hint = if selection_count == 0 {
        format!(
            "Start with {} or choose Select to inspect an existing object.",
            active_tool_name
        )
    } else {
        format!(
            "{} Use the inspector to refine paint, stroke, and stacking.",
            selection_summary
        )
    };
    let stroke_width_label = format!("{stroke_width_val:.1}px");
    let paint_summary = format!("Fill {fill_hex} / Stroke {stroke_hex} / {stroke_width_label}");
    let fill_swatch_style = format!("background: {fill_hex};");
    let stroke_swatch_style = format!("background: {stroke_hex};");
    let document_badge_class = if current_file.is_some() {
        "doc-badge"
    } else {
        "doc-badge warning"
    };
    let selection_metric_class = if selection_count > 0 {
        "metric-card accent"
    } else {
        "metric-card"
    };
    let selection_preview_title = if selection_count > 0 {
        "Live preview"
    } else {
        "Waiting for selection"
    };

    rsx! {
        style { {CSS} }
        div { class: "app",
            div { class: "topbar",
                div { class: "topbar-main",
                    div { class: "brand-block",
                        div { class: "brand-mark", "SG" }
                        div { class: "brand-copy",
                            span { class: "brand-kicker", "Vector Workspace" }
                            h1 { class: "brand-title", "Sivgo Studio" }
                            div { class: "doc-strip",
                                p { class: "doc-title", "{document_name}" }
                                span { class: document_badge_class, "{document_state}" }
                                span { class: "doc-badge", "{fullscreen_label}" }
                            }
                        }
                    }
                    p { class: "brand-description",
                        "A cleaner desktop shell for focused SVG editing, stronger hierarchy, and inspector-first refinement."
                    }
                }

                div { class: "topbar-actions",
                    div { class: "action-row",
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

                    div { class: "metric-grid",
                        div { class: selection_metric_class,
                            span { class: "metric-label", "Selection" }
                            strong { class: "metric-value", "{selection_metric}" }
                            span { class: "metric-copy", "{selection_summary}" }
                        }
                        div { class: "metric-card",
                            span { class: "metric-label", "Structure" }
                            strong { class: "metric-value", "{object_count}" }
                            span { class: "metric-copy", "{structure_summary}" }
                        }
                        div { class: "metric-card",
                            span { class: "metric-label", "Viewport" }
                            strong { class: "metric-value", "{zoom_pct}" }
                            span { class: "metric-copy", "{display_mode_note}" }
                        }
                    }
                }

                div { class: "topbar-side",
                    div { class: "session-card",
                        div { class: "session-tool-mark", "{active_tool_glyph}" }
                        div { class: "meta-stack",
                            span { class: "meta-kicker", "Active Tool" }
                            strong { class: "session-title", "{active_tool_name}" }
                            p { class: "session-copy", "{active_tool_copy}" }
                        }
                    }
                    div { class: "topbar-meta",
                        span { class: "top-chip active", "{display_mode_name}" }
                        span { class: "top-chip", "{selection_state}" }
                        span { class: "top-chip", "{history_state}" }
                        span { class: "top-chip", "{fullscreen_label}" }
                        button {
                            class: "quick-btn subtle",
                            onclick: {
                                let desktop = desktop.clone();
                                move |_| handle_editor_action("view.fullscreen", engine, render_version, file_path, is_fullscreen, desktop.clone())
                            },
                            "{fullscreen_button_label}"
                        }
                    }
                }
            }

            div { class: "workspace",
                aside { class: "left-rail",
                    div { class: "rail-head",
                        span { class: "rail-eyebrow", "Tool Rail" }
                        h2 { class: "rail-title", "{active_tool_name}" }
                        p { class: "rail-copy", "{active_tool_copy}" }
                    }
                    div { class: "rail-spotlight",
                        div { class: "rail-spotlight-mark", "{active_tool_glyph}" }
                        div { class: "meta-stack",
                            span { class: "rail-spotlight-label", "Shortcut" }
                            strong { class: "rail-spotlight-value", "{active_tool_shortcut}" }
                            p { class: "rail-copy", "Switch tools fast without leaving the canvas or interrupting the current flow." }
                        }
                    }
                    Toolbar {
                        active_tool: active_tool_id,
                        on_tool_change: move |id: ToolId| {
                            engine.write().tools.switch_to(id);
                            active_tool.set(id);
                        },
                    }
                    div { class: "rail-footer",
                        span { class: "rail-footer-label", "Flow" }
                        p { class: "rail-copy", "Keep creation in the rail, refinement in the inspector, and file actions in the workspace bar." }
                    }
                }

                section { class: "canvas-stage",
                    div { class: "stage-bar",
                        div {
                            span { class: "stage-kicker", "Stage" }
                            h2 { class: "stage-title", "{document_name}" }
                            p { class: "stage-copy", "{document_state_copy}" }
                        }
                        div { class: "stage-actions",
                            div { class: "stage-chip-row",
                                span { class: "stage-chip active", "{display_mode_name}" }
                                span { class: "stage-chip", "{selection_summary}" }
                                span { class: "stage-chip", "{structure_summary}" }
                            }
                            div { class: "stage-mode-buttons",
                                button {
                                    class: if display_mode == DisplayMode::Normal { "quick-btn active" } else { "quick-btn" },
                                    onclick: {
                                        let desktop = desktop.clone();
                                        move |_| handle_editor_action("view.display_mode_normal", engine, render_version, file_path, is_fullscreen, desktop.clone())
                                    },
                                    "Render"
                                }
                                button {
                                    class: if display_mode == DisplayMode::Outline { "quick-btn active" } else { "quick-btn" },
                                    onclick: {
                                        let desktop = desktop.clone();
                                        move |_| handle_editor_action("view.display_mode_outline", engine, render_version, file_path, is_fullscreen, desktop.clone())
                                    },
                                    "Outline"
                                }
                                button {
                                    class: if display_mode == DisplayMode::NoFilters { "quick-btn active" } else { "quick-btn" },
                                    onclick: {
                                        let desktop = desktop.clone();
                                        move |_| handle_editor_action("view.display_mode_no_filters", engine, render_version, file_path, is_fullscreen, desktop.clone())
                                    },
                                    "No Filters"
                                }
                            }
                        }
                    }
                    div { class: "canvas-frame",
                        div { class: "canvas-hud top-left",
                            span { class: "canvas-hud-kicker", "Document" }
                            strong { class: "canvas-hud-title", "{document_name}" }
                            span { class: "canvas-hud-copy", "{document_state}" }
                        }
                        div { class: "canvas-hud top-right",
                            span { class: "canvas-hud-kicker", "Stage Stats" }
                            div { class: "canvas-stat-grid",
                                div { class: "canvas-stat",
                                    span { class: "canvas-stat-label", "Objects" }
                                    strong { class: "canvas-stat-value", "{object_count}" }
                                }
                                div { class: "canvas-stat",
                                    span { class: "canvas-stat-label", "Layers" }
                                    strong { class: "canvas-stat-value", "{layer_count}" }
                                }
                                div { class: "canvas-stat",
                                    span { class: "canvas-stat-label", "Pages" }
                                    strong { class: "canvas-stat-value", "{surface_count}" }
                                }
                                div { class: "canvas-stat",
                                    span { class: "canvas-stat-label", "Zoom" }
                                    strong { class: "canvas-stat-value", "{zoom_pct}" }
                                }
                            }
                        }
                        div { class: "canvas-hud bottom-left",
                            span { class: "canvas-hud-kicker", "Guide" }
                            strong { class: "canvas-hud-title", "{active_tool_name}" }
                            span { class: "canvas-hud-copy", "{stage_hint}" }
                        }
                        div { class: "canvas-hud bottom-right",
                            span { class: "canvas-hud-kicker", "Current Paint" }
                            div { class: "canvas-swatch-row",
                                span { class: "canvas-swatch", style: fill_swatch_style.clone() }
                                span { class: "canvas-swatch", style: stroke_swatch_style.clone() }
                            }
                            span { class: "canvas-hud-copy", "{paint_summary}" }
                        }
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
                    div { class: "stage-footer",
                        span { class: "stage-footer-item", "Canvas {CANVAS_WIDTH} x {CANVAS_HEIGHT}" }
                        span { class: "stage-footer-item", "{structure_summary}" }
                        span { class: "stage-footer-item", "{history_state}" }
                        span { class: "stage-footer-item", "{document_state}" }
                    }
                }

                aside { class: "inspector-shell",
                    div { class: "inspector-head",
                        span { class: "inspector-kicker", "Inspector" }
                        h2 { class: "inspector-title", "Appearance & Layers" }
                        p { class: "inspector-copy", "Live paint controls, structure management, and mode feedback stay anchored on the right." }
                    }
                    div { class: "inspector-overview",
                        div { class: "inspector-card",
                            span { class: "inspector-card-label", "Selection Paint" }
                            strong { class: "inspector-card-value", "{selection_preview_title}" }
                            div { class: "inspector-swatch-row",
                                span { class: "inspector-swatch", style: fill_swatch_style.clone() }
                                span { class: "inspector-swatch", style: stroke_swatch_style.clone() }
                            }
                            p { class: "inspector-card-copy", "{paint_summary}" }
                        }
                        div { class: "inspector-card",
                            span { class: "inspector-card-label", "Document Structure" }
                            strong { class: "inspector-card-value", "{layer_summary}" }
                            p { class: "inspector-card-copy", "{object_count} objects / {page_summary}" }
                        }
                        div { class: "inspector-card wide",
                            span { class: "inspector-card-label", "Editing Mode" }
                            strong { class: "inspector-card-value", "{active_tool_name} / {display_mode_name}" }
                            p { class: "inspector-card-copy", "{display_mode_note}" }
                        }
                    }
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
