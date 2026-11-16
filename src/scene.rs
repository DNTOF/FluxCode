//! Animated backgrounds: five looping pixel videos (ocean, river, beach, mountains,
//! anime dusk) behind the panels.
//!
//! The videos are rendered ahead of time (tools/backgrounds/render.py: tiny 4 x 5 pixel
//! character cells, so they read as detailed pixel video) and stored as animated WebP,
//! the video format GPUI decodes itself. Nothing is drawn per frame here: a background
//! thread decodes one frame at a time, the backdrop shows it, and the frame before it
//! leaves the GPU atlas, so memory stays at a frame or two whatever the length.
//!
//! The files are not compiled in (about 3-5 MB each): the first time a scene is picked
//! it is downloaded from the repository into `~/.fluxcode/backgrounds`.
//!
//! The scene is its own view under the workbench, which is drawn as a cached view
//! (`Shell`): a new frame repaints the backdrop and replays the workbench's last frame
//! instead of rebuilding it.

use gpui_kit::{prelude::FluentBuilder, *};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene {
    Ocean,
    River,
    Beach,
    Mountains,
    Anime,
}

impl Scene {
    pub const ALL: [Scene; 5] = [Scene::Ocean, Scene::River, Scene::Beach, Scene::Mountains, Scene::Anime];

    pub fn id(self) -> &'static str {
        match self {
            Scene::Ocean => "ocean",
            Scene::River => "river",
            Scene::Beach => "beach",
            Scene::Mountains => "mountains",
            Scene::Anime => "anime",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Scene::Ocean => "Ocean",
            Scene::River => "River",
            Scene::Beach => "Beach",
            Scene::Mountains => "Mountains",
            Scene::Anime => "Anime",
        }
    }

    pub fn from_id(id: &str) -> Option<Scene> {
        Scene::ALL.into_iter().find(|s| s.id() == id)
    }

    /// The color under the video while it loads (the video's own darkest tone).
    fn paper(self) -> Hsla {
        Hsla::from(rgb(match self {
            Scene::Ocean => 0x050C1C,
            Scene::River => 0x0B1B20,
            Scene::Beach => 0x1A1230,
            Scene::Mountains => 0x120F24,
            Scene::Anime => 0x150826,
        }))
    }
}

/// 0: no scene, n: `Scene::ALL[n - 1]`.
static SCENE: AtomicU8 = AtomicU8::new(0);
static ANIMATE: AtomicBool = AtomicBool::new(true);

/// Which scene shows (None: none) and whether it plays (off: its first frame stays).
pub fn set(scene: Option<Scene>, animate: bool) {
    let n = scene.and_then(|s| Scene::ALL.iter().position(|x| *x == s)).map_or(0, |i| i as u8 + 1);
    SCENE.store(n, Ordering::Relaxed);
    ANIMATE.store(animate, Ordering::Relaxed);
}

pub fn current() -> Option<Scene> {
    match SCENE.load(Ordering::Relaxed) {
        0 => None,
        n => Scene::ALL.get(n as usize - 1).copied(),
    }
}

pub fn active() -> bool {
    current().is_some()
}

const BASE_URL: &str = "https://raw.githubusercontent.com/MuaraAI/FluxCode/main/assets/backgrounds";

fn cache_dir() -> PathBuf {
    crate::ext::home().join(".fluxcode").join("backgrounds")
}

/// The scene's video on disk, if it is there: the download cache, then (in a
/// development build) the repository's own copy.
fn video_path(scene: Scene) -> Option<PathBuf> {
    let name = format!("{}.webp", scene.id());
    let cached = cache_dir().join(&name);
    if cached.is_file() {
        return Some(cached);
    }
    if cfg!(debug_assertions) {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets").join("backgrounds").join(&name);
        if repo.is_file() {
            return Some(repo);
        }
    }
    None
}

/// Fetch the scene's video into the cache (blocking; run it off the UI thread).
fn download(scene: Scene) -> Result<PathBuf, String> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let to = dir.join(format!("{}.webp", scene.id()));
    let part = dir.join(format!("{}.webp.part", scene.id()));
    let url = format!("{BASE_URL}/{}.webp", scene.id());
    let out = crate::proc::command(if cfg!(windows) { "curl.exe" } else { "curl" })
        .args(["-s", "-S", "-f", "-L", "--max-time", "180", "--max-filesize", "40000000", "-H", "User-Agent: FluxCode", "-o"])
        .arg(&part)
        .arg(&url)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("curl could not start ({e})"))?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&part);
        return Err(crate::acp::curl_error(&String::from_utf8_lossy(&out.stderr)));
    }
    std::fs::rename(&part, &to).map_err(|e| e.to_string())?;
    Ok(to)
}

/// Decode the video one frame at a time and hand each over; the channel holds two,
/// so decoding runs only as fast as the frames are shown. Stops when the receiver
/// is dropped (another scene, or the scene turned off).
fn play(path: PathBuf, animate: bool) -> Receiver<Arc<RenderImage>> {
    let (tx, rx) = sync_channel(2);
    std::thread::Builder::new()
        .name("backdrop-video".into())
        .spawn(move || {
            use image::AnimationDecoder;
            let Ok(bytes) = std::fs::read(&path) else { return };
            loop {
                let Ok(decoder) = image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(&bytes)) else { return };
                let mut any = false;
                for frame in decoder.into_frames() {
                    let Ok(mut frame) = frame else { break };
                    // GPUI's images are BGRA.
                    for px in frame.buffer_mut().as_chunks_mut::<4>().0 {
                        px.swap(0, 2);
                    }
                    any = true;
                    if tx.send(Arc::new(RenderImage::new(vec![frame]))).is_err() || !animate {
                        return;
                    }
                }
                if !any {
                    return;
                }
            }
        })
        .ok();
    rx
}

