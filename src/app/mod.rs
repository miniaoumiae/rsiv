mod actions;
mod camera;
mod cursor;
mod gallery;
mod render;
mod winit_handler;

pub use camera::Camera;
pub use cursor::{CursorState, CursorZone, build_cursor};
pub use gallery::Gallery;

use crate::cache::CacheManager;
use crate::image_item::{ImageItem, ImageSlot};
use crate::loader::{Loader, LoaderError};
use crate::status_bar::StatusBar;
use pixels::Pixels;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::event_loop::EventLoopProxy;
use winit::keyboard::ModifiersState;
use winit::window::Window;

use std::sync::atomic::AtomicBool;

#[derive(Debug)]
pub enum AppEvent {
    InitialCount(usize),
    MetadataLoaded(usize, ImageItem),
    MetadataError(usize, PathBuf, LoaderError),
    DiscoveryComplete,
    ImagePixelsLoaded(PathBuf, Arc<crate::image_item::LoadedAsset>),
    ThumbnailLoaded(PathBuf, Arc<(u32, u32, Vec<u8>)>),
    LoadError(PathBuf, LoaderError),
    LoadCancelled(PathBuf),
    FileChanged(ImageItem),
    FileDeleted(PathBuf),
    HandlerFinished,
    Ipc(
        crate::ipc::IpcRequest,
        std::sync::mpsc::Sender<crate::ipc::IpcResponse>,
    ),
}

#[derive(Debug, PartialEq, Clone)]
pub enum InputMode {
    Normal,
    Filtering,
    WaitingForHandler,
    AwaitingTarget(String),
}

pub struct Playback {
    pub current_frame_index: usize,
    pub is_playing: bool,
    pub last_update: Instant,
    pub frame_timer: Duration,
    pub slideshow_on: bool,
    pub slideshow_delay: Duration,
    pub last_slide_time: Instant,
}

impl Playback {
    pub fn reset(&mut self) {
        let config = crate::config::AppConfig::get();
        self.current_frame_index = 0;
        self.frame_timer = Duration::ZERO;
        self.is_playing = config.options.autoplay_animations;
    }
}

pub struct InputState {
    pub modifiers: ModifiersState,
    pub mode: InputMode,
    pub prefix_count: Option<usize>,
    pub bindings: Vec<crate::keybinds::Binding>,
    pub handler_cancel_flag: Arc<AtomicBool>,
    pub is_handler_running: bool,
}

impl InputState {
    pub fn pop_count(&mut self) -> usize {
        let val = self.prefix_count.take();
        val.unwrap_or(1).max(1)
    }
}

pub struct AssetManager {
    pub loader: Loader,
    pub cache: CacheManager,
    pub pending: HashSet<PathBuf>,
}

pub struct App {
    pub gallery: Gallery,
    pub camera: Camera,
    pub playback: Playback,
    pub input: InputState,
    pub cursor: CursorState,
    pub assets: AssetManager,

    // Core OS / Rendering components
    pub window: Option<Arc<Window>>,
    pub pixels: Option<Pixels<'static>>,
    pub proxy: EventLoopProxy<AppEvent>,

    // Top-level UI components
    pub status_bar: StatusBar,
    pub show_status_bar: bool,
}

impl App {
    pub fn new(
        images: Vec<ImageSlot>,
        start_in_grid_mode: bool,
        proxy: EventLoopProxy<AppEvent>,
    ) -> Self {
        let config = crate::config::AppConfig::get();

        Self {
            gallery: Gallery {
                all: images.clone(),
                filtered: images,
                current_index: 0,
                filter_text: String::new(),
                marked_files: HashSet::new(),
                discovery_complete: false,
            },
            camera: Camera {
                mode: config.options.default_view,
                off_x: 0,
                off_y: 0,
                grid_mode: start_in_grid_mode,
                show_alpha: false,
            },
            playback: Playback {
                current_frame_index: 0,
                is_playing: config.options.autoplay_animations,
                last_update: Instant::now(),
                frame_timer: Duration::ZERO,
                slideshow_on: false,
                slideshow_delay: Duration::from_secs(config.options.slideshow_default_delay),
                last_slide_time: Instant::now(),
            },
            input: InputState {
                modifiers: ModifiersState::default(),
                mode: InputMode::Normal,
                prefix_count: None,
                bindings: crate::keybinds::Binding::get_all_bindings(),
                handler_cancel_flag: Arc::new(AtomicBool::new(false)),
                is_handler_running: false,
            },
            cursor: CursorState {
                pos: None,
                zone: CursorZone::None,
                next_icon: None,
                prev_icon: None,
                is_dragging: false,
                drag_start_pos: None,
                camera_start_pos: None,
            },
            assets: AssetManager {
                loader: Loader::new(proxy.clone()),
                cache: CacheManager::new(config.options.max_memory_percent),
                pending: HashSet::new(),
            },
            window: None,
            pixels: None,
            proxy,
            status_bar: StatusBar::new(),
            show_status_bar: true,
        }
    }

    pub fn pop_count(&mut self) -> usize {
        self.input.pop_count()
    }

    fn get_available_window_size(&self) -> Option<(f64, f64)> {
        if let Some(w) = &self.window {
            let s = w.inner_size();
            let mut h = s.height as f64;
            if self.show_status_bar {
                h -= self.status_bar.height as f64;
            }
            Some((s.width as f64, h))
        } else {
            None
        }
    }

    fn get_current_scale(&self) -> f64 {
        if self.gallery.filtered.is_empty() {
            return 1.0;
        }
        let ImageSlot::MetadataLoaded(item) = &self.gallery.filtered[self.gallery.current_index]
        else {
            return 1.0;
        };

        let (buf_w, buf_h) = if let Some((w, h)) = self.get_available_window_size() {
            (w, h)
        } else {
            return 1.0;
        };

        self.camera
            .get_current_scale((buf_w, buf_h), (item.width as f64, item.height as f64))
    }

    pub fn clamp_offsets(&mut self) {
        if self.gallery.filtered.is_empty() || self.camera.grid_mode {
            return;
        }

        let ImageSlot::MetadataLoaded(item) = &self.gallery.filtered[self.gallery.current_index]
        else {
            return;
        };

        let Some((buf_w, buf_h)) = self.get_available_window_size() else {
            return;
        };

        let scale = self.get_current_scale();
        let scaled_w = item.width as f64 * scale;
        let scaled_h = item.height as f64 * scale;

        self.camera
            .clamp_offsets((buf_w, buf_h), (scaled_w, scaled_h));
    }
}
