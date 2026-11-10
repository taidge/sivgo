//! Desktop drawing lifecycle and the File menu's persistent state.

use std::{
    cell::RefCell,
    io,
    rc::Rc,
    time::{Duration, Instant},
};

use rfd::MessageDialogResult;
use sivgo_io::{recent_files::RecentFiles, IoError};

use super::*;

#[derive(Clone, Copy)]
pub(super) struct FileSession {
    config_dir: Cage<Option<PathBuf>>,
    recent: Signal<RecentFiles>,
    recovery: Cage<Rc<RefCell<recovery::Recovery>>>,
    recovered_name: Cage<Option<String>>,
    recovery_failed: Cage<bool>,
}

impl FileSession {
    pub(super) fn new(scope: &Scope, config_dir: Option<PathBuf>, capacity: usize) -> Self {
        let mut recent = config_dir
            .as_ref()
            .and_then(
                |dir| match RecentFiles::load_from_file(dir.join("recent_files.json")) {
                    Ok(recent) => Some(recent),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                    Err(error) => {
                        log::warn!("Could not read recent files: {error}");
                        None
                    }
                },
            )
            .unwrap_or_else(|| RecentFiles::new(capacity));
        recent.set_capacity(capacity);
        Self {
            config_dir: scope.cage(config_dir),
            recent: Signal::new(scope, recent),
            recovery: scope.cage(Rc::new(RefCell::new(recovery::Recovery::default()))),
            recovered_name: scope.cage(None),
            recovery_failed: scope.cage(false),
        }
    }

    pub(super) fn drawing_name(self, state: AppState) -> String {
        self.recovered_name
            .get()
            .map(|name| format!("Recovered · {name}"))
            .unwrap_or_else(|| current_document_name(state.file_path.read().as_ref()))
    }

    pub(super) fn recovery_failed(self) -> Cage<bool> {
        self.recovery_failed
    }

