//! Desktop conversation and reviewed, undoable SVG proposals.
use std::sync::mpsc::{self, Receiver, TryRecvError};

use sivgo_agent::{
    AgentClient, EditRequest, Message, Role, MAX_HISTORY_MESSAGES, MAX_MESSAGE_BYTES, MAX_SVG_BYTES,
};
use sivgo_dom::{serialize_svg, SvgDocument};
use sivgo_ui::icons::EditorIcon;

use super::*;

#[derive(Clone, Copy)]
pub(super) struct AgentState {
    base_url: Cage<String>,
    model: Cage<String>,
    key: Cage<String>,
    prompt: Cage<String>,
    return_panel: Cage<InspectorPanel>,
    session: Signal<Session>,
}

#[derive(Default)]
struct Session {
    history: Vec<Message>,
    status: String,
    pending: Option<Pending>,
    proposal: Option<Proposal>,
}

type WorkerResult = Result<(String, Option<Candidate>), String>;

struct Pending {
    root: Option<NodeId>,
    source: String,
    prompt: String,
    result: Receiver<WorkerResult>,
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}

struct Candidate {
    document: SvgDocument,
    thumbnail: String,
    review: Vec<String>,
}

struct Proposal {
    root: Option<NodeId>,
    source: String,
    candidate: Candidate,
}

impl AgentState {
    pub(super) fn new(scope: &Scope) -> Self {
        Self {
            base_url: scope.cage(
                std::env::var("SIVGO_AI_BASE_URL")
                    .unwrap_or_else(|_| sivgo_agent::DEFAULT_BASE_URL.to_owned()),
            ),
            model: scope.cage(std::env::var("SIVGO_AI_MODEL").unwrap_or_default()),
            key: scope.cage(std::env::var("SIVGO_AI_API_KEY").unwrap_or_default()),
            prompt: scope.cage(String::new()),
            return_panel: scope.cage(InspectorPanel::Style),
            session: Signal::new(scope, Session::default()),
        }
    }

    pub(super) fn toggle(self, state: AppState) {
        let current = *state.inspector_panel.peek();
        if current == InspectorPanel::Agent {
            state.inspector_panel.set(self.return_panel.get());
        } else {
            self.return_panel.revise(|panel| *panel = current);
            state.inspector_panel.set(InspectorPanel::Agent);
        }
    }

    pub(super) fn reset(self) {
        let mut session = self.session.write();
        session.pending = None;
        session.history.clear();
        session.proposal = None;
        session.status.clear();
        self.prompt.revise(String::clear);
    }

    fn cancel(self) {
        let mut session = self.session.write();
        if let Some(pending) = session.pending.take() {
            if self.prompt.get_untracked().is_empty() {
                self.prompt
                    .revise(|prompt| *prompt = pending.prompt.clone());
            }
            session.status = "Request stopped. Your drawing is unchanged.".into();
        }
    }

