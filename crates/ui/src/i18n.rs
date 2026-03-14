//! Internationalization (i18n) framework.
//!
//! Provides a simple key-based translation system. Translation strings
//! are loaded from a `HashMap<String, String>` (one per locale). A
//! fallback locale (English) is always available.

use std::collections::HashMap;

/// The translation catalog for a single locale.
#[derive(Debug, Clone)]
pub struct Locale {
    /// BCP-47 language tag (e.g. "en", "zh-CN", "ja").
    pub tag: String,
    /// Human-readable name of the language (e.g. "English", "中文").
    pub display_name: String,
    /// Key → translated string.
    pub strings: HashMap<String, String>,
}

/// The i18n manager holding all loaded locales.
#[derive(Debug, Clone)]
pub struct I18n {
    locales: Vec<Locale>,
    active: usize,
    fallback: usize,
}

impl I18n {
    /// Create a new i18n manager with English as the default and fallback.
    pub fn new() -> Self {
        let en = build_english();
        Self {
            locales: vec![en],
            active: 0,
            fallback: 0,
        }
    }

    /// Add a locale to the manager.
    pub fn add_locale(&mut self, locale: Locale) {
        self.locales.push(locale);
    }

    /// Set the active locale by tag. Returns `false` if the tag is not found.
    pub fn set_locale(&mut self, tag: &str) -> bool {
        if let Some(idx) = self.locales.iter().position(|l| l.tag == tag) {
            self.active = idx;
            true
        } else {
            false
        }
    }

    /// Get the active locale tag.
    pub fn active_tag(&self) -> &str {
        &self.locales[self.active].tag
    }

    /// List all available locale tags.
    pub fn available_locales(&self) -> Vec<(&str, &str)> {
        self.locales
            .iter()
            .map(|l| (l.tag.as_str(), l.display_name.as_str()))
            .collect()
    }

