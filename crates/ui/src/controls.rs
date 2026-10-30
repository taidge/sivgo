//! Shared owner-scoped controls for the editor's semantic panels.
use glory::prelude::*;

pub(crate) fn editor_theme() -> sapid::Theme {
    sapid::Theme::dark()
}

fn field_row(scope: &Scope, label: &str) -> Result<Scope> {
    let row = panel(scope, "compact-field", "")?;
    text()
        .content(label.to_owned())
        .style(Style::new().width(Length::Px(96.0)))
        .show_in(&row)?;
    Ok(row)
}

pub(crate) fn panel(scope: &Scope, class: &str, title: &str) -> Result<Scope> {
    let node = scope.cage(None);
    let horizontal = matches!(
        class,
        "tool-controls" | "status-bar" | "color-bar" | "compact-field"
    );
    let mut style = Style::new()
        .display(Display::Flex)
        .flex_direction(if horizontal {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        })
        .flex_wrap(glory::style::FlexWrap::NoWrap)
        .gap(if class == "toolbar-group" { 1.0 } else { 6.0 });
    style.shrink = Some(0.0);
    if horizontal {
        style.align = Some(glory::style::Align::Center);
    }
    if matches!(class, "toolbar" | "toolbar-group") {
        style.width = Some(Length::Px(40.0));
        style.min_width = Some(Length::Px(40.0));
        style.padding = Some(glory::style::Edges::all(0.0));
        style.align = Some(glory::style::Align::Center);
    }
    if class == "color-bar" {
        style.height = Some(Length::Px(32.0));
        style.padding = Some(glory::style::Edges::all(2.0));
    }
    if class == "status-bar" {
        style.height = Some(Length::Px(24.0));
        style.padding = Some(glory::style::Edges::all(2.0));
        style.gap = Some(12.0);
    }
    col()
        .class(class.to_owned())
        .node_ref(node)
        .style(style)
        .show_in(scope)?;
    let child = scope.child_in(node.get().ok_or(glory::ui::Error::Gone)?)?;
    scope.retain(child.clone())?;
    if !title.is_empty() {
        let mut heading_style = Style::new();
        heading_style.color = Some(editor_theme().foreground);
        heading_style.font_size = Some(13.0);
        text()
            .class("panel-header")
            .content(title.to_owned())
            .style(heading_style)
            .show_in(&child)?;
    }
    Ok(child)
}

pub(crate) fn text_field(
    scope: &Scope,
    label: &str,
    value: String,
    handler: impl FnMut(String) + 'static,
) -> Result {
    let row = field_row(scope, label)?;
    sapid::text_field()
        .theme(editor_theme())
        .size(sapid::Size::ExtraSmall)
        .style(
            Style::new()
                .width(Length::Px(140.0))
                .min_width(Length::Px(0.0)),
        )
        .name(label.to_owned())
        .default_value(value)
        .on_commit(handler)
        .show_in(&row)
}

pub(crate) fn number_field(
    scope: &Scope,
    label: &str,
    value: f64,
    handler: impl FnMut(f64) + 'static,
) -> Result {
    let row = field_row(scope, label)?;
    number_field_width(&row, label, value, 112.0, handler)
}

pub(crate) fn compact_number_field(
    scope: &Scope,
    label: &str,
    value: f64,
    handler: impl FnMut(f64) + 'static,
) -> Result {
    let field = panel(scope, "compact-field", "")?;
    text().content(label.to_owned()).show_in(&field)?;
    number_field_width(&field, label, value, 72.0, handler)
}

fn number_field_width(
    scope: &Scope,
    label: &str,
    value: f64,
    width: f64,
    mut handler: impl FnMut(f64) + 'static,
) -> Result {
    sapid::number_input()
        .theme(editor_theme())
        .name(label.to_owned())
        .size(sapid::Size::ExtraSmall)
        .controls(false)
        .style(Style::new().width(Length::Px(width)))
        .default_value(sapid::NumberValue::Text(value.to_string()))
        .on_commit(move |value| {
            if let sapid::NumberParse::Complete(value) = value.parse(sapid::NumberLocale::EnUs) {
                let value = value.to_f64();
                if value.is_finite() {
                    handler(value);
                }
            }
        })
        .show_in(scope)
}

pub(crate) fn control_key(label: &str) -> String {
    let mut key = String::from("sivgo-");
    key.extend(label.chars().take(80).map(|c| {
        if c.is_ascii_alphanumeric() {
            c.to_ascii_lowercase()
        } else {
            '-'
        }
    }));
    key
}