    fn send(self, state: AppState) {
        let mut session = self.session.write();
        let prompt = self.prompt.get_untracked().trim().to_owned();
        if session.pending.is_some() || session.proposal.is_some() || prompt.is_empty() {
            return;
        }
        if prompt.len() > MAX_MESSAGE_BYTES {
            session.status = "Use an instruction of up to 4 KiB.".into();
            return;
        }
        let client =
            match AgentClient::new(&self.base_url.get(), &self.model.get(), &self.key.get()) {
                Ok(client) => client,
                Err(error) => {
                    session.status = error.to_string();
                    return;
                }
            };
        let engine = state.engine.peek();
        let root = engine.document.root();
        let source = serialize_svg(&engine.document);
        if source.len() > MAX_SVG_BYTES {
            session.status = "AI editing supports SVG drawings up to 256 KiB.".into();
            return;
        }
        let (svg, selected_ids) = selection_context(&engine, &source);
        let original = engine.document.clone();
        drop(engine);

        let request = EditRequest {
            svg,
            prompt: prompt.clone(),
            history: session.history.clone(),
            selected_ids,
        };
        let (send, result) = mpsc::channel();
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        let wake = state.desktop.get().background_waker();
        let started = std::thread::Builder::new()
            .name("sivgo-agent".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    propose(client, request, original, cancelled)
                }))
                .unwrap_or_else(|_| Err("Could not prepare the proposed drawing.".into()));
                let _ = send.send(result);
                wake();
            });
        match started {
            Ok(_) => {
                session.pending = Some(Pending {
                    root,
                    source,
                    prompt,
                    result,
                    cancel: Some(cancel),
                });
                session.status = "Waiting for the model…".into();
                self.prompt.revise(String::clear);
            }
            Err(_) => session.status = "Could not start the AI request.".into(),
        }
    }

    pub(super) fn poll(self, state: AppState) -> bool {
        let result = match self.session.peek().pending.as_ref() {
            Some(pending) => match pending.result.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) => return false,
                Err(TryRecvError::Disconnected) => {
                    Err("The AI request stopped unexpectedly.".into())
                }
            },
            None => return false,
        };
        let mut session = self.session.write();
        let Some(pending) = session.pending.take() else {
            return false;
        };
        if pending.root.is_none() {
            return true;
        }
        if !matches_source(state, pending.root, &pending.source) {
            session.status =
                "The drawing changed. Send a new instruction to review an edit.".into();
            if self.prompt.get_untracked().is_empty() {
                self.prompt
                    .revise(|prompt| *prompt = pending.prompt.clone());
            }
            return true;
        }
        match result {
            Ok((message, candidate)) => {
                session.history.push(Message {
                    role: Role::User,
                    content: pending.prompt.clone(),
                });
                session.history.push(Message {
                    role: Role::Assistant,
                    content: message,
                });
                let excess = session.history.len().saturating_sub(MAX_HISTORY_MESSAGES);
                session.history.drain(..excess);
                session.status.clear();
                session.proposal = candidate.map(|candidate| Proposal {
                    root: pending.root,
                    source: pending.source.clone(),
                    candidate,
                });
            }
            Err(error) => {
                session.status = error;
                if self.prompt.get_untracked().is_empty() {
                    self.prompt
                        .revise(|prompt| *prompt = pending.prompt.clone());
                }
            }
        }
        true
    }

    fn apply(self, state: AppState) {
        let Some(proposal) = self.session.write().proposal.take() else {
            return;
        };
        if !matches_source(state, proposal.root, &proposal.source)
            || *state.canvas_pointer_down.peek()
        {
            self.session.write().status = "The drawing changed. Request a new proposal.".into();
            return;
        }
        state
            .engine
            .write()
            .replace_drawing(proposal.candidate.document);
        bump_render(state.render_version);
        self.session.write().status = "Applied to the drawing. Undo is available.".into();
    }
}

fn matches_source(state: AppState, root: Option<NodeId>, svg: &str) -> bool {
    let engine = state.engine.peek();
    engine.document.root() == root && serialize_svg(&engine.document) == svg
}

fn selection_context(engine: &EditorEngine, source: &str) -> (String, Vec<String>) {
    let mut snapshot = None;
    let mut selected_ids = Vec::new();
    for &id in engine.selection.selected().iter().take(100) {
        let Some(node) = engine.document.get_node(id) else {
            continue;
        };
        if let Some(name) = node.xml_id.as_ref().filter(|name| !name.is_empty()) {
            if name.len() <= 256 {
                selected_ids.push(name.clone());
            }
            continue;
        }
        // Freshly drawn shapes may have no XML ID. Label them in the snapshot
        // so that "the selected rectangle" resolves without touching the canvas.
        let snapshot = snapshot.get_or_insert_with(|| engine.document.clone());
        let mut name = format!("sivgo-selection-{id}");
        while snapshot.find_by_xml_id(&name).is_some() {
            name.push('_');
        }
        if let Some(node) = snapshot.get_node_mut(id) {
            node.xml_id = Some(name.clone());
            selected_ids.push(name);
        }
    }
    let svg = snapshot
        .as_ref()
        .map(serialize_svg)
        .unwrap_or_else(|| source.to_owned());
    (svg, selected_ids)
}

