//! Menu bar component backed by the shared menu model.
//!
//! The desktop native menu and the Dioxus in-window menu both use
//! `menu_model`, so unsupported actions are disabled consistently.

use dioxus::prelude::*;

use crate::menu_model::{default_menu_bar_with_state, MenuItem};

/// The top menu bar with dropdown menus for every editor category.
///
/// Clicking an enabled menu item emits its action id (for example
/// `"file.save"` or `"edit.undo"`) through the `on_action` handler. The parent
/// maps that action string to editor operations.
#[component]
pub fn MenuBar(
    on_action: EventHandler<String>,
    #[props(default = false)] can_undo: bool,
    #[props(default = false)] can_redo: bool,
    #[props(default = false)] has_selection: bool,
) -> Element {
    let mut open_menu = use_signal(|| Option::<usize>::None);
    let menus = default_menu_bar_with_state(can_undo, can_redo, has_selection);

    rsx! {
        div { class: "menu-bar",
            for (idx, menu) in menus.iter().enumerate() {
                div {
                    key: "{menu.label}-{idx}",
                    class: "menu-dropdown",
                    button {
                        class: if *open_menu.read() == Some(idx) { "menu-top-btn active" } else { "menu-top-btn" },
                        onclick: move |_| {
                            let current = *open_menu.read();
                            if current == Some(idx) {
                                open_menu.set(None);
                            } else {
                                open_menu.set(Some(idx));
                            }
                        },
                        onmouseenter: move |_| {
                            if open_menu.read().is_some() {
                                open_menu.set(Some(idx));
                            }
                        },
                        "{menu.label}"
                    }

                    if *open_menu.read() == Some(idx) {
                        div { class: "menu-dropdown-panel",
                            for (item_idx, item) in menu.children.iter().enumerate() {
                                {render_menu_item(item, item_idx, on_action, open_menu)}
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_menu_item(
    item: &MenuItem,
    item_idx: usize,
    on_action: EventHandler<String>,
    mut open_menu: Signal<Option<usize>>,
) -> Element {
    if item.separator {
        return rsx! { div { key: "{item_idx}", class: "menu-sep" } };
    }

    let Some(action) = item.action.clone() else {
        return rsx! {
            button {
                key: "{item_idx}",
                class: "menu-item",
                disabled: true,
                span { class: "menu-item-label", "{item.label}" }
            }
        };
    };

    let shortcut = item.shortcut.as_deref().unwrap_or_default();
    let is_enabled = item.enabled;

    rsx! {
        button {
            key: "{item_idx}",
            class: "menu-item",
            disabled: !is_enabled,
            onclick: move |_| {
                if is_enabled {
                    on_action.call(action.clone());
                    open_menu.set(None);
                }
            },
            span { class: "menu-item-label", "{item.label}" }
            if !shortcut.is_empty() {
                span { class: "menu-item-shortcut", "{shortcut}" }
            }
        }
    }
}