/// Keep picker drags local so an editor snapshot rebuild cannot dismiss the
/// popup. Closing commits its final valid color; text edits commit normally.
pub(crate) fn color_field(
    scope: &Scope,
    label: &str,
    value: String,
    handler: impl Fn(String) + 'static,
) -> Result {
    let draft = scope.cage(value.clone());
    let committed = scope.cage(value.clone());
    let opened = scope.cage(false);
    let commit_color = std::rc::Rc::new(move |value: String| {
        if sivgo_dom::Color::parse(&value).is_some() && committed.get() != value {
            committed.revise(|last| *last = value.clone());
            handler(value);
        }
    });
    let commit_text = commit_color.clone();
    text().content(label.to_owned()).show_in(scope)?;
    sapid::color_input(control_key(label))
        .theme(editor_theme())
        .size(sapid::Size::ExtraSmall)
        .style(
            Style::new()
                .width(Length::Px(230.0))
                .min_width(Length::Px(0.0)),
        )
        .name(label.to_owned())
        .default_value(value)
        .show_picker(true)
        .on_value_change(move |value| draft.revise(|draft| *draft = value))
        .on_commit(move |value| {
            draft.revise(|draft| *draft = value.clone());
            if !opened.get() {
                commit_text(value);
            }
        })
        .on_open_change(move |open| {
            opened.revise(|opened| *opened = open);
            if !open {
                commit_color(draft.get());
            }
        })
        .show_in(scope)
}

pub(crate) fn choice_field(
    scope: &Scope,
    label: &str,
    value: String,
    options: &[&str],
    handler: impl FnMut(String) + 'static,
) -> Result {
    let choices: Vec<(String, String)> = options
        .iter()
        .map(|v| ((*v).to_owned(), (*v).to_owned()))
        .collect();
    let selected = choices
        .iter()
        .find(|(_, label)| label.eq_ignore_ascii_case(&value))
        .map(|(key, _)| key.clone())
        .unwrap_or(value);
    choice_options(scope, label, selected, choices, handler)
}

/// A compact choice control built from portable buttons. Glory's current
/// native host does not implement HTML Select or combobox semantics.
pub(crate) fn choice_options(
    scope: &Scope,
    label: &str,
    value: String,
    options: Vec<(String, String)>,
    handler: impl FnMut(String) + 'static,
) -> Result {
    let open = scope.cage(false);
    let selected = scope.cage(value);
    let handler = std::rc::Rc::new(std::cell::RefCell::new(handler));
    let labels = options.clone();
    let title = label.to_owned();
    sapid::button()
        .theme(editor_theme())
        .name(format!("Choose {label}"))
        .size(sapid::Size::ExtraSmall)
        .variant(sapid::ButtonVariant::Secondary)
        .class("choice-trigger")
        .label(selected.map(move |key| {
            format!(
                "{}: {}",
                title,
                labels
                    .iter()
                    .find(|(value, _)| value == key)
                    .map(|(_, label)| label.as_str())
                    .unwrap_or(key)
            )
        }))
        .on_press(move || open.revise(|value| *value = !*value))
        .show_in(scope)?;
    when(open, move || {
        let options = options.clone();
        let handler = handler.clone();
        glory::ui::widget(move |scope| {
            let scope = panel(scope, "choice-options", "")?;
            for (key, label) in &options {
                let key = key.clone();
                let handler = handler.clone();
                sapid::button()
                    .theme(editor_theme())
                    .label(label.clone())
                    .size(sapid::Size::ExtraSmall)
                    .variant(if selected.get() == key {
                        sapid::ButtonVariant::Primary
                    } else {
                        sapid::ButtonVariant::Secondary
                    })
                    .class(if selected.get() == key {
                        "choice-option active"
                    } else {
                        "choice-option"
                    })
                    .on_press(move || {
                        selected.revise(|value| *value = key.clone());
                        open.revise(|value| *value = false);
                        (handler.borrow_mut())(key.clone());
                    })
                    .show_in(&scope)?;
            }
            Ok(())
        })
    })
    .show_in(scope)
}

pub(crate) fn action_button(
    scope: &Scope,
    label: &str,
    active: bool,
    handler: impl FnMut() + 'static,
) -> Result {
    sapid::button()
        .theme(editor_theme())
        .label(label.to_owned())
        .size(sapid::Size::ExtraSmall)
        .variant(if active {
            sapid::ButtonVariant::Primary
        } else {
            sapid::ButtonVariant::Secondary
        })
        .class(if active {
            "action-btn active"
        } else {
            "action-btn"
        })
        .on_press(handler)
        .show_in(scope)
}
