//! Native Blitz event host for the editor canvas and window commands.
//! Canvas input uses viewport logical pixels; widgets retain Glory's semantic input.
use blitz_dom::{DocGuard, DocGuardMut, Document};
use blitz_shell::{BlitzApplication, BlitzShellEvent, BlitzShellProxy, WindowConfig};
use blitz_traits::events::{BlitzWheelDelta, MouseEventButton, UiEvent};
use glory::blitz::shell::{CpuRenderer, WindowApp};
use keyboard_types::{Key, Modifiers};
use sivgo_core::MouseButton;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
    task::Context,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    monitor::Fullscreen,
    window::{WindowAttributes, WindowId},
};

#[derive(Debug, Clone)]
pub enum NativeInput {
    PointerDown {
        x: f64,
        y: f64,
        button: MouseButton,
        modifiers: Modifiers,
    },
    PointerMove {
        x: f64,
        y: f64,
        button: MouseButton,
        modifiers: Modifiers,
    },
    PointerUp {
        x: f64,
        y: f64,
        button: MouseButton,
        modifiers: Modifiers,
    },
    DoubleClick {
        x: f64,
        y: f64,
        button: MouseButton,
        modifiers: Modifiers,
    },
    PointerCancel,
    FocusLost,
    Wheel {
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
        modifiers: Modifiers,
    },
    KeyDown {
        key: Key,
        modifiers: Modifiers,
        editable: bool,
    },
    Resize {
        width: f64,
        height: f64,
    },
}

enum Command {
    Title(String),
    Fullscreen(bool),
    Close,
}
struct WindowState {
    commands: RefCell<VecDeque<Command>>,
    size: Cell<(f64, f64)>,
    proxy: RefCell<Option<BlitzShellProxy>>,
}
#[derive(Clone)]
pub struct DesktopContext(Rc<WindowState>);
impl Default for DesktopContext {
    fn default() -> Self {
        Self::new()
    }
}
impl DesktopContext {
    pub fn new() -> Self {
        Self(Rc::new(WindowState {
            commands: RefCell::default(),
            size: Cell::new((1440.0, 900.0)),
            proxy: RefCell::default(),
        }))
    }
    fn queue(&self, command: Command) {
        self.0.commands.borrow_mut().push_back(command);
        if let Some(proxy) = self.0.proxy.borrow().as_ref() {
            proxy.wake_up();
        }
    }
    pub fn set_title(&self, title: &str) {
        self.queue(Command::Title(title.into()));
    }
    pub fn logical_size(&self) -> (f64, f64) {
        self.0.size.get()
    }
    pub fn close(&self) {
        self.queue(Command::Close);
    }
    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.queue(Command::Fullscreen(fullscreen));
    }
}