fn propose(
    client: AgentClient,
    request: EditRequest,
    original: SvgDocument,
    cancelled: tokio::sync::oneshot::Receiver<()>,
) -> WorkerResult {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Could not start the AI connection.".to_owned())?;
    let result = runtime
        .block_on(async {
            tokio::select! {
                biased;
                _ = cancelled => Err(sivgo_agent::AgentError::Input("Request stopped.")),
                result = client.suggest(&request) => result,
            }
        })
        .map_err(|error| error.to_string())?;
    let candidate = result
        .svg
        .map(|svg| {
            let document = if result.operations.is_empty() {
                sivgo_dom::parse_svg(&svg)
                    .map_err(|_| "Could not open the proposed SVG.".to_owned())?
            } else {
                sivgo_agent::apply_object_edits(&original, &request.svg, &result.operations)
                    .map_err(|error| error.to_string())?
            };
            if serialize_svg(&document) == request.svg
                || serialize_svg(&document) == serialize_svg(&original)
            {
                return Ok(None);
            }
            let thumbnail = render_thumbnail(&document)?;
            let review = proposal_review(&request.svg, &result.operations);
            Ok::<_, String>(Some(Candidate {
                document,
                thumbnail,
                review,
            }))
        })
        .transpose()?
        .flatten();
    Ok((result.message, candidate))
}

fn proposal_review(source: &str, operations: &[sivgo_agent::ObjectEdit]) -> Vec<String> {
    if operations.is_empty() {
        return vec!["Full drawing replacement. Review the preview before applying.".into()];
    }
    sivgo_agent::review_object_edits(source, operations)
}

fn render_thumbnail(document: &SvgDocument) -> Result<String, String> {
    use base64::Engine as _;
    const SIZE: u32 = 216;
    let mut preview = document.clone();
    let root = preview.root().and_then(|id| preview.get_node_mut(id));
    if let Some(SvgElement::Svg(attrs)) = root.map(|node| &mut node.element) {
        let width = attrs
            .width
            .map(|length| length.to_px())
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(800.0);
        let height = attrs
            .height
            .map(|length| length.to_px())
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(600.0);
        if attrs.view_box.is_none() {
            attrs.view_box = Some(ViewBox::new(0.0, 0.0, width, height));
        }
        attrs.width = Some(SvgLength::new(f64::from(SIZE), LengthUnit::Px));
        attrs.height = attrs.width;
        attrs.preserve_aspect_ratio = None;
    }
    let pixels =
        sivgo_render::renderer::render_document_to_pixmap_without_file_access(&preview, SIZE, SIZE);
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, SIZE, SIZE);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(&pixels))
        .map_err(|_| "Could not render the proposed SVG.".to_owned())?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ))
}

