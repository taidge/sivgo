//! Desktop menus use intrinsic menu controls for layout, focus and submenus.
use super::*;

pub(super) fn editor_menu(state: AppState) -> impl Widget {
    widget(move |scope| {
        let availability = state.render_version.map(move |_| menu_availability(state));
        let mut bar = sapid::menubar("editor")
            .theme(sapid::Theme::dark())
            .size(sapid::Size::ExtraSmall)
            .class("glory-menu-bar")
            .name("Main menu")
            .style(
                row_style()
                    .gap(1.0)
                    .padding(Edges {
                        left: 6.0,
                        right: 6.0,
                        top: 2.0,
                        bottom: 2.0,
                    })
                    .border_width(Edges::all(0.0))
                    .radius(0.0),
            )
            .value(
                state
                    .menu_open
                    .map(|open| open.map(|index| format!("root-{index}"))),
            )
            .on_value_change(move |key| {
                state.menu_open.revise(|open| {
                    *open = key.and_then(|key| key.strip_prefix("root-")?.parse().ok());
                });
            });
        for (index, item) in default_menu_bar_with_state(true, true, true)
            .into_iter()
            .enumerate()
        {
            let mut root = sapid::menubar_menu(format!("root-{index}"))
                .label(item.label)
                .width(296.0);
            for entry in entries(scope, &item.children, state, availability) {
                root = root.item(entry);
            }
            bar = bar.menu(root);
        }
        bar.show_in(scope)
    })
}

fn entries(
    scope: &Scope,
    items: &[MenuNode],
    state: AppState,
    availability: glory::Bond<HashMap<String, bool>>,
) -> Vec<sapid::MenuEntry> {
    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            if item.separator {
                return Some(sapid::MenuEntry::Separator(format!("separator-{index}")));
            }
            if !item.children.is_empty() {
                let mut submenu = sapid::menu_submenu(format!("submenu-{index}"))
                    .label(item.label.clone())
                    .left_section(icon_gutter())
                    .style(item_style())
                    .width(276.0);
                for entry in entries(scope, &item.children, state, availability) {
                    submenu = submenu.item(entry);
                }
                return Some(submenu.into());
            }
            let action = item.action.clone()?;
            if action == "file.open_recent" {
                return Some(state.files.recent_submenu(state).into());
            }
            let enabled_key = action.clone();
            let enabled = scope.bond(move || {
                availability.with(|values| values.get(&enabled_key).copied().unwrap_or(false))
            });
            let mut entry = if let Some(default) = item.checked {
                let key = action.clone();
                sapid::menu_checkbox(format!("action-{index}")).checked(state.render_version.map(
                    move |_| {
                        let engine = state.engine.peek();
                        match key.as_str() {
                            "view.toggle_grid" => grid_visible(&engine),
                            "view.toggle_guides" => engine.snapping.prefs.snap_to_guides,
                            "view.toggle_snapping" => engine.snapping.prefs.any_enabled(),
                            _ => default,
                        }
                    },
                ))
            } else {
                let mut entry = sapid::menu_item(format!("action-{index}"));
                if let Some(icon) = action_icon(&action) {
                    entry = entry.left_section(icon.widget());
                } else {
                    entry = entry.left_section(icon_gutter());
                }
                entry.right_section(
                    text()
                        .content(item.shortcut.clone().unwrap_or_default())
                        .class("editor-menu-shortcut")
                        .style(UiStyle {
                            color: Some(glory::style::Color(143, 150, 164, 255)),
                            font_size: Some(11.0),
                            ..UiStyle::new().white_space(glory::style::WhiteSpace::NoWrap)
                        }),
                )
            };
            entry = entry
                .label(item.label.clone())
                .enabled(enabled)
                .style(item_style())
                .on_activate(move || state.action(&action));
            Some(entry.into())
        })
        .collect()
}

fn icon_gutter() -> impl Widget {
    text()
        .content("")
        .style(UiStyle::new().width(UiLength::Px(16.0)))
}

fn item_style() -> UiStyle {
    UiStyle::new()
        .min_height(UiLength::Px(28.0))
        .padding(Edges {
            top: 4.0,
            bottom: 4.0,
            left: 8.0,
            right: 8.0,
        })
}

fn action_icon(action: &str) -> Option<EditorIcon> {
    Some(match action {
        "file.new" => EditorIcon::New,
        "file.open" => EditorIcon::Open,
        "file.save" | "file.save_as" | "file.save_copy" => EditorIcon::Save,
        "file.export_png" | "file.export_svg" => EditorIcon::Export,
        "edit.undo" => EditorIcon::Undo,
        "edit.redo" => EditorIcon::Redo,
        "edit.preferences" => EditorIcon::Settings,
        _ => return None,
    })
}