/// The window root: the scene, and the workbench over it as a cached view.
pub struct Shell {
    backdrop: Entity<Backdrop>,
    workbench: Entity<crate::workbench::Workbench>,
}

impl Shell {
    pub fn new(workbench: Entity<crate::workbench::Workbench>, cx: &mut Context<Self>) -> Self {
        Shell { backdrop: cx.new(|_| Backdrop::default()), workbench }
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut style = StyleRefinement::default();
        style.size.width = Some(relative(1.).into());
        style.size.height = Some(relative(1.).into());
        div()
            .size_full()
            .relative()
            .when(active(), |d| d.child(div().absolute().inset_0().child(self.backdrop.clone())))
            .child(self.workbench.clone().cached(style))
    }
}

#[derive(Default)]
pub struct Backdrop {
    /// What the player was started for: the scene and whether it plays.
    playing: Option<(Scene, bool)>,
    frames: Option<Receiver<Arc<RenderImage>>>,
    current: Option<Arc<RenderImage>>,
    /// Frames replaced since the last paint, to take out of the GPU atlas.
    stale: Vec<Arc<RenderImage>>,
    /// A scene being downloaded.
    fetching: Option<Scene>,
    /// The frame timer is running.
    ticking: bool,
    /// The window is in the background: frames come slower.
    idle: bool,
}

/// The videos are made at 10 frames a second.
const FRAME: Duration = Duration::from_millis(100);
const IDLE_FRAME: Duration = Duration::from_millis(400);

impl Backdrop {
    fn start(&mut self, scene: Scene, animate: bool, cx: &mut Context<Self>) {
        self.playing = Some((scene, animate));
        self.frames = None;
        if let Some(old) = self.current.take() {
            self.stale.push(old);
        }
        match video_path(scene) {
            Some(path) => {
                self.frames = Some(play(path, animate));
                self.tick(cx);
            }
            None if self.fetching != Some(scene) => {
                self.fetching = Some(scene);
                cx.spawn(async move |this, cx| {
                    let got = cx.background_spawn(async move { download(scene) }).await;
                    _ = this.update(cx, |b, cx| {
                        b.fetching = None;
                        if got.is_ok() {
                            // Start again now that the file is there.
                            b.playing = None;
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            None => {}
        }
    }

    /// Take the next decoded frame on the video's own clock.
    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.ticking {
            return;
        }
        self.ticking = true;
        cx.spawn(async move |this, cx| loop {
            let idle = this.read_with(cx, |b, _| b.idle).unwrap_or(true);
            cx.background_executor().timer(if idle { IDLE_FRAME } else { FRAME }).await;
            let go = this
                .update(cx, |b, cx| {
                    let Some(rx) = &b.frames else {
                        b.ticking = false;
                        return false;
                    };
                    match rx.try_recv() {
                        Ok(frame) => {
                            if let Some(old) = b.current.replace(frame) {
                                b.stale.push(old);
                            }
                            cx.notify();
                            true
                        }
                        Err(TryRecvError::Empty) => true,
                        // A still picture (animation off) ends after its one frame.
                        Err(TryRecvError::Disconnected) => {
                            b.frames = None;
                            b.ticking = false;
                            false
                        }
                    }
                })
                .unwrap_or(false);
            if !go {
                break;
            }
        })
        .detach();
    }
}

impl Render for Backdrop {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.idle = !window.is_window_active();
        let want = current().map(|s| (s, ANIMATE.load(Ordering::Relaxed)));
        if want != self.playing {
            match want {
                Some((scene, animate)) => self.start(scene, animate, cx),
                None => {
                    self.playing = None;
                    self.frames = None;
                    if let Some(old) = self.current.take() {
                        self.stale.push(old);
                    }
                }
            }
        }
        let paper = current().map(Scene::paper).unwrap_or_default();
        let image = self.current.clone();
        let stale = std::mem::take(&mut self.stale);
        canvas(|_, _, _| (), move |b, _, window, _| {
            for old in stale {
                _ = window.drop_image(old);
            }
            window.paint_quad(fill(b, paper));
            let Some(img) = image else { return };
            let s = img.size(0);
            let (iw, ih) = (s.width.0 as f32, s.height.0 as f32);
            if iw <= 0. || ih <= 0. {
                return;
            }
            // Cover the window, cropping the longer side, like `object-fit: cover`.
            let (bw, bh) = (f32::from(b.size.width), f32::from(b.size.height));
            let k = (bw / iw).max(bh / ih);
            let (w, h) = (iw * k, ih * k);
            let at = Bounds::new(point(b.origin.x + px((bw - w) / 2.), b.origin.y + px((bh - h) / 2.)), size(px(w), px(h)));
            _ = window.paint_image(b, at, Corners::default(), img, 0, false);
            // A light scrim: bright spots (a sun, foam) never fight the text on top.
            window.paint_quad(fill(b, hsla(0., 0., 0., 0.28)));
        })
        .size_full()
    }
}

#[cfg(test)]
mod tests {
    use super::Scene;

    #[test]
    fn scene_ids_round_trip() {
        for s in Scene::ALL {
            assert_eq!(Scene::from_id(s.id()), Some(s));
        }
        assert_eq!(Scene::from_id("none"), None);
    }
}