type InputHandler = Rc<RefCell<Box<dyn FnMut(NativeInput) -> bool>>>;
struct EditorDocument {
    inner: WindowApp,
    input: InputHandler,
    error: Rc<RefCell<Option<String>>>,
    last_click: Option<(Instant, f64, f64)>,
    // true: editor canvas gesture; false: semantic widget gesture.
    pointer_owner: Option<bool>,
}
fn mouse_button(button: MouseEventButton) -> MouseButton {
    match button {
        MouseEventButton::Auxiliary => MouseButton::Middle,
        MouseEventButton::Secondary => MouseButton::Right,
        _ => MouseButton::Left,
    }
}
impl EditorDocument {
    fn editable(&self) -> bool {
        let doc = self.inner.inner();
        doc.get_focussed_node_id()
            .and_then(|id| doc.get_node(id))
            .and_then(|node| node.element_data())
            .is_some_and(|element| {
                matches!(
                    element.name.local.as_ref(),
                    "input" | "textarea" | "select" | "button" | "a"
                ) || element
                    .attr(blitz_dom::LocalName::from("contenteditable"))
                    .is_some_and(|v| v != "false")
            })
    }
    fn interactive_target(&self, x: f32, y: f32) -> bool {
        let doc = self.inner.inner();
        let mut target = doc.hit(x, y).map(|hit| hit.node_id);
        while let Some(id) = target {
            let Some(node) = doc.get_node(id) else { break };
            if node.element_data().is_some_and(|element| {
                matches!(
                    element.name.local.as_ref(),
                    "input" | "textarea" | "select" | "button" | "a"
                ) || element
                    .attr(blitz_dom::LocalName::from("role"))
                    .is_some_and(|role| {
                        matches!(
                            role,
                            "button"
                                | "link"
                                | "menu"
                                | "menuitem"
                                | "menuitemcheckbox"
                                | "menuitemradio"
                                | "dialog"
                                | "listbox"
                                | "combobox"
                                | "slider"
                                | "spinbutton"
                                | "textbox"
                                | "option"
                        )
                    })
                    || element
                        .attr(blitz_dom::LocalName::from("contenteditable"))
                        .is_some_and(|value| value != "false")
            }) {
                return true;
            }
            target = node.parent.or(node.layout_parent.get());
        }
        false
    }
    fn emit(&self, input: NativeInput) -> bool {
        (self.input.borrow_mut())(input)
    }
}
impl Document for EditorDocument {
    fn inner(&self) -> DocGuard<'_> {
        self.inner.inner()
    }
    fn inner_mut(&mut self) -> DocGuardMut<'_> {
        self.inner.inner_mut()
    }
    fn handle_ui_event(&mut self, event: UiEvent) {
        let terminal = matches!(event, UiEvent::PointerUp(_) | UiEvent::PointerCancel(_));
        let widget_gesture = self.pointer_owner == Some(false)
            && matches!(
                event,
                UiEvent::PointerMove(_) | UiEvent::PointerUp(_) | UiEvent::PointerCancel(_)
            );
        let interactive_target = match &event {
            UiEvent::PointerDown(e) => self.interactive_target(e.page_x(), e.page_y()),
            UiEvent::PointerMove(e) if self.pointer_owner.is_none() => {
                self.interactive_target(e.page_x(), e.page_y())
            }
            UiEvent::Wheel(e) => self.interactive_target(e.page_x(), e.page_y()),
            _ => false,
        };
        if widget_gesture || interactive_target {
            if matches!(event, UiEvent::PointerDown(_)) {
                self.pointer_owner = Some(false);
                self.last_click = None;
            }
            if terminal {
                self.pointer_owner = None;
            }
            self.inner.handle_ui_event(event);
            return;
        }
        let mut double_click = None;
        let input = match &event {
            UiEvent::PointerDown(e) => {
                let x = f64::from(e.client_x());
                let y = f64::from(e.client_y());
                if e.button == MouseEventButton::Main {
                    let now = Instant::now();
                    if self.last_click.is_some_and(|(time, px, py)| {
                        now.duration_since(time) <= Duration::from_millis(500)
                            && (x - px).hypot(y - py) <= 5.0
                    }) {
                        double_click = Some(NativeInput::DoubleClick {
                            x,
                            y,
                            button: MouseButton::Left,
                            modifiers: e.mods,
                        });
                        self.last_click = None;
                    } else {
                        self.last_click = Some((now, x, y));
                    }
                }
                Some(NativeInput::PointerDown {
                    x,
                    y,
                    button: mouse_button(e.button),
                    modifiers: e.mods,
                })
            }
            UiEvent::PointerMove(e) => Some(NativeInput::PointerMove {
                x: e.client_x().into(),
                y: e.client_y().into(),
                button: mouse_button(e.button),
                modifiers: e.mods,
            }),
            UiEvent::PointerUp(e) => Some(NativeInput::PointerUp {
                x: e.client_x().into(),
                y: e.client_y().into(),
                button: mouse_button(e.button),
                modifiers: e.mods,
            }),
            UiEvent::PointerCancel(_) => Some(NativeInput::PointerCancel),
            UiEvent::Wheel(e) => {
                let (dx, dy) = match e.delta {
                    BlitzWheelDelta::Lines(x, y) => (x * 40.0, y * 40.0),
                    BlitzWheelDelta::Pixels(x, y) => (x, y),
                };
                Some(NativeInput::Wheel {
                    x: e.client_x().into(),
                    y: e.client_y().into(),
                    dx,
                    dy,
                    modifiers: e.mods,
                })
            }
            UiEvent::KeyDown(e) if !e.is_composing => Some(NativeInput::KeyDown {
                key: e.key.clone(),
                modifiers: e.modifiers,
                editable: self.editable(),
            }),
            _ => None,
        };
        let consumed = input.is_some_and(|input| self.emit(input));
        if matches!(event, UiEvent::PointerDown(_)) {
            self.pointer_owner = Some(consumed);
        }
        if terminal {
            self.pointer_owner = None;
        }
        if consumed {
            if matches!(event, UiEvent::PointerDown(_)) {
                self.inner.inner_mut().clear_focus();
            }
            if let Some(input) = double_click {
                self.emit(input);
            }
            self.inner.renderer().invalidate();
        } else {
            self.inner.handle_ui_event(event);
        }
    }
    fn poll(&mut self, context: Option<Context>) -> bool {
        let Some(context) = context else { return false };
        match self.inner.observe(&context) {
            Ok(changed) => changed,
            Err(error) => {
                *self.error.borrow_mut() = Some(error.to_string());
                false
            }
        }
    }
}

