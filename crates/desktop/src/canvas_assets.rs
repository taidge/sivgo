//! Bounded PNG data-URI transport for rendered canvas frames and toolbar icons.
//! No filesystem or network resource is accepted by this provider.
use base64::Engine;
use glory::apace::assets::{Bytes, Method, NetHandler, NetProvider, Request};
use std::{
    collections::VecDeque,
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::Notify;

const PREFIX: &str = "data:image/png;base64,";
const MAX_PNG_BYTES: usize = 8 * 1024 * 1024;
const MAX_URL_BYTES: usize = PREFIX.len() + MAX_PNG_BYTES.div_ceil(3) * 4;
const MAX_QUEUED: usize = 64;
const MAX_QUEUE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Default)]
struct Cancel {
    cancelled: AtomicBool,
    notify: Notify,
}
impl Cancel {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.notify.notify_one();
    }
    async fn wait(&self) {
        loop {
            let notified = self.notify.notified();
            if self.cancelled.load(Ordering::Acquire) {
                return;
            }
            notified.await;
        }
    }
}
struct Job {
    request: Request,
    response_url: String,
    handler: Box<dyn NetHandler>,
    cancel: Arc<Cancel>,
}
#[derive(Default)]
struct State {
    document: Option<usize>,
    next_response: u64,
    closed: bool,
    queue: VecDeque<Job>,
    queued_bytes: usize,
    running: Option<(String, Arc<Cancel>)>,
}
struct Inner {
    state: Mutex<State>,
    available: Notify,
}