    /// Look up a translated string by key. Falls back to English if the
    /// key is missing in the active locale.
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        let active = &self.locales[self.active];
        if let Some(s) = active.strings.get(key) {
            return s;
        }
        let fallback = &self.locales[self.fallback];
        if let Some(s) = fallback.strings.get(key) {
            return s;
        }
        key
    }

    /// Format a translated string with positional arguments.
    ///
    /// Placeholders are `{0}`, `{1}`, etc.
    pub fn tf(&self, key: &str, args: &[&str]) -> String {
        let template = self.t(key);
        let mut result = template.to_string();
        for (i, arg) in args.iter().enumerate() {
            result = result.replace(&format!("{{{}}}", i), arg);
        }
        result
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the English locale with all UI strings.
fn build_english() -> Locale {
    let mut s = HashMap::new();

    // File menu
    s.insert("menu.file".into(), "File".into());
    s.insert("menu.file.new".into(), "New".into());
    s.insert("menu.file.open".into(), "Open...".into());
    s.insert("menu.file.open_recent".into(), "Open Recent".into());
    s.insert("menu.file.save".into(), "Save".into());
    s.insert("menu.file.save_as".into(), "Save As...".into());
    s.insert("menu.file.export".into(), "Export PNG Image...".into());
    s.insert("menu.file.import".into(), "Import...".into());
    s.insert("menu.file.print".into(), "Print...".into());
    s.insert("menu.file.close".into(), "Close".into());
    s.insert("menu.file.quit".into(), "Quit".into());

    // Edit menu
    s.insert("menu.edit".into(), "Edit".into());
    s.insert("menu.edit.undo".into(), "Undo".into());
    s.insert("menu.edit.redo".into(), "Redo".into());
    s.insert("menu.edit.cut".into(), "Cut".into());
    s.insert("menu.edit.copy".into(), "Copy".into());
    s.insert("menu.edit.paste".into(), "Paste".into());
    s.insert("menu.edit.paste_in_place".into(), "Paste in Place".into());
    s.insert("menu.edit.duplicate".into(), "Duplicate".into());
    s.insert("menu.edit.delete".into(), "Delete".into());
    s.insert("menu.edit.select_all".into(), "Select All".into());
    s.insert("menu.edit.find".into(), "Find/Replace...".into());
    s.insert("menu.edit.preferences".into(), "Preferences...".into());

    // View menu
    s.insert("menu.view".into(), "View".into());
    s.insert("menu.view.zoom_in".into(), "Zoom In".into());
    s.insert("menu.view.zoom_out".into(), "Zoom Out".into());
    s.insert("menu.view.zoom_100".into(), "Zoom 1:1".into());
    s.insert("menu.view.zoom_fit_page".into(), "Zoom to Fit Page".into());
    s.insert(
        "menu.view.zoom_fit_drawing".into(),
        "Zoom to Fit Drawing".into(),
    );
    s.insert("menu.view.grid".into(), "Page Grid".into());
    s.insert("menu.view.guides".into(), "Guides".into());
    s.insert("menu.view.rulers".into(), "Rulers".into());
    s.insert("menu.view.fullscreen".into(), "Fullscreen".into());

    // Object menu
    s.insert("menu.object".into(), "Object".into());
    s.insert("menu.object.group".into(), "Group".into());
    s.insert("menu.object.ungroup".into(), "Ungroup".into());
    s.insert("menu.object.raise".into(), "Raise".into());
    s.insert("menu.object.lower".into(), "Lower".into());
    s.insert("menu.object.to_front".into(), "Raise to Top".into());
    s.insert("menu.object.to_back".into(), "Lower to Bottom".into());
    s.insert("menu.object.flip_h".into(), "Flip Horizontal".into());
    s.insert("menu.object.flip_v".into(), "Flip Vertical".into());

    // Path menu
    s.insert("menu.path".into(), "Path".into());
    s.insert("menu.path.object_to_path".into(), "Object to Path".into());
    s.insert("menu.path.stroke_to_path".into(), "Stroke to Path".into());
    s.insert("menu.path.union".into(), "Union".into());
    s.insert("menu.path.intersection".into(), "Intersection".into());
    s.insert("menu.path.difference".into(), "Difference".into());
    s.insert("menu.path.exclusion".into(), "Exclusion".into());
    s.insert("menu.path.simplify".into(), "Simplify".into());

    // Tools
    s.insert("tool.select".into(), "Select and transform objects".into());
    s.insert("tool.node".into(), "Edit paths by nodes".into());
    s.insert("tool.rect".into(), "Create rectangles and squares".into());
    s.insert("tool.ellipse".into(), "Create circles and ellipses".into());
    s.insert("tool.pen".into(), "Draw Bezier curves".into());
    s.insert("tool.pencil".into(), "Draw freehand lines".into());
    s.insert("tool.text".into(), "Create and edit text objects".into());
    s.insert("tool.gradient".into(), "Create and edit gradients".into());
    s.insert("tool.dropper".into(), "Pick colors from the canvas".into());
    s.insert("tool.zoom".into(), "Zoom in or out".into());
    s.insert("tool.measure".into(), "Measure distances and angles".into());

    // Panels
    s.insert("panel.layers".into(), "Layers".into());
    s.insert("panel.objects".into(), "Objects".into());
    s.insert("panel.fill_stroke".into(), "Fill and Stroke".into());
    s.insert("panel.align".into(), "Align and Distribute".into());
    s.insert("panel.transform".into(), "Transform".into());
    s.insert("panel.xml_editor".into(), "XML Editor".into());
    s.insert("panel.text".into(), "Text and Font".into());
    s.insert("panel.export".into(), "Export".into());

    // Status bar
    s.insert("status.zoom".into(), "Zoom: {0}%".into());
    s.insert("status.objects".into(), "{0} objects".into());
    s.insert("status.selected".into(), "{0} selected".into());
    s.insert("status.cursor".into(), "({0}, {1})".into());

    // Dialogs
    s.insert("dialog.ok".into(), "OK".into());
    s.insert("dialog.cancel".into(), "Cancel".into());
    s.insert("dialog.apply".into(), "Apply".into());
    s.insert("dialog.close".into(), "Close".into());
    s.insert(
        "dialog.save_changes".into(),
        "Save changes to \"{0}\"?".into(),
    );
    s.insert(
        "dialog.unsaved".into(),
        "This document has unsaved changes.".into(),
    );

    Locale {
        tag: "en".into(),
        display_name: "English".into(),
        strings: s,
    }
}

/// Build a Chinese (Simplified) locale.
pub fn build_chinese_simplified() -> Locale {
    let mut s = HashMap::new();

    s.insert("menu.file".into(), "文件".into());
    s.insert("menu.file.new".into(), "新建".into());
    s.insert("menu.file.open".into(), "打开...".into());
    s.insert("menu.file.save".into(), "保存".into());
    s.insert("menu.file.save_as".into(), "另存为...".into());
    s.insert("menu.file.export".into(), "导出PNG图像...".into());
    s.insert("menu.file.import".into(), "导入...".into());
    s.insert("menu.file.quit".into(), "退出".into());

    s.insert("menu.edit".into(), "编辑".into());
    s.insert("menu.edit.undo".into(), "撤销".into());
    s.insert("menu.edit.redo".into(), "重做".into());
    s.insert("menu.edit.cut".into(), "剪切".into());
    s.insert("menu.edit.copy".into(), "复制".into());
    s.insert("menu.edit.paste".into(), "粘贴".into());
    s.insert("menu.edit.delete".into(), "删除".into());
    s.insert("menu.edit.select_all".into(), "全选".into());

    s.insert("menu.view".into(), "视图".into());
    s.insert("menu.object".into(), "对象".into());
    s.insert("menu.path".into(), "路径".into());

    s.insert("tool.select".into(), "选择和变换对象".into());
    s.insert("tool.node".into(), "按节点编辑路径".into());
    s.insert("tool.rect".into(), "创建矩形和正方形".into());
    s.insert("tool.text".into(), "创建和编辑文本对象".into());

    s.insert("panel.layers".into(), "图层".into());
    s.insert("panel.objects".into(), "对象".into());
    s.insert("panel.fill_stroke".into(), "填充和描边".into());

    s.insert("dialog.ok".into(), "确定".into());
    s.insert("dialog.cancel".into(), "取消".into());

    Locale {
        tag: "zh-CN".into(),
        display_name: "中文 (简体)".into(),
        strings: s,
    }
}

/// Build a Japanese locale.
pub fn build_japanese() -> Locale {
    let mut s = HashMap::new();

    s.insert("menu.file".into(), "ファイル".into());
    s.insert("menu.file.new".into(), "新規".into());
    s.insert("menu.file.open".into(), "開く...".into());
    s.insert("menu.file.save".into(), "保存".into());
    s.insert("menu.file.quit".into(), "終了".into());

    s.insert("menu.edit".into(), "編集".into());
    s.insert("menu.edit.undo".into(), "元に戻す".into());
    s.insert("menu.edit.redo".into(), "やり直し".into());

    s.insert("dialog.ok".into(), "OK".into());
    s.insert("dialog.cancel".into(), "キャンセル".into());

    Locale {
        tag: "ja".into(),
        display_name: "日本語".into(),
        strings: s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_default() {
        let i18n = I18n::new();
        assert_eq!(i18n.active_tag(), "en");
        assert_eq!(i18n.t("menu.file"), "File");
        assert_eq!(i18n.t("menu.edit.undo"), "Undo");
    }

    #[test]
    fn switch_locale() {
        let mut i18n = I18n::new();
        i18n.add_locale(build_chinese_simplified());
        assert!(i18n.set_locale("zh-CN"));
        assert_eq!(i18n.t("menu.file"), "文件");
        assert_eq!(i18n.t("menu.edit.undo"), "撤销");
    }

    #[test]
    fn fallback_to_english() {
        let mut i18n = I18n::new();
        i18n.add_locale(build_chinese_simplified());
        i18n.set_locale("zh-CN");
        // Key exists in English but not Chinese
        assert_eq!(i18n.t("menu.file.print"), "Print...");
    }

    #[test]
    fn missing_key_returns_key() {
        let i18n = I18n::new();
        assert_eq!(i18n.t("nonexistent.key"), "nonexistent.key");
    }

    #[test]
    fn format_with_args() {
        let i18n = I18n::new();
        let result = i18n.tf("status.zoom", &["150"]);
        assert_eq!(result, "Zoom: 150%");

        let result = i18n.tf("status.cursor", &["42.5", "100.3"]);
        assert_eq!(result, "(42.5, 100.3)");
    }

    #[test]
    fn available_locales_list() {
        let mut i18n = I18n::new();
        i18n.add_locale(build_chinese_simplified());
        i18n.add_locale(build_japanese());
        let locales = i18n.available_locales();
        assert_eq!(locales.len(), 3);
        assert_eq!(locales[0].0, "en");
        assert_eq!(locales[1].0, "zh-CN");
        assert_eq!(locales[2].0, "ja");
    }

    #[test]
    fn set_nonexistent_locale_fails() {
        let mut i18n = I18n::new();
        assert!(!i18n.set_locale("xx-XX"));
        assert_eq!(i18n.active_tag(), "en");
    }
}