pub(super) fn panel(state: AppState) -> impl Widget {
    widget(move |scope| {
        let agent = state.agent;
        let settings = scope.cage(agent.model.get_untracked().is_empty());
        let can_send = scope.bond(move || {
            let session = agent.session.read();
            session.pending.is_none()
                && session.proposal.is_none()
                && !agent.prompt.get().trim().is_empty()
        });
        let can_reset = agent.session.map(|session| {
            session.pending.is_none() && (!session.history.is_empty() || !session.status.is_empty())
        });
        col()
            .class("agent-panel")
            .style(UiStyle {
                grow: Some(1.0),
                shrink: Some(1.0),
                ..column_style()
                    .gap(10.0)
                    .padding(Edges::all(10.0))
                    .min_height(UiLength::Px(0.0))
            })
            .fill((
                row().style(row_style().gap(6.0)).fill((
                    EditorIcon::Agent.widget(),
                    text()
                        .class("agent-title")
                        .content("AI Agent")
                        .style(UiStyle {
                            grow: Some(1.0),
                            ..UiStyle::default()
                        }),
                    sapid::action_icon("AI provider settings")
                        .theme(sapid::Theme::dark())
                        .size(sapid::Size::ExtraSmall)
                        .icon(EditorIcon::Settings.widget())
                        .on_press(move || settings.revise(|open| *open = !*open)),
                    sapid::close_button()
                        .theme(sapid::Theme::dark())
                        .size(sapid::Size::ExtraSmall)
                        .name("Close AI Agent")
                        .on_press(move || agent.toggle(state)),
                )),
                text()
                    .class("agent-muted")
                    .content("Sending shares this drawing and recent chat with your provider."),
                conversation(state, settings),
                sapid::text_field()
                    .theme(sapid::Theme::dark())
                    .name("Edit instruction")
                    .placeholder("E.g. add a blue circle in the center")
                    .multiline(true)
                    .style(UiStyle {
                        shrink: Some(0.0),
                        ..UiStyle::new().height(UiLength::Px(68.0))
                    })
                    .value(agent.prompt)
                    .on_value_change(move |value| agent.prompt.revise(|prompt| *prompt = value)),
                row().style(row_style().gap(6.0)).fill((
                    sapid::button()
                        .theme(sapid::Theme::dark())
                        .size(sapid::Size::ExtraSmall)
                        .label("Send")
                        .left_section(EditorIcon::Send.widget())
                        .enabled(can_send)
                        .on_press(move || agent.send(state)),
                    glory::ui::when(
                        agent.session.map(|session| session.pending.is_some()),
                        move || {
                            sapid::action_icon("Stop AI request")
                                .theme(sapid::Theme::dark())
                                .size(sapid::Size::ExtraSmall)
                                .icon(EditorIcon::Stop.widget())
                                .on_press(move || agent.cancel())
                        },
                    ),
                    sapid::button()
                        .theme(sapid::Theme::dark())
                        .size(sapid::Size::ExtraSmall)
                        .label("New chat")
                        .left_section(EditorIcon::NewChat.widget())
                        .variant(sapid::ButtonVariant::Secondary)
                        .enabled(can_reset)
                        .on_press(move || agent.reset()),
                )),
                text()
                    .class("agent-message agent-muted")
                    .content(agent.session.map(|session| session.status.clone())),
            ))
            .show_in(scope)
    })
}

fn conversation(state: AppState, settings: Cage<bool>) -> impl Widget {
    widget(move |scope| {
        let agent = state.agent;
        let end = scope.cage(None);
        let weak = scope.downgrade();
        let newest = agent.session.map(|session| {
            session
                .history
                .last()
                .map(|message| message.content.clone())
        });
        let mut previous = None;
        scope.effect(move || {
            let message = newest.get();
            let changed = message.is_some() && message != previous;
            previous = message;
            if let (true, Some(end), Some(scope)) = (changed, end.get(), weak.upgrade()) {
                let _ = scope.action(glory::render::Action::ScrollIntoView(end));
            }
        });
        col()
            .name("AI conversation")
            .style(UiStyle {
                grow: Some(1.0),
                shrink: Some(1.0),
                ..column_style()
                    .gap(8.0)
                    .min_height(UiLength::Px(0.0))
                    .overflow_y(Overflow::Auto)
                    .overflow_x(Overflow::Hidden)
            })
            .fill((
                glory::ui::when(settings, move || {
                    col()
                        .class("agent-card")
                        .style(column_style().gap(5.0).padding(Edges::all(8.0)))
                        .fill((
                            text().class("agent-section").content("Provider"),
                            provider_field("API base URL", agent.base_url, false),
                            provider_field("Model", agent.model, false),
                            provider_field("API key (session only)", agent.key, true),
                        ))
                }),
                glory::ui::each(agent.session.map(|session| {
                    session
                        .history
                        .iter()
                        .enumerate()
                        .map(|(index, message)| {
                            (
                                index,
                                matches!(message.role, Role::User),
                                message.content.clone(),
                            )
                        })
                        .collect::<Vec<_>>()
                }))
                .key(|entry| entry.clone())
                .seed(|(_, user, content)| {
                    col()
                        .class(if user {
                            "agent-card agent-user"
                        } else {
                            "agent-card"
                        })
                        .style(UiStyle {
                            shrink: Some(0.0),
                            ..column_style().gap(4.0).padding(Edges::all(8.0))
                        })
                        .fill((
                            text().class("agent-section").content(if user {
                                "You"
                            } else {
                                "Agent"
                            }),
                            text().class("agent-message").content(content),
                        ))
                }),
                glory::ui::when(
                    agent.session.map(|session| session.proposal.is_some()),
                    move || proposal_card(state),
                ),
                view()
                    .node_ref(end)
                    .style(UiStyle::new().height(UiLength::Px(1.0))),
            ))
            .show_in(scope)
    })
}