/// One provider owns a single document and one worker thread. Dropping or closing
/// it cancels queued and preparing frames without joining on the UI thread.
pub struct CanvasImageProvider {
    inner: Arc<Inner>,
}
impl CanvasImageProvider {
    pub fn new() -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()?;
        let inner = Arc::new(Inner {
            state: Mutex::new(State::default()),
            available: Notify::new(),
        });
        let worker = inner.clone();
        std::thread::Builder::new()
            .name("sivgo-canvas-images".into())
            .spawn(move || {
                runtime.block_on(run(worker));
            })?;
        Ok(Self { inner })
    }
    fn shutdown(&self) {
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        state.closed = true;
        state.queue.clear();
        state.queued_bytes = 0;
        if let Some((_, cancel)) = &state.running {
            cancel.cancel();
        }
        drop(state);
        self.inner.available.notify_one();
    }
}
impl Drop for CanvasImageProvider {
    fn drop(&mut self) {
        self.shutdown();
    }
}
impl NetProvider for CanvasImageProvider {
    fn fetch(&self, doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let url = request.url.as_str();
        if request.method != Method::GET || !url.starts_with(PREFIX) || url.len() > MAX_URL_BYTES {
            handler.error(
                "sivgo-image://rejected".to_owned(),
                "Only bounded PNG data URIs are supported".into(),
            );
            return;
        }
        if request
            .signal
            .as_ref()
            .is_some_and(|signal| signal.aborted())
        {
            return;
        }
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed || state.document.is_some_and(|id| id != doc_id) {
            drop(state);
            handler.error(
                "sivgo-image://rejected".into(),
                "Canvas image document is closed or mismatched".into(),
            );
            return;
        }
        if state.queue.len() >= MAX_QUEUED
            || state.queued_bytes.saturating_add(url.len()) > MAX_QUEUE_BYTES
        {
            drop(state);
            handler.error(request.url.to_string(), "Canvas image queue is full".into());
            return;
        }
        let Some(sequence) = state.next_response.checked_add(1) else {
            drop(state);
            handler.error(
                "sivgo-image://rejected".into(),
                "Canvas response identity exhausted".into(),
            );
            return;
        };
        state.next_response = sequence;
        let response_url = format!("sivgo-image://document/{doc_id}/{sequence}.png");
        state.document = Some(doc_id);
        state.queued_bytes += url.len();
        state.queue.push_back(Job {
            request,
            response_url,
            handler,
            cancel: Arc::default(),
        });
        drop(state);
        self.inner.available.notify_one();
    }
    fn cancel(&self, doc_id: usize, url: &str) {
        let mut state = self.inner.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.document != Some(doc_id) {
            return;
        }
        state.queue.retain(|job| job.request.url.as_str() != url);
        state.queued_bytes = state
            .queue
            .iter()
            .map(|job| job.request.url.as_str().len())
            .sum();
        if let Some((active, cancel)) = &state.running {
            if active == url {
                cancel.cancel();
            }
        }
    }
    fn close(&self, doc_id: usize) {
        let matches = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .document
            .is_none_or(|id| id == doc_id);
        if matches {
            self.shutdown();
        }
    }
}
fn decode(url: &str) -> Result<Bytes, String> {
    let payload = url.strip_prefix(PREFIX).ok_or("Unsupported image URI")?;
    if url.len() > MAX_URL_BYTES {
        return Err("PNG data URI is too large".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|_| "Invalid PNG base64")?;
    if bytes.len() > MAX_PNG_BYTES || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Invalid or oversized PNG data".into());
    }
    Ok(Bytes::from(bytes))
}
fn aborted(job: &Job) -> bool {
    job.cancel.cancelled.load(Ordering::Acquire)
        || job
            .request
            .signal
            .as_ref()
            .is_some_and(|signal| signal.aborted())
}
async fn run(inner: Arc<Inner>) {
    loop {
        let available = inner.available.notified();
        let job = {
            let mut state = inner.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.closed {
                break;
            }
            let job = state.queue.pop_front();
            if let Some(job) = &job {
                state.queued_bytes -= job.request.url.as_str().len();
                state.running = Some((job.request.url.to_string(), job.cancel.clone()));
            }
            job
        };
        let Some(mut job) = job else {
            available.await;
            continue;
        };
        let url = job.request.url.to_string();
        // Apace caps response metadata URLs at 4096 bytes. A data URI can be
        // much larger; return a unique bounded alias as a redirected resource.
        // The engine matches the request ID and caches under the original URL,
        // which also remains our cancellation identity.
        let response_url = job.response_url.clone();
        if !aborted(&job) {
            let decoded = decode(&url);
            let outcome = match decoded {
                Ok(bytes) => {
                    let signal = job.request.signal.clone();
                    let cancel = job.cancel.clone();
                    let prepared = tokio::select! {
                        _ = cancel.wait() => None,
                        _ = async {
                            loop {
                                if signal.as_ref().is_some_and(|s| s.aborted()) { break; }
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                        } => None,
                        result = tokio::time::timeout(Duration::from_secs(30), job.handler.prepare(&response_url, &bytes)) => Some(result.unwrap_or_else(|_| Err("PNG decode admission timed out".into()))),
                    };
                    prepared.map(|result| result.map(|()| bytes))
                }
                Err(error) => Some(Err(error)),
            };
            // Publication and revocation share this short gate, preventing a
            // cancellation from racing a completed prepare into late delivery.
            let state = inner.state.lock().unwrap_or_else(|e| e.into_inner());
            if !state.closed && !aborted(&job) {
                if let Some(result) = outcome {
                    match result {
                        Ok(bytes) => job.handler.bytes(response_url, bytes),
                        Err(error) => job.handler.error(response_url, error),
                    }
                }
            }
            drop(state);
        }
        inner
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .running = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glory::apace::assets::{NetReady, Url};
    use std::sync::mpsc;
    type Reply = Result<(bool, Vec<u8>), String>;
    struct Handler {
        prepared: bool,
        sender: mpsc::Sender<Reply>,
    }
    impl NetHandler for Handler {
        fn prepare<'a>(&'a mut self, _: &'a str, _: &'a Bytes) -> NetReady<'a> {
            self.prepared = true;
            Box::pin(std::future::ready(Ok(())))
        }
        fn bytes(self: Box<Self>, _: String, bytes: Bytes) {
            let _ = self.sender.send(Ok((self.prepared, bytes.to_vec())));
        }
        fn error(self: Box<Self>, _: String, error: String) {
            let _ = self.sender.send(Err(error));
        }
    }
    fn uri() -> String {
        uri_color([255, 0, 0, 255])
    }
    fn uri_color(pixel: [u8; 4]) -> String {
        let mut encoded = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut encoded, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixel)
                .unwrap();
        }
        format!(
            "{PREFIX}{}",
            base64::engine::general_purpose::STANDARD.encode(encoded)
        )
    }
    fn request(provider: &CanvasImageProvider, url: &str) -> Reply {
        let (sender, receiver) = mpsc::channel();
        provider.fetch(
            1,
            Request::get(Url::parse(url).unwrap()),
            Box::new(Handler {
                prepared: false,
                sender,
            }),
        );
        receiver.recv_timeout(Duration::from_secs(5)).unwrap()
    }
    #[test]
    fn canvas_png_is_prepared_before_delivery_and_retains_pixels() {
        let provider = CanvasImageProvider::new().unwrap();
        let (prepared, bytes) = request(&provider, &uri()).unwrap();
        assert!(prepared);
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!((info.width, info.height), (1, 1));
        assert_eq!(&pixels[..info.buffer_size()], &[255, 0, 0, 255]);
    }
    #[test]
    fn refuses_network_files_other_mimes_and_malformed_png() {
        let provider = CanvasImageProvider::new().unwrap();
        for url in [
            "https://example.com/frame.png",
            "file:///C:/secret.png",
            "data:image/svg+xml;base64,PHN2Zy8+",
            "data:image/png;base64,broken",
            "data:image/png;base64,aGVsbG8=",
        ] {
            assert!(request(&provider, url).is_err(), "{url}");
        }
        provider.close(1);
        assert!(request(&provider, &uri()).is_err());
    }
    struct Pending {
        entered: mpsc::Sender<()>,
        completed: mpsc::Sender<()>,
    }
    impl NetHandler for Pending {
        fn prepare<'a>(&'a mut self, _: &'a str, _: &'a Bytes) -> NetReady<'a> {
            let _ = self.entered.send(());
            Box::pin(std::future::pending())
        }
        fn bytes(self: Box<Self>, _: String, _: Bytes) {
            let _ = self.completed.send(());
        }
        fn error(self: Box<Self>, _: String, _: String) {
            let _ = self.completed.send(());
        }
    }
    #[test]
    fn cancelling_prepare_unblocks_worker_and_revokes_delivery() {
        let provider = CanvasImageProvider::new().unwrap();
        let url = uri();
        let (entered, started) = mpsc::channel();
        let (completed, received) = mpsc::channel();
        provider.fetch(
            1,
            Request::get(Url::parse(&url).unwrap()),
            Box::new(Pending { entered, completed }),
        );
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        provider.cancel(1, &url);
        assert!(request(&provider, &url).is_ok());
        assert!(received.try_recv().is_err());
    }
    #[test]
    fn window_app_decodes_canvas_data_uri_and_paints_actual_red_pixels() {
        use glory::prelude::*;
        let provider = CanvasImageProvider::new().unwrap();
        let app = glory::apace::shell::WindowApp::with_config(glory::apace::Config {
            provider: Some(Arc::new(provider)),
            ..Default::default()
        });
        app.renderer().set_viewport(16, 16, 1.0).unwrap();
        app.renderer()
            .set_css("html, body { margin:0; padding:0; } img { display:block; }");
        app.mount(
            image().source(uri()).alt("Canvas pixel test").style(
                Style::new()
                    .width(Length::Px(16.0))
                    .height(Length::Px(16.0)),
            ),
        )
        .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let context = std::task::Context::from_waker(std::task::Waker::noop());
        while std::time::Instant::now() < deadline {
            app.observe(&context).unwrap();
            if app.renderer().resource_stats().cached_images == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let stats = app.renderer().resource_stats();
        assert_eq!(
            stats.cached_images, 1,
            "PNG must reach the real Apace decode/cache path: {stats:?}"
        );
        assert_eq!(stats.failures, 0);
        assert_eq!(stats.pending_images, 0);
        let mut raster = glory::apace::Raster::new();
        raster.draw(app.renderer()).unwrap();
        let red = raster
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[0] > 240 && pixel[1] < 10 && pixel[2] < 10 && pixel[3] > 240)
            .count();
        assert!(
            red >= 128,
            "expected the canvas PNG to cover the 16x16 frame; red pixels={red}"
        );
        app.app().clear().unwrap();
        app.observe(&context).unwrap();
        assert_eq!(app.renderer().resource_stats().requests, 0);
    }
    #[test]
    fn startup_burst_loads_twenty_six_images_without_queue_rejection() {
        use glory::prelude::*;
        let provider = Arc::new(CanvasImageProvider::new().unwrap());
        let app = glory::apace::shell::WindowApp::with_config(glory::apace::Config {
            provider: Some(provider.clone()),
            ..Default::default()
        });
        let doc_id = app.renderer().frame_info().document;
        // Hold admission while all startup image requests arrive, so this
        // checks burst capacity independently of the worker's scheduling speed.
        let blocker = uri_color([0, 255, 0, 255]);
        let (entered, started) = mpsc::channel();
        let (completed, _) = mpsc::channel();
        provider.fetch(
            doc_id,
            Request::get(Url::parse(&blocker).unwrap()),
            Box::new(Pending { entered, completed }),
        );
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        let images: Vec<Box<dyn Widget>> = (0..26)
            .map(|index| {
                Box::new(
                    image()
                        .source(uri_color([255, index, 0, 255]))
                        .alt(format!("Startup icon {index}")),
                ) as Box<dyn Widget>
            })
            .collect();
        app.mount(row().fill(images)).unwrap();
        let context = std::task::Context::from_waker(std::task::Waker::noop());
        app.observe(&context).unwrap();
        assert_eq!(provider.inner.state.lock().unwrap().queue.len(), 26);
        provider.cancel(doc_id, &blocker);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            app.observe(&context).unwrap();
            if app.renderer().resource_stats().cached_images == 26 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let stats = app.renderer().resource_stats();
        assert_eq!(
            stats.cached_images, 26,
            "all toolbar/canvas startup images must load: {stats:?}"
        );
        assert_eq!(stats.failures, 0);
        assert_eq!(stats.pending_images, 0);
    }
    #[test]
    fn full_editor_canvas_png_decodes_and_paints_absolutely_positioned_paper() {
        use glory::prelude::*;
        let mut engine = sivgo_core::EditorEngine::new(800.0, 600.0);
        engine.load_document(sivgo_dom::parse_svg(r##"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600"><rect x="40" y="40" width="200" height="160" fill="#ff0000"/></svg>"##).unwrap());
        let source = sivgo_ui::render_to_data_uri(&engine, 800, 600);
        let bytes = decode(&source).unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes.clone()))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut pixels).unwrap();
        assert_eq!(
            &pixels[..4],
            &[255, 255, 255, 255],
            "document source must contain white paper"
        );
        assert_eq!(
            &pixels[(60 * 800 + 60) * 4..(60 * 800 + 60) * 4 + 4],
            &[255, 0, 0, 255],
            "document source must contain red rectangle"
        );
        let app = glory::apace::shell::WindowApp::with_config(glory::apace::Config {
            provider: Some(Arc::new(CanvasImageProvider::new().unwrap())),
            ..Default::default()
        });
        app.renderer().set_viewport(840, 532, 1.0).unwrap();
        app.renderer().set_css("html,body{margin:0;padding:0;}");
        let reference = app.app().scope().cage(None);
        let source = app.app().scope().cage(source);
        app.mount(
            view()
                .style(
                    Style {
                        background: Some(Color(48, 49, 54, 255)),
                        ..Style::default()
                    }
                    .position(Position::Relative)
                    .width(Length::Px(840.0))
                    .height(Length::Px(532.0))
                    .overflow_x(Overflow::Hidden)
                    .overflow_y(Overflow::Hidden),
                )
                .fill(
                    image()
                        .node_ref(reference)
                        .source(source)
                        .alt("SVG drawing")
                        .style(
                            Style::new()
                                .position(Position::Absolute)
                                .left(Length::Px(0.0))
                                .top(Length::Px(0.0))
                                .width(Length::Px(800.0))
                                .height(Length::Px(600.0)),
                        ),
                ),
        )
        .unwrap();
        let context = std::task::Context::from_waker(std::task::Waker::noop());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            app.observe(&context).unwrap();
            if app.renderer().resource_stats().cached_images == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let stats = app.renderer().resource_stats();
        eprintln!(
            "full canvas encoded={} source={} stats={stats:?}",
            bytes.len(),
            source.get().len()
        );
        assert_eq!(stats.cached_images, 1);
        let mut raster = glory::apace::Raster::new();
        raster.draw(app.renderer()).unwrap();
        let white = raster
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] > 240 && p[1] > 240 && p[2] > 240 && p[3] > 240)
            .count();
        let red = raster
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] > 240 && p[1] < 10 && p[2] < 10 && p[3] > 240)
            .count();
        eprintln!("full canvas native white={white} red={red}");
        assert!(white > 300_000, "white paper must be visible");
        assert!(red > 30_000, "red rectangle must be visible");
        let original_source = source.get();
        engine.load_document(sivgo_dom::parse_svg(r##"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600"><rect x="40" y="40" width="200" height="160" fill="#00ff00"/></svg>"##).unwrap());
        source.revise(|source| *source = sivgo_ui::render_to_data_uri(&engine, 800, 600));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            app.observe(&context).unwrap();
            if app.renderer().resource_stats().cached_images == 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            app.renderer().resource_stats().cached_images,
            2,
            "distinct canvas frames retain distinct original cache identities"
        );
        assert_eq!(app.renderer().resource_stats().pending_images, 0);
        assert_eq!(app.renderer().resource_stats().failures, 0);
        raster.draw(app.renderer()).unwrap();
        let green = raster
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] < 10 && p[1] > 240 && p[2] < 10 && p[3] > 240)
            .count();
        let red = raster
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] > 240 && p[1] < 10 && p[2] < 10 && p[3] > 240)
            .count();
        assert!(green > 30_000, "new canvas frame must be painted");
        assert_eq!(red, 0, "old canvas frame must be replaced");
        source.revise(|source| *source = original_source);
        app.observe(&context).unwrap();
        raster.draw(app.renderer()).unwrap();
        let red = raster
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] > 240 && p[1] < 10 && p[2] < 10 && p[3] > 240)
            .count();
        assert!(
            red > 30_000,
            "returning to the first source reuses the matching cached frame"
        );
        assert_eq!(app.renderer().resource_stats().cached_images, 2);
    }
}