    pub(super) fn action(self, state: AppState, action: &str) -> bool {
        match action {
            "file.new" => {
                if confirm_discard_changes(state.engine, "Start a new drawing") {
                    let defaults = state.document_defaults.peek().clone();
                    let preferences = state.preferences_state.peek().clone();
                    self.clear_recovery();
                    let mut engine = state.engine.write();
                    engine.cancel_tool_interaction();
                    engine.new_document(defaults.width_px, defaults.height_px);
                    apply_document_defaults_to_engine(&mut engine, &defaults);
                    apply_preferences_to_engine(&mut engine, &preferences);
                    engine.mark_saved();
                    drop(engine);
                    self.document_changed(state, None, None);
                }
            }
            "file.open" => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SVG drawings", &["svg", "svgz"])
                    .pick_file()
                {
                    self.open_path(state, path);
                }
            }
            "file.save" | "file.save_as" => {
                let path = if action == "file.save" {
                    state.file_path.peek().clone()
                } else {
                    None
                };
                let path = path.or_else(|| {
                    rfd::FileDialog::new()
                        .add_filter("SVG files", &["svg"])
                        .add_filter("Compressed SVG files", &["svgz"])
                        .save_file()
                });
                if let Some(path) = path {
                    self.save_path(state, path);
                }
            }
            "file.recover_draft" => self.offer_recovery(state, true),
            _ => return false,
        }
        true
    }

    fn open_path(self, state: AppState, path: PathBuf) {
        match self.load_path(&path) {
            Ok(document) => {
                if confirm_discard_changes(state.engine, "Open another file") {
                    self.clear_recovery();
                    self.load_document(state, document, Some(path.clone()), None);
                    self.record(&path);
                }
            }
            Err(error) => {
                report_error(
                    "Could not open the drawing",
                    &format!("{}: {error}", path.display()),
                );
            }
        }
    }

    fn load_path(self, path: &Path) -> Result<sivgo_dom::SvgDocument, IoError> {
        let loaded = sivgo_io::load_svg(path);
        if matches!(&loaded, Err(IoError::Io(error)) if error.kind() == io::ErrorKind::NotFound) {
            self.recent.write().remove(&path.to_string_lossy());
            self.persist_recent();
        }
        loaded
    }

    fn save_path(self, state: AppState, path: PathBuf) {
        if save_svg_document(state.engine, &path, true) {
            self.clear_recovery();
            self.recovered_name.revise(|name| *name = None);
            state.file_path.set(Some(path.clone()));
            set_window_title(&state.desktop.get(), Some(&path));
            self.record(&path);
            bump_render(state.render_version);
        }
    }

    fn load_document(
        self,
        state: AppState,
        document: sivgo_dom::SvgDocument,
        path: Option<PathBuf>,
        recovered_name: Option<String>,
    ) {
        let mut engine = state.engine.write();
        engine.cancel_tool_interaction();
        engine.load_document(document);
        apply_preferences_to_engine(&mut engine, &state.preferences_state.peek());
        if recovered_name.is_some() {
            engine.history.mark_unsaved();
        }
        drop(engine);
        self.document_changed(state, path, recovered_name);
    }

    fn document_changed(
        self,
        state: AppState,
        path: Option<PathBuf>,
        recovered_name: Option<String>,
    ) {
        cancel_canvas_input(state);
        state.editing.revise(|editing| *editing = false);
        if let Some(name) = &recovered_name {
            state
                .desktop
                .get()
                .set_title(&format!("Recovered · {name} — Sivgo"));
        } else {
            set_window_title(&state.desktop.get(), path.as_ref());
        }
        self.recovered_name.revise(|name| *name = recovered_name);
        state.file_path.set(path);
        state.agent.reset();
        bump_render(state.render_version);
    }

    pub(super) fn close(self, state: AppState) -> bool {
        if !confirm_discard_changes(state.engine, "Close this drawing") {
            return false;
        }
        self.clear_recovery();
        true
    }

    fn record(self, path: &Path) {
        let Ok(path) = std::path::absolute(path) else {
            return;
        };
        if let Some(path) = path.to_str() {
            self.recent.write().add(path, None);
            self.persist_recent();
        }
    }

    pub(super) fn set_recent_capacity(self, capacity: usize) {
        self.recent.write().set_capacity(capacity);
        self.persist_recent();
    }

    fn persist_recent(self) {
        let Some(folder) = self.config_dir.get_untracked() else {
            return;
        };
        if let Err(error) = fs::create_dir_all(&folder).and_then(|()| {
            self.recent
                .peek()
                .save_to_file(folder.join("recent_files.json"))
        }) {
            log::warn!("Could not save recent files: {error}");
        }
    }

    pub(super) fn recent_submenu(self, state: AppState) -> sapid::MenuSubmenu {
        let mut submenu = sapid::menu_submenu("recent")
            .label("Open Recent")
            .left_section(EditorIcon::Open.widget())
            .width(296.0)
            .item(
                sapid::menu_label("empty", "No recent drawings")
                    .visible(self.recent.map(|recent| recent.entries().is_empty())),
            );
        // Mount stable slots so opening/saving files updates the menu without
        // replacing its focused controls or closing the active menu.
        for index in 0..50 {
            submenu = submenu.item(
                sapid::menu_item(format!("recent-{index}"))
                    .label(self.recent.map(move |recent| {
                        recent
                            .entries()
                            .get(index)
                            .map(|entry| {
                                Path::new(&entry.path)
                                    .file_name()
                                    .map(|name| name.to_string_lossy().chars().take(60).collect())
                                    .unwrap_or_else(|| "Recent drawing".to_owned())
                            })
                            .unwrap_or_else(|| "Recent drawing".to_owned())
                    }))
                    .name(self.recent.map(move |recent| {
                        recent
                            .entries()
                            .get(index)
                            .map(|entry| format!("Open {}", entry.path))
                            .unwrap_or_else(|| "Recent drawing".to_owned())
                    }))
                    .visible(
                        self.recent
                            .map(move |recent| recent.entries().get(index).is_some()),
                    )
                    .on_activate(move || {
                        let path = self
                            .recent
                            .peek()
                            .entries()
                            .get(index)
                            .map(|entry| entry.path.clone());
                        if let Some(path) = path {
                            self.open_path(state, PathBuf::from(path));
                        }
                    }),
            );
        }
        submenu.separator("clear-separator").item(
            sapid::menu_item("clear")
                .label("Clear recent files")
                .enabled(self.recent.map(|recent| !recent.entries().is_empty()))
                .on_activate(move || {
                    self.recent.write().clear();
                    self.persist_recent();
                }),
        )
    }

    pub(super) fn tick(self, state: AppState, now: Instant) -> Option<Instant> {
        let config_dir = self.config_dir.get_untracked()?;
        let preferences = state.preferences_state.peek();
        let enabled = preferences.auto_save;
        let interval = Duration::from_secs(preferences.auto_save_interval_secs.into());
        drop(preferences);
        if !enabled {
            self.set_recovery_failed(false);
        }
        let engine = state.engine.peek();
        if !engine.is_dirty() {
            self.clear_recovery();
            return None;
        }
        let recovery = self.recovery.get_untracked();
        let mut recovery = recovery.borrow_mut();
        let due = recovery.schedule(
            now,
            enabled,
            interval,
            true,
            *state.canvas_pointer_down.peek(),
        )?;
        if now < due {
            return Some(due);
        }
        let name = self
            .recovered_name
            .get_untracked()
            .unwrap_or_else(|| current_document_name(state.file_path.peek().as_ref()));
        match recovery.checkpoint(&config_dir.join("recovery"), now, &name, &engine.document) {
            Ok(next) => {
                self.set_recovery_failed(false);
                Some(next)
            }
            Err(error) => {
                log::warn!("Could not save recovery draft: {error}");
                self.set_recovery_failed(true);
                Some(now + interval)
            }
        }
    }

    fn set_recovery_failed(self, failed: bool) {
        if self.recovery_failed.get_untracked() != failed {
            self.recovery_failed.revise(|value| *value = failed);
        }
    }

    fn clear_recovery(self) {
        if let Err(error) = self.recovery.get_untracked().borrow_mut().reset() {
            log::warn!("Could not remove the completed recovery draft: {error}");
        }
        self.set_recovery_failed(false);
    }

    pub(super) fn offer_recovery(self, state: AppState, manual: bool) {
        let Some(folder) = self.config_dir.get_untracked() else {
            return;
        };
        let recovered = match recovery::newest(&folder.join("recovery")) {
            Ok(Some(recovered)) => recovered,
            Ok(None) => {
                if manual {
                    MessageDialog::new()
                        .set_title("Recover draft")
                        .set_description("There are no unsaved drafts to recover.")
                        .show();
                }
                return;
            }
            Err(error) => {
                if manual {
                    report_error("Could not open the recovery draft", &error.to_string());
                } else {
                    log::warn!("Could not open the recovery draft: {error}");
                }
                return;
            }
        };
        let choice = MessageDialog::new()
            .set_title("Recover unsaved drawing")
            .set_description(format!(
                "A recovery draft for {} is available. Recover it as an unsaved drawing, discard it, or leave it for later.",
                recovered.name
            ))
            .set_buttons(MessageButtons::YesNoCancelCustom("Recover".into(), "Discard draft".into(), "Later".into()))
            .show();
        match choice {
            MessageDialogResult::Custom(choice) if choice == "Discard draft" => {
                if let Err(error) = recovered.discard() {
                    report_error("Could not discard the recovery draft", &error.to_string());
                }
            }
            choice
                if (choice == MessageDialogResult::Yes
                    || matches!(&choice, MessageDialogResult::Custom(label) if label == "Recover"))
                    && confirm_discard_changes(state.engine, "Recover the draft") =>
            {
                self.restore(state, recovered);
            }
            _ => {}
        }
    }

    fn restore(self, state: AppState, recovered: recovery::Recovered) {
        self.clear_recovery();
        let (name, document) = self.recovery.get_untracked().borrow_mut().adopt(recovered);
        self.load_document(state, document, None, Some(name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_state(app: &glory::App, dir: &Path) -> AppState {
        let mut state = AppState::new(app.scope(), DesktopContext::new());
        state.files = FileSession::new(app.scope(), Some(dir.to_owned()), 10);
        state
    }

    #[test]
    fn recovered_edits_save_to_a_new_path_and_retire_the_draft() {
        let folder = tempfile::tempdir().unwrap();
        let original = folder.path().join("original.svg");
        let saved = folder.path().join("saved.svgz");
        let original_svg = "<svg xmlns='http://www.w3.org/2000/svg'/>";
        fs::write(&original, original_svg).unwrap();
        let now = Instant::now();
        let recovery_folder = folder.path().join("recovery");
        {
            let app = glory::App::memory();
            let state = app_state(&app, folder.path());
            state.files.open_path(state, original.clone());
            assert_eq!(state.files.tick(state, now), None);
            state.engine.write().set_document_title("Unsaved title");
            let due = state.files.tick(state, now).unwrap();
            assert_eq!(due, now + Duration::from_secs(300));
            assert!(!recovery_folder.exists());
            assert!(state.files.tick(state, due).is_some());
            assert!(recovery::newest(&recovery_folder).unwrap().is_none());
            // No orderly close, as if the process stopped with unsaved work.
        }
        let app = glory::App::memory();
        let state = app_state(&app, folder.path());
        state
            .files
            .restore(state, recovery::newest(&recovery_folder).unwrap().unwrap());
        assert!(state.engine.peek().is_dirty());
        assert!(
            state.file_path.peek().is_none(),
            "Save must ask for a destination"
        );
        assert_eq!(state.files.drawing_name(state), "Recovered · original.svg");
        assert_eq!(root_text(&state.engine.peek(), "title"), "Unsaved title");
        state.files.save_path(state, saved.clone());
        assert!(!state.engine.peek().is_dirty());
        assert_eq!(fs::read_to_string(&original).unwrap(), original_svg);
        assert!(
            sivgo_dom::serialize_svg(&sivgo_io::load_svg(&saved).unwrap())
                .contains("Unsaved title")
        );
        assert!(recovery::newest(&recovery_folder).unwrap().is_none());

        let recent = RecentFiles::load_from_file(folder.path().join("recent_files.json")).unwrap();
        assert_eq!(recent.entries()[0].path, saved.to_str().unwrap());
        assert_eq!(recent.entries()[1].path, original.to_str().unwrap());
    }

    #[test]
    fn file_menu_opens_recent_drawings_and_clears_persisted_entries() {
        let folder = tempfile::tempdir().unwrap();
        let drawing = folder.path().join("drawing.svg");
        let missing = folder.path().join("missing.svg");
        fs::write(&drawing, "<svg xmlns='http://www.w3.org/2000/svg'/>").unwrap();
        let app = glory::App::memory();
        let state = app_state(&app, folder.path());
        state.files.record(&drawing);
        state.files.record(&missing);
        assert!(state.files.load_path(&missing).is_err());
        assert_eq!(state.files.recent.peek().entries().len(), 1);
        app.mount(editor_app(state)).unwrap();
        app.settle(8).unwrap();
        let press = |label: &str| {
            let button = crate::usability_tests::find_button(&app, label);
            app.dispatch(button, UiEvent::Activate).unwrap();
            app.settle(8).unwrap();
        };
        press("File");
        press("Open Recent");
        press(&format!("Open {}", drawing.display()));
        assert_eq!(*state.file_path.peek(), Some(drawing));
        assert!(!state.engine.peek().is_dirty());
        press("File");
        press("Open Recent");
        press("Clear recent files");
        let recent = RecentFiles::load_from_file(folder.path().join("recent_files.json")).unwrap();
        assert!(recent.entries().is_empty());
    }
}