fn proposal_card(state: AppState) -> impl Widget {
    let agent = state.agent;
    let thumbnail = agent.session.map(|session| {
        session
            .proposal
            .as_ref()
            .map(|proposal| proposal.candidate.thumbnail.clone())
            .unwrap_or_default()
    });
    col()
        .class("agent-card")
        .style(UiStyle {
            shrink: Some(0.0),
            ..column_style().gap(8.0).padding(Edges::all(8.0))
        })
        .fill((
            text().class("agent-section").content("Proposed drawing"),
            glory::ui::each(agent.session.map(|session| {
                session
                    .proposal
                    .as_ref()
                    .map(|proposal| proposal.candidate.review.clone())
                    .unwrap_or_default()
            }))
            .key(|line| line.clone())
            .seed(|line| text().class("agent-message agent-muted").content(line)),
            row()
                .style(UiStyle {
                    justify: Some(glory::style::Justify::Center),
                    ..row_style()
                })
                .fill(
                    image()
                        .source(thumbnail)
                        .alt("Preview of proposed SVG drawing")
                        .style(
                            UiStyle::new()
                                .width(UiLength::Px(192.0))
                                .height(UiLength::Px(192.0)),
                        ),
                ),
            row().style(row_style().gap(6.0)).fill((
                sapid::button()
                    .theme(sapid::Theme::dark())
                    .size(sapid::Size::ExtraSmall)
                    .label("Apply edit")
                    .left_section(EditorIcon::Check.widget())
                    .on_press(move || agent.apply(state)),
                sapid::button()
                    .theme(sapid::Theme::dark())
                    .size(sapid::Size::ExtraSmall)
                    .variant(sapid::ButtonVariant::Secondary)
                    .label("Discard")
                    .left_section(EditorIcon::Trash.widget())
                    .on_press(move || {
                        let mut session = agent.session.write();
                        session.proposal = None;
                        session.status = "Proposal discarded.".into();
                    }),
            )),
        ))
}