struct EditorShell {
    inner: BlitzApplication<CpuRenderer>,
    desktop: DesktopContext,
    input: InputHandler,
    close: Box<dyn FnMut() -> bool>,
    error: Rc<RefCell<Option<String>>>,
}
impl EditorShell {
    fn prepare(view: &mut blitz_shell::View<CpuRenderer>) {
        let time = view.current_animation_time();
        let doc = view.downcast_doc_mut::<EditorDocument>();
        if let Err(error) = doc.inner.renderer().advance_time(time) {
            *doc.error.borrow_mut() = Some(error.to_string());
        }
    }
    fn resize(&mut self, id: WindowId) {
        if let Some(view) = self.inner.windows.get(&id) {
            let size = view
                .window
                .surface_size()
                .to_logical::<f64>(view.window.scale_factor());
            let next = (size.width, size.height);
            if self.desktop.0.size.replace(next) != next {
                (self.input.borrow_mut())(NativeInput::Resize {
                    width: next.0,
                    height: next.1,
                });
            }
        }
    }
    fn commands(&mut self, event_loop: &dyn ActiveEventLoop) {
        let commands: Vec<_> = self.desktop.0.commands.borrow_mut().drain(..).collect();
        for command in commands {
            match command {
                Command::Title(title) => {
                    for view in self.inner.windows.values() {
                        view.window.set_title(&title);
                    }
                }
                Command::Fullscreen(value) => {
                    for view in self.inner.windows.values() {
                        view.window
                            .set_fullscreen(value.then_some(Fullscreen::Borderless(None)));
                    }
                }
                Command::Close => {
                    if (self.close)() {
                        self.inner.windows.clear();
                        event_loop.exit();
                    }
                }
            }
        }
        if self.error.borrow().is_some() {
            self.inner.windows.clear();
            event_loop.exit();
        }
    }
}
impl ApplicationHandler for EditorShell {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.can_create_surfaces(event_loop);
        let ids: Vec<_> = self.inner.windows.keys().copied().collect();
        for id in ids {
            self.resize(id);
        }
        self.commands(event_loop);
    }
    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.destroy_surfaces(event_loop);
    }
    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) && !(self.close)() {
            return;
        }
        if matches!(event, WindowEvent::Focused(false)) {
            if let Some(view) = self.inner.windows.get_mut(&id) {
                view.downcast_doc_mut::<EditorDocument>().pointer_owner = None;
            }
            (self.input.borrow_mut())(NativeInput::FocusLost);
        }
        let resized = matches!(
            event,
            WindowEvent::SurfaceResized(_) | WindowEvent::ScaleFactorChanged { .. }
        );
        if let Some(view) = self.inner.windows.get_mut(&id) {
            Self::prepare(view);
            if matches!(event, WindowEvent::RedrawRequested) {
                view.poll();
            } else {
                view.downcast_doc_mut::<EditorDocument>()
                    .inner
                    .renderer()
                    .invalidate();
            }
        }
        self.inner.window_event(event_loop, id, event);
        if resized {
            self.resize(id);
        }
        self.commands(event_loop);
    }
    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        let mut polled = std::collections::HashSet::new();
        for index in 0..64 {
            let Ok(event) = self.inner.event_queue.try_recv() else {
                break;
            };
            if let BlitzShellEvent::Poll { window_id } = &event {
                if !polled.insert(*window_id) {
                    self.inner.proxy.send_event(event);
                    break;
                }
                if let Some(view) = self.inner.windows.get_mut(window_id) {
                    Self::prepare(view);
                }
            }
            if matches!(event, BlitzShellEvent::CloseWindow { .. }) && !(self.close)() {
                continue;
            }
            if let BlitzShellEvent::RequestRedraw { doc_id } = &event {
                for view in self
                    .inner
                    .windows
                    .values_mut()
                    .filter(|v| v.doc.id() == *doc_id)
                {
                    view.downcast_doc_mut::<EditorDocument>()
                        .inner
                        .renderer()
                        .invalidate();
                }
            }
            let resumed = matches!(event, BlitzShellEvent::ResumeReady { .. });
            self.inner.handle_blitz_shell_event(event_loop, event);
            if resumed {
                for view in self.inner.windows.values_mut() {
                    Self::prepare(view);
                    view.poll();
                    view.request_redraw();
                }
            }
            if index == 63 {
                self.inner.proxy.wake_up();
            }
        }
        self.commands(event_loop);
    }
    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.commands(event_loop);
    }
}