fn provider_field(label: &'static str, value: Cage<String>, secret: bool) -> impl Widget {
    let input = sapid::text_field()
        .theme(sapid::Theme::dark())
        .size(sapid::Size::ExtraSmall)
        .name(label)
        .default_value(value.get_untracked())
        .on_value_change(move |text| value.revise(|current| *current = text));
    let input = if secret {
        input.purpose(glory::render::Purpose::Password)
    } else {
        input
    };
    col()
        .style(column_style().gap(3.0))
        .fill((text().class("agent-muted").content(label), input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
        time::{Duration, Instant},
    };

    #[test]
    fn stop_aborts_the_provider_connection_and_preserves_the_drawing_and_prompt() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = server.local_addr().unwrap();
        let (accepted, received) = mpsc::channel();
        let upstream = std::thread::spawn(move || {
            let (mut connection, _) = server.accept().unwrap();
            connection
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut input = BufReader::new(&mut connection);
            let mut line = String::new();
            let mut length = 0;
            loop {
                line.clear();
                input.read_line(&mut line).unwrap();
                assert!(!line.is_empty());
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse().unwrap();
                    }
                }
            }
            input.read_exact(&mut vec![0; length]).unwrap();
            accepted.send(()).unwrap();
            // The provider deliberately never responds. Cancellation must drop
            // the actual HTTP future rather than just hide its eventual answer.
            let mut byte = [0];
            match input.read(&mut byte) {
                Ok(0) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
                    ) => {}
                other => panic!("provider connection was not cancelled: {other:?}"),
            }
        });
        let app = glory::App::memory();
        let state = AppState::new(app.scope(), DesktopContext::new());
        let agent = state.agent;
        agent
            .base_url
            .revise(|url| *url = format!("http://{address}/v1"));
        agent.model.revise(|model| *model = "test-model".into());
        agent
            .prompt
            .revise(|prompt| *prompt = "Make the drawing blue".into());
        app.mount(editor_app(state)).unwrap();
        agent.toggle(state);
        app.settle(8).unwrap();
        let original = serialize_svg(&state.engine.peek().document);
        app.dispatch(
            crate::usability_tests::find_button(&app, "Send"),
            UiEvent::Activate,
        )
        .unwrap();
        app.settle(8).unwrap();
        received.recv_timeout(Duration::from_secs(5)).unwrap();
        app.dispatch(
            crate::usability_tests::find_button(&app, "Stop AI request"),
            UiEvent::Activate,
        )
        .unwrap();
        app.settle(8).unwrap();
        assert!(agent.session.peek().pending.is_none());
        assert_eq!(agent.prompt.get_untracked(), "Make the drawing blue");
        assert!(agent.session.peek().history.is_empty());
        assert_eq!(serialize_svg(&state.engine.peek().document), original);
        assert!(!state.engine.peek().is_dirty());
        upstream.join().unwrap();
        assert!(!agent.poll(state));
        assert!(agent.session.peek().proposal.is_none());
    }

    #[test]
    fn agent_preview_requires_apply_and_supports_a_follow_up_and_undo() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = server.local_addr().unwrap();
        let upstream = std::thread::spawn(move || {
            for turn in 0..3 {
                let (mut connection, _) = server.accept().unwrap();
                connection
                    .set_read_timeout(Some(Duration::from_secs(4)))
                    .unwrap();
                let mut input = BufReader::new(&mut connection);
                let mut line = String::new();
                input.read_line(&mut line).unwrap();
                assert!(line.starts_with("POST /v1/chat/completions "));
                let mut length = None;
                let mut bearer = false;
                loop {
                    line.clear();
                    input.read_line(&mut line).unwrap();
                    assert!(!line.is_empty(), "unexpected end of request");
                    if line == "\r\n" {
                        break;
                    }
                    let (name, value) = line.split_once(':').unwrap();
                    if name.eq_ignore_ascii_case("content-length") {
                        length = Some(value.trim().parse::<usize>().unwrap());
                    } else if name.eq_ignore_ascii_case("authorization") {
                        bearer = value.trim() == "Bearer test-only";
                    }
                }
                assert!(bearer);
                let mut body = vec![0; length.unwrap()];
                input.read_exact(&mut body).unwrap();
                let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(body["model"], "local-test-model");
                let messages = body["messages"].as_array().unwrap();
                assert_eq!(messages.len(), 2 + 2 * turn);
                let context: serde_json::Value =
                    serde_json::from_str(messages.last().unwrap()["content"].as_str().unwrap())
                        .unwrap();
                let source = context["current_svg"].as_str().unwrap();
                let answer = if turn == 0 {
                    assert!(!source.contains("circle"));
                    let selected = context["selected_ids"][0].as_str().unwrap();
                    assert!(selected.starts_with("sivgo-selection-"));
                    assert!(source.contains(&format!("id=\"{selected}\"")));
                    serde_json::json!({
                        "message": "Added the circle.",
                        "svg": source.replace("</svg>", "<circle id='dot' cx='400' cy='300' r='80' fill='blue'/></svg>")
                    })
                } else if turn == 1 {
                    serde_json::json!({"message":"Changed circle color.", "svg":null,
                        "operations":[{"id":"dot","attributes":{"fill":"red"}}]})
                } else {
                    assert!(source.contains("circle"));
                    assert_eq!(messages[1]["content"], "Add a blue circle");
                    assert_eq!(messages[2]["content"], "Added the circle.");
                    serde_json::json!({"message": "The circle is red.", "svg": null})
                };
                let response = serde_json::json!({
                    "choices": [{"message": {"content": answer.to_string()}, "finish_reason": "stop"}],
                }).to_string();
                write!(connection,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response.len(), response,
                ).unwrap();
            }
        });

        let app = glory::apace::shell::WindowApp::with_config(glory::apace::Config {
            provider: Some(std::sync::Arc::new(
                canvas_assets::CanvasImageProvider::new().unwrap(),
            )),
            ..Default::default()
        });
        let (width, height) = (1200, 800);
        app.renderer().set_viewport(width, height, 1.0).unwrap();
        app.renderer()
            .set_css(&format!("{CSS}\n{HOST_CSS}\n{AGENT_CSS}"));
        let state = AppState::new(app.app().scope(), DesktopContext::new());
        state.window_size.set((f64::from(width), f64::from(height)));
        {
            let document = sivgo_dom::parse_svg(
                "<svg xmlns='http://www.w3.org/2000/svg' width='800' height='600'><rect width='10' height='10'/></svg>",
            ).unwrap();
            let rect = document.children(document.root().unwrap())[0];
            let mut engine = state.engine.write();
            engine.load_document(document);
            engine.selection.select(rect);
        }
        let agent = state.agent;
        agent
            .base_url
            .revise(|url| *url = format!("http://{address}/v1"));
        agent
            .model
            .revise(|model| *model = "local-test-model".into());
        agent.key.revise(|key| *key = "test-only".into());
        app.mount(editor_app(state)).unwrap();
        let press = |label| {
            app.app()
                .dispatch(
                    crate::usability_tests::find_button(app.app(), label),
                    UiEvent::Activate,
                )
                .unwrap();
            app.app().settle(8).unwrap();
        };
        app.app().settle(8).unwrap();
        press("AI Agent");

        let original = serialize_svg(&state.engine.peek().document);
        assert!(!original.contains("sivgo-selection-"));
        let run_turn = |prompt: &str| {
            agent.prompt.revise(|value| *value = prompt.into());
            app.app().settle(8).unwrap();
            press("Send");
            let deadline = Instant::now() + Duration::from_secs(8);
            while !handle_native_input(state, NativeInput::BackgroundReady) {
                assert!(Instant::now() < deadline, "provider response never arrived");
                std::thread::sleep(Duration::from_millis(5));
            }
            app.app().settle(8).unwrap();
            assert_eq!(agent.session.peek().status, "");
        };
        run_turn("Add a blue circle");
        assert_eq!(serialize_svg(&state.engine.peek().document), original);
        assert!(agent.session.peek().proposal.is_some());
        assert!(!state.engine.peek().is_dirty());

        let deadline = Instant::now() + Duration::from_secs(5);
        let context = std::task::Context::from_waker(std::task::Waker::noop());
        let mut raster = glory::apace::Raster::new();
        loop {
            app.observe(&context).unwrap();
            raster.draw(app.renderer()).unwrap();
            let has_blue_thumbnail =
                raster
                    .pixels()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .enumerate()
                    .any(|(index, pixel)| {
                        index % width as usize > (width - 300) as usize
                            && pixel[0] < 20
                            && pixel[1] < 20
                            && pixel[2] > 240
                    });
            if has_blue_thumbnail {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "preview never painted blue pixels in the inspector"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        press("Apply edit");
        assert!(state.engine.peek().is_dirty());
        let blue = serialize_svg(&state.engine.peek().document);
        let circle_id = state.engine.peek().document.find_by_xml_id("dot").unwrap();
        state.engine.write().selection.select(circle_id);
        run_turn("Make the circle red");
        assert_eq!(serialize_svg(&state.engine.peek().document), blue);
        assert!(agent
            .session
            .peek()
            .proposal
            .as_ref()
            .unwrap()
            .candidate
            .review
            .iter()
            .any(|line| line.contains("fill: #0000ff → red")));
        if let Some(path) = std::env::var_os("SIVGO_TEST_SNAPSHOT") {
            app.observe(&context).unwrap();
            raster.draw(app.renderer()).unwrap();
            let file = std::fs::File::create(path).unwrap();
            let mut encoder = png::Encoder::new(file, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(raster.pixels())
                .unwrap();
        }
        press("Apply edit");
        assert_eq!(
            state.engine.peek().document.find_by_xml_id("dot"),
            Some(circle_id)
        );
        assert!(state.engine.peek().selection.is_selected(circle_id));
        run_turn("What color is it?");
        assert!(agent.session.peek().proposal.is_none());
        assert_eq!(agent.session.peek().history.len(), 6);
        state.engine.write().undo();
        assert_eq!(serialize_svg(&state.engine.peek().document), blue);
        state.engine.write().undo();
        assert_eq!(serialize_svg(&state.engine.peek().document), original);
        assert!(!state.engine.peek().is_dirty());
        state.engine.write().redo();
        assert!(serialize_svg(&state.engine.peek().document).contains("circle"));
        upstream.join().unwrap();
    }
}