pub fn run(
    app: WindowApp,
    desktop: DesktopContext,
    input: impl FnMut(NativeInput) -> bool + 'static,
    close: impl FnMut() -> bool + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::builder().build()?;
    let (proxy, queue) = BlitzShellProxy::new(event_loop.create_proxy());
    *desktop.0.proxy.borrow_mut() = Some(proxy.clone());
    let input: InputHandler = Rc::new(RefCell::new(Box::new(input)));
    let error = Rc::new(RefCell::new(None));
    let doc = EditorDocument {
        inner: app,
        input: input.clone(),
        error: error.clone(),
        last_click: None,
        pointer_owner: None,
    };
    let mut inner = BlitzApplication::new(proxy, queue);
    inner.add_window(WindowConfig::with_attributes(
        Box::new(doc),
        CpuRenderer::new(),
        WindowAttributes::default()
            .with_title("Sivgo")
            .with_surface_size(winit::dpi::LogicalSize::new(1440.0, 900.0)),
    ));
    let shell = EditorShell {
        inner,
        desktop: desktop.clone(),
        input,
        close: Box::new(close),
        error: error.clone(),
    };
    event_loop.run_app(shell)?;
    desktop.0.proxy.borrow_mut().take();
    if let Some(error) = error.borrow_mut().take() {
        return Err(std::io::Error::other(error).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use blitz_traits::events::{BlitzKeyEvent, KeyState};
    use glory::ui::events::value_changed;
    use keyboard_types::{Code, Location};

    #[test]
    fn editable_keys_are_reported_and_unconsumed_input_reaches_glory() {
        let app = WindowApp::new();
        let value = Rc::new(RefCell::new(String::new()));
        let changed = value.clone();
        app.mount(
            glory::ui::input()
                .value("abc")
                .on(value_changed, move |event| {
                    if let glory::render::Event::ValueChanged { value, .. } = event {
                        *changed.borrow_mut() = value;
                    }
                }),
        )
        .unwrap();
        app.app().settle(8).unwrap();
        let editable = Rc::new(Cell::new(false));
        let seen = editable.clone();
        let input: InputHandler = Rc::new(RefCell::new(Box::new(move |event| {
            if let NativeInput::KeyDown { editable, .. } = event {
                seen.set(editable);
            }
            false
        })));
        let mut doc = EditorDocument {
            inner: app,
            input,
            error: Rc::default(),
            last_click: None,
            pointer_owner: None,
        };
        let node = {
            let dom = doc.inner();
            let mut queue = vec![dom.root_node().id];
            let mut input = None;
            while let Some(id) = queue.pop() {
                let node = dom.get_node(id).unwrap();
                if node
                    .element_data()
                    .is_some_and(|e| &*e.name.local == "input")
                {
                    input = Some(id);
                    break;
                }
                queue.extend(node.children.iter().copied());
            }
            input.unwrap()
        };
        assert!(doc.inner_mut().set_focus_to(node));
        doc.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
            key: Key::Character("x".into()),
            code: Code::KeyX,
            modifiers: Modifiers::empty(),
            location: Location::Standard,
            is_auto_repeating: false,
            is_composing: false,
            state: KeyState::Pressed,
            text: Some("x".into()),
        }));
        doc.inner.app().settle(8).unwrap();
        assert!(
            editable.get(),
            "focused editor must suppress global canvas shortcuts"
        );
        assert!(
            value.borrow().contains('x'),
            "unconsumed key must reach the actual native text editor"
        );
    }

    #[test]
    fn consumed_shortcut_never_edits_the_focused_input() {
        let app = WindowApp::new();
        let value = Rc::new(RefCell::new(String::new()));
        let changed = value.clone();
        app.mount(
            glory::ui::input()
                .value("abc")
                .on(value_changed, move |event| {
                    if let glory::render::Event::ValueChanged { value, .. } = event {
                        *changed.borrow_mut() = value;
                    }
                }),
        )
        .unwrap();
        let input: InputHandler = Rc::new(RefCell::new(Box::new(|_| true)));
        let mut doc = EditorDocument {
            inner: app,
            input,
            error: Rc::default(),
            last_click: None,
            pointer_owner: None,
        };
        let node = {
            let dom = doc.inner();
            let mut queue = vec![dom.root_node().id];
            let mut input = None;
            while let Some(id) = queue.pop() {
                let node = dom.get_node(id).unwrap();
                if node
                    .element_data()
                    .is_some_and(|e| &*e.name.local == "input")
                {
                    input = Some(id);
                    break;
                }
                queue.extend(node.children.iter().copied());
            }
            input.unwrap()
        };
        assert!(doc.inner_mut().set_focus_to(node));
        doc.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
            key: Key::Character("s".into()),
            code: Code::KeyS,
            modifiers: Modifiers::CONTROL,
            location: Location::Standard,
            is_auto_repeating: false,
            is_composing: false,
            state: KeyState::Pressed,
            text: Some("s".into()),
        }));
        doc.inner.app().settle(8).unwrap();
        assert!(value.borrow().is_empty());
    }
    fn pointer(x: f32, y: f32) -> blitz_traits::events::BlitzPointerEvent {
        use blitz_traits::events::*;
        BlitzPointerEvent {
            id: BlitzPointerId::Mouse,
            is_primary: true,
            coords: PointerCoords {
                page_x: x,
                page_y: y,
                screen_x: x,
                screen_y: y,
                client_x: x,
                client_y: y,
            },
            button: MouseEventButton::Main,
            buttons: MouseEventButtons::Primary,
            mods: Modifiers::empty(),
            details: Default::default(),
            element: Default::default(),
            active_pointers: Default::default(),
        }
    }
    fn input_document(input: InputHandler) -> EditorDocument {
        let inner = WindowApp::new();
        inner.renderer().set_css("html, body { margin:0; padding:0; } input { display:block; width:100px; height:30px; }");
        inner.mount(glory::ui::input().value("abc")).unwrap();
        inner.app().settle(8).unwrap();
        EditorDocument {
            inner,
            input,
            error: Rc::default(),
            last_click: None,
            pointer_owner: None,
        }
    }
    #[test]
    fn canvas_click_releases_input_focus_before_the_next_shortcut() {
        let editable = Rc::new(Cell::new(true));
        let seen = editable.clone();
        let input: InputHandler = Rc::new(RefCell::new(Box::new(move |event| {
            if let NativeInput::KeyDown { editable, .. } = event {
                seen.set(editable);
            }
            true
        })));
        let mut doc = input_document(input);
        doc.handle_ui_event(UiEvent::PointerDown(pointer(10.0, 10.0)));
        doc.handle_ui_event(UiEvent::PointerUp(pointer(10.0, 10.0)));
        assert!(doc.editable());
        doc.handle_ui_event(UiEvent::PointerDown(pointer(300.0, 300.0)));
        assert!(!doc.editable());
        doc.handle_ui_event(UiEvent::KeyDown(BlitzKeyEvent {
            key: Key::Character("r".into()),
            code: Code::KeyR,
            modifiers: Modifiers::empty(),
            location: Location::Standard,
            is_auto_repeating: false,
            is_composing: false,
            state: KeyState::Pressed,
            text: Some("r".into()),
        }));
        assert!(!editable.get());
    }
    #[test]
    fn editable_pointer_gesture_bypasses_canvas_even_when_dragging_outside_input() {
        let calls = Rc::new(Cell::new(0));
        let seen = calls.clone();
        let input: InputHandler = Rc::new(RefCell::new(Box::new(move |_| {
            seen.set(seen.get() + 1);
            true
        })));
        let mut doc = input_document(input);
        doc.handle_ui_event(UiEvent::PointerDown(pointer(10.0, 10.0)));
        assert!(doc.editable());
        doc.handle_ui_event(UiEvent::PointerMove(pointer(300.0, 300.0)));
        doc.handle_ui_event(UiEvent::PointerUp(pointer(300.0, 300.0)));
        assert_eq!(
            calls.get(),
            0,
            "native text selection must never trigger a canvas gesture"
        );
    }
    #[test]
    fn popup_button_receives_activation_instead_of_canvas_pointer_actions() {
        let calls = Rc::new(Cell::new(0));
        let raw = calls.clone();
        let input: InputHandler = Rc::new(RefCell::new(Box::new(move |_| {
            raw.set(raw.get() + 1);
            true
        })));
        let pressed = Rc::new(Cell::new(0));
        let on_press = pressed.clone();
        let inner = WindowApp::new();
        inner.renderer().set_css("html, body { margin:0; padding:0; } button { display:block; width:100px; height:30px; }");
        inner
            .mount(
                sapid::button()
                    .label("Menu action")
                    .on_press(move || on_press.set(on_press.get() + 1)),
            )
            .unwrap();
        inner.app().settle(8).unwrap();
        let mut doc = EditorDocument {
            inner,
            input,
            error: Rc::default(),
            last_click: None,
            pointer_owner: None,
        };
        doc.handle_ui_event(UiEvent::PointerDown(pointer(10.0, 10.0)));
        doc.handle_ui_event(UiEvent::PointerUp(pointer(10.0, 10.0)));
        doc.inner.app().settle(8).unwrap();
        assert_eq!(calls.get(), 0);
        assert_eq!(pressed.get(), 1);
    }

    #[test]
    fn inspector_tabs_remain_clickable_after_scrolling_their_content() {
        let inner = WindowApp::new();
        inner.renderer().set_viewport(1440, 900, 1.0).unwrap();
        inner
            .renderer()
            .set_css(&format!("{}\n{}", crate::CSS, crate::HOST_CSS));
        let state = crate::AppState::new(inner.app().scope(), DesktopContext::new());
        {
            let mut engine = state.engine.write();
            let root = engine.document.root().unwrap();
            let id = engine
                .document
                .add_node(sivgo_dom::SvgElement::Rect(sivgo_dom::RectAttrs::default()));
            engine.document.append_child(root, id);
            engine.selection.select(id);
        }
        inner.mount(crate::editor_app(state)).unwrap();
        crate::refresh_canvas_bounds(inner.app().scope(), state);
        inner.app().settle(16).unwrap();
        let input: InputHandler = Rc::new(RefCell::new(Box::new(move |event| {
            crate::handle_native_input(state, event)
        })));
        let mut doc = EditorDocument {
            inner,
            input,
            error: Rc::default(),
            last_click: None,
            pointer_owner: None,
        };
        let find = |doc: &EditorDocument, attribute: &str, expected: &str| {
            let dom = doc.inner();
            let mut queue = vec![dom.root_node().id];
            while let Some(id) = queue.pop() {
                let node = dom.get_node(id).unwrap();
                if node.element_data().is_some_and(|element| {
                    element.attr(blitz_dom::LocalName::from(attribute)) == Some(expected)
                }) {
                    return id;
                }
                queue.extend(node.children.iter().copied());
            }
            panic!("missing {attribute}={expected}");
        };
        let scroll = find(&doc, "class", "inspector-content");
        doc.inner_mut().scroll_node_by(scroll, 0.0, -300.0, |_| {});
        assert!(
            doc.inner().get_node(scroll).unwrap().scroll_offset().y > 20.0,
            "fixture must actually scroll the populated style panel"
        );
        doc.inner.renderer().invalidate();
        doc.inner.app().settle(16).unwrap();
        let button = find(&doc, "aria-label", "Show Objects panel");
        let (x, y) = {
            let dom = doc.inner();
            let node = dom.get_node(button).unwrap();
            let position = node.absolute_position(0.0, 0.0);
            (
                position.x + node.final_layout().size.width / 2.0,
                position.y + node.final_layout().size.height / 2.0,
            )
        };
        let mut hover = pointer(x, y);
        hover.buttons = blitz_traits::events::MouseEventButtons::empty();
        doc.handle_ui_event(UiEvent::PointerMove(hover));
        doc.handle_ui_event(UiEvent::PointerDown(pointer(x, y)));
        let mut release = pointer(x, y);
        release.buttons = blitz_traits::events::MouseEventButtons::empty();
        doc.handle_ui_event(UiEvent::PointerUp(release));
        doc.inner.app().settle(16).unwrap();
        assert_eq!(
            *state.inspector_panel.peek(),
            crate::InspectorPanel::Objects,
            "content clipped above the scrollport must not intercept the header tab"
        );
        let scroll = find(&doc, "class", "inspector-content");
        assert_eq!(
            doc.inner().get_node(scroll).unwrap().scroll_offset().y,
            0.0,
            "a newly opened panel must start at the top"
        );
        state.inspector_panel.set(crate::InspectorPanel::Style);
        doc.inner.app().settle(16).unwrap();
        let scroll = find(&doc, "class", "inspector-content");
        doc.inner_mut().scroll_node_by(scroll, 0.0, -300.0, |_| {});
        state.engine.write().selection.clear();
        crate::bump_render(state.render_version);
        doc.inner.app().settle(16).unwrap();
        let scroll = find(&doc, "class", "inspector-content");
        assert_eq!(
            doc.inner().get_node(scroll).unwrap().scroll_offset().y,
            0.0,
            "removing the selection must not leave a shorter panel scrolled out of view"
        );
    }
}
