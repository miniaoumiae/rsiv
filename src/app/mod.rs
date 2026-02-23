mod camera;
mod cursor;
mod gallery;
mod winit_handler;

pub use camera::Camera;
pub use cursor::{CursorState, CursorZone};
pub use gallery::Gallery;

use crate::cache::CacheManager;
use crate::image_item::{ImageItem, ImageSlot};
use crate::keybinds::Action;
use crate::loader::Loader;
use crate::status_bar::{StatusBar, StatusContext};
use crate::view_mode::ViewMode;
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
    MetadataError(usize, PathBuf, String),
    DiscoveryComplete,
    ImagePixelsLoaded(PathBuf, Arc<crate::image_item::LoadedAsset>),
    ThumbnailLoaded(PathBuf, Arc<(u32, u32, Vec<u8>)>),
    LoadError(PathBuf, String),
    LoadCancelled(PathBuf),
    FileChanged(ImageItem),
    FileDeleted(PathBuf),
    HandlerFinished,
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

    fn reset_view_for_new_image(&mut self) {
        self.camera.off_x = 0;
        self.camera.off_y = 0;
        self.playback.reset();
    }

    fn handle_navigation_action(&mut self, action: Action, count: usize) -> bool {
        let mut needs_redraw = false;
        match action {
            Action::NextImage => {
                if !self.gallery.filtered.is_empty() {
                    self.gallery.current_index =
                        (self.gallery.current_index + count) % self.gallery.filtered.len();
                    self.reset_view_for_new_image();
                    needs_redraw = true;
                }
            }
            Action::PrevImage => {
                if !self.gallery.filtered.is_empty() {
                    let len = self.gallery.filtered.len();
                    self.gallery.current_index =
                        (self.gallery.current_index + len - (count % len)) % len;
                    self.reset_view_for_new_image();
                    needs_redraw = true;
                }
            }
            Action::FirstImage => {
                if !self.gallery.filtered.is_empty() {
                    let target = if count > 1 {
                        count.saturating_sub(1)
                    } else {
                        0
                    };
                    self.gallery.current_index = target.min(self.gallery.filtered.len() - 1);
                    self.reset_view_for_new_image();
                    needs_redraw = true;
                }
            }
            Action::LastImage => {
                if !self.gallery.filtered.is_empty() {
                    self.gallery.current_index = self.gallery.filtered.len() - 1;
                    self.reset_view_for_new_image();
                    needs_redraw = true;
                }
            }
            Action::NextFrame => {
                if !self.gallery.filtered.is_empty() {
                    if let ImageSlot::MetadataLoaded(item) =
                        &self.gallery.filtered[self.gallery.current_index]
                    {
                        if let Some(img) = self.assets.cache.get_image(&item.path) {
                            let frame_count = img.frame_count();
                            if frame_count > 1 {
                                self.playback.is_playing = false;
                                self.playback.current_frame_index =
                                    (self.playback.current_frame_index + count) % frame_count;
                                needs_redraw = true;
                            }
                        }
                    }
                }
            }
            Action::PrevFrame => {
                if !self.gallery.filtered.is_empty() {
                    if let ImageSlot::MetadataLoaded(item) =
                        &self.gallery.filtered[self.gallery.current_index]
                    {
                        if let Some(img) = self.assets.cache.get_image(&item.path) {
                            let frame_count = img.frame_count();
                            if frame_count > 1 {
                                self.playback.is_playing = false;
                                let jump = count % frame_count;
                                self.playback.current_frame_index =
                                    (self.playback.current_frame_index + frame_count - jump)
                                        % frame_count;
                                needs_redraw = true;
                            }
                        }
                    }
                }
            }
            Action::NextMark => {
                if !self.gallery.filtered.is_empty() && !self.gallery.marked_files.is_empty() {
                    for _ in 0..count {
                        for i in 1..self.gallery.filtered.len() {
                            let idx =
                                (self.gallery.current_index + i) % self.gallery.filtered.len();
                            if let ImageSlot::MetadataLoaded(item) = &self.gallery.filtered[idx] {
                                if self
                                    .gallery
                                    .marked_files
                                    .contains(&item.path.to_string_lossy().to_string())
                                {
                                    self.gallery.current_index = idx;
                                    break;
                                }
                            }
                        }
                    }
                    self.reset_view_for_new_image();
                    needs_redraw = true;
                }
            }
            Action::PrevMark => {
                if !self.gallery.filtered.is_empty() && !self.gallery.marked_files.is_empty() {
                    for _ in 0..count {
                        for i in 1..self.gallery.filtered.len() {
                            let idx = (self.gallery.current_index + self.gallery.filtered.len()
                                - i)
                                % self.gallery.filtered.len();
                            if let ImageSlot::MetadataLoaded(item) = &self.gallery.filtered[idx] {
                                if self
                                    .gallery
                                    .marked_files
                                    .contains(&item.path.to_string_lossy().to_string())
                                {
                                    self.gallery.current_index = idx;
                                    break;
                                }
                            }
                        }
                    }
                    self.reset_view_for_new_image();
                    needs_redraw = true;
                }
            }
            _ => {}
        }
        needs_redraw
    }

    fn handle_grid_movement_action(&mut self, action: Action, count: usize) -> bool {
        let mut needs_redraw = false;
        match action {
            Action::GridMoveLeft => {
                if self.gallery.current_index >= count {
                    self.gallery.current_index -= count;
                    needs_redraw = true;
                } else if self.gallery.current_index > 0 {
                    self.gallery.current_index = 0;
                    needs_redraw = true;
                }
            }
            Action::GridMoveRight => {
                if self.gallery.current_index + count < self.gallery.filtered.len() {
                    self.gallery.current_index += count;
                    needs_redraw = true;
                } else if self.gallery.current_index < self.gallery.filtered.len() - 1 {
                    self.gallery.current_index = self.gallery.filtered.len() - 1;
                    needs_redraw = true;
                }
            }
            Action::GridMoveUp => {
                if let Some(w) = &self.window {
                    let config = crate::config::AppConfig::get();
                    let cell_size = config.options.thumbnail_size + config.options.grid_padding;
                    let width = w.inner_size().width;
                    let cols = (width / cell_size).max(1);
                    let jump = cols as usize * count;

                    if self.gallery.current_index >= jump {
                        self.gallery.current_index -= jump;
                        needs_redraw = true;
                    } else if self.gallery.current_index >= cols as usize {
                        // If we can't jump full count but can jump at least one row,
                        self.gallery.current_index %= cols as usize;
                        needs_redraw = true;
                    }
                }
            }
            Action::GridMoveDown => {
                if let Some(w) = &self.window {
                    let config = crate::config::AppConfig::get();
                    let cell_size = config.options.thumbnail_size + config.options.grid_padding;
                    let width = w.inner_size().width;
                    let cols = (width / cell_size).max(1);
                    let jump = cols as usize * count;

                    if self.gallery.current_index + jump < self.gallery.filtered.len() {
                        self.gallery.current_index += jump;
                        needs_redraw = true;
                    } else {
                        // Go to last row same column if possible
                        let mut next = self.gallery.current_index;
                        while next + (cols as usize) < self.gallery.filtered.len() {
                            next += cols as usize;
                        }
                        if next != self.gallery.current_index {
                            self.gallery.current_index = next;
                            needs_redraw = true;
                        }
                    }
                }
            }
            Action::GridMovePageUp => {
                if let Some(w) = &self.window {
                    let config = crate::config::AppConfig::get();
                    let cell_size = config.options.thumbnail_size + config.options.grid_padding;
                    let s = w.inner_size();
                    let mut h = s.height;
                    if self.show_status_bar {
                        h = h.saturating_sub(self.status_bar.height);
                    }
                    let cols = (s.width / cell_size).max(1);
                    let rows = (h / cell_size).max(1);
                    let jump_rows = (rows / 2).max(1);
                    let jump_idx = (jump_rows * cols) as usize * count;

                    if self.gallery.current_index >= jump_idx {
                        self.gallery.current_index -= jump_idx;
                        needs_redraw = true;
                    } else if self.gallery.current_index > 0 {
                        self.gallery.current_index = 0;
                        needs_redraw = true;
                    }
                }
            }
            Action::GridMovePageDown => {
                if let Some(w) = &self.window {
                    let config = crate::config::AppConfig::get();
                    let cell_size = config.options.thumbnail_size + config.options.grid_padding;
                    let s = w.inner_size();
                    let mut h = s.height;
                    if self.show_status_bar {
                        h = h.saturating_sub(self.status_bar.height);
                    }
                    let cols = (s.width / cell_size).max(1);
                    let rows = (h / cell_size).max(1);
                    let jump_rows = (rows / 2).max(1);
                    let jump_idx = (jump_rows * cols) as usize * count;

                    if self.gallery.current_index + jump_idx < self.gallery.filtered.len() {
                        self.gallery.current_index += jump_idx;
                        needs_redraw = true;
                    } else if self.gallery.current_index < self.gallery.filtered.len() - 1 {
                        self.gallery.current_index = self.gallery.filtered.len() - 1;
                        needs_redraw = true;
                    }
                }
            }
            _ => {}
        }
        needs_redraw
    }

    fn handle_view_action(&mut self, action: Action, old_scale: f64) -> bool {
        let mut needs_redraw = false;
        let mut changed_scale = false;
        let config = crate::config::AppConfig::get();
        let step = config.options.pan_step;
        let zoom_step = if config.options.zoom_step <= 0.0 {
            1.0
        } else {
            config.options.zoom_step
        };

        match action {
            Action::ResetView => {
                self.camera.off_x = 0;
                self.camera.off_y = 0;
                needs_redraw = true;
            }
            Action::FitToWindow => {
                self.camera.mode = ViewMode::FitToWindow;
                if config.options.auto_center {
                    self.camera.off_x = 0;
                    self.camera.off_y = 0;
                }
                needs_redraw = true;
            }
            Action::BestFit => {
                self.camera.mode = ViewMode::BestFit;
                if config.options.auto_center {
                    self.camera.off_x = 0;
                    self.camera.off_y = 0;
                }
                needs_redraw = true;
            }
            Action::Cover => {
                self.camera.mode = ViewMode::Cover;
                if config.options.auto_center {
                    self.camera.off_x = 0;
                    self.camera.off_y = 0;
                }
                needs_redraw = true;
            }
            Action::FitWidth => {
                self.camera.mode = ViewMode::FitWidth;
                if config.options.auto_center {
                    self.camera.off_x = 0;
                    self.camera.off_y = 0;
                }
                needs_redraw = true;
            }
            Action::FitHeight => {
                self.camera.mode = ViewMode::FitHeight;
                if config.options.auto_center {
                    self.camera.off_x = 0;
                    self.camera.off_y = 0;
                }
                needs_redraw = true;
            }
            Action::PanLeft => {
                self.camera.off_x += step;
                needs_redraw = true;
            }
            Action::PanRight => {
                self.camera.off_x -= step;
                needs_redraw = true;
            }
            Action::PanUp => {
                self.camera.off_y += step;
                needs_redraw = true;
            }
            Action::PanDown => {
                self.camera.off_y -= step;
                needs_redraw = true;
            }
            Action::PanToLeftEdge => {
                let (buf_w, _) = self.get_available_window_size().unwrap_or((0.0, 0.0));
                if let ImageSlot::MetadataLoaded(item) =
                    &self.gallery.filtered[self.gallery.current_index]
                {
                    let scaled_w = item.width as f64 * old_scale;
                    self.camera.off_x = ((scaled_w - buf_w) / 2.0) as i32;
                }
                needs_redraw = true;
            }
            Action::PanToRightEdge => {
                let (buf_w, _) = self.get_available_window_size().unwrap_or((0.0, 0.0));
                if let ImageSlot::MetadataLoaded(item) =
                    &self.gallery.filtered[self.gallery.current_index]
                {
                    let scaled_w = item.width as f64 * old_scale;
                    self.camera.off_x = -((scaled_w - buf_w) / 2.0) as i32;
                }
                needs_redraw = true;
            }
            Action::PanToTopEdge => {
                let (_, buf_h) = self.get_available_window_size().unwrap_or((0.0, 0.0));
                if let ImageSlot::MetadataLoaded(item) =
                    &self.gallery.filtered[self.gallery.current_index]
                {
                    let scaled_h = item.height as f64 * old_scale;
                    self.camera.off_y = ((scaled_h - buf_h) / 2.0) as i32;
                }
                needs_redraw = true;
            }
            Action::PanToBottomEdge => {
                let (_, buf_h) = self.get_available_window_size().unwrap_or((0.0, 0.0));
                if let ImageSlot::MetadataLoaded(item) =
                    &self.gallery.filtered[self.gallery.current_index]
                {
                    let scaled_h = item.height as f64 * old_scale;
                    self.camera.off_y = -((scaled_h - buf_h) / 2.0) as i32;
                }
                needs_redraw = true;
            }
            Action::ZoomReset => {
                self.camera.mode = ViewMode::Absolute;
                if config.options.auto_center {
                    self.camera.off_x = 0;
                    self.camera.off_y = 0;
                }
                needs_redraw = true;
            }
            Action::ZoomIn => {
                self.camera.mode =
                    ViewMode::Zoom((old_scale * zoom_step).min(config.options.zoom_max));
                changed_scale = true;
            }
            Action::ZoomOut => {
                self.camera.mode =
                    ViewMode::Zoom((old_scale / zoom_step).max(config.options.zoom_min));
                changed_scale = true;
            }
            _ => {}
        }

        if changed_scale {
            let new_scale = self.get_current_scale();
            self.camera.off_x = (self.camera.off_x as f64 * (new_scale / old_scale)) as i32;
            self.camera.off_y = (self.camera.off_y as f64 * (new_scale / old_scale)) as i32;
            needs_redraw = true;
        }

        if needs_redraw {
            self.clamp_offsets();
        }

        needs_redraw
    }

    fn handle_image_ops_action(&mut self, action: Action, count: usize) -> bool {
        let mut needs_redraw = false;
        match action {
            Action::MarkFile => {
                if !self.gallery.filtered.is_empty() {
                    if count > 1 {
                        for i in 0..count {
                            let idx =
                                (self.gallery.current_index + i) % self.gallery.filtered.len();
                            if let ImageSlot::MetadataLoaded(item) = &self.gallery.filtered[idx] {
                                let path = item.path.to_string_lossy().to_string();
                                if !self.gallery.marked_files.remove(&path) {
                                    self.gallery.marked_files.insert(path);
                                }
                            }
                        }
                        self.gallery.current_index =
                            (self.gallery.current_index + count) % self.gallery.filtered.len();
                    } else if let ImageSlot::MetadataLoaded(item) =
                        &self.gallery.filtered[self.gallery.current_index]
                    {
                        let path = item.path.to_string_lossy().to_string();
                        if !self.gallery.marked_files.remove(&path) {
                            self.gallery.marked_files.insert(path);
                        }
                    }
                    needs_redraw = true;
                }
            }
            Action::RemoveImage => {
                if !self.gallery.filtered.is_empty() {
                    for _ in 0..count {
                        if self.gallery.filtered.is_empty() {
                            break;
                        }
                        let path_to_remove = if let ImageSlot::MetadataLoaded(item) =
                            &self.gallery.filtered[self.gallery.current_index]
                        {
                            Some(item.path.clone())
                        } else {
                            None
                        };
                        if let Some(p) = &path_to_remove {
                            self.gallery
                                .marked_files
                                .remove(&p.to_string_lossy().to_string());
                        }
                        self.gallery.filtered.remove(self.gallery.current_index);
                        if let Some(p) = path_to_remove {
                            self.gallery.all.retain(|slot| {
                                if let ImageSlot::MetadataLoaded(item) = slot {
                                    item.path != p
                                } else {
                                    true
                                }
                            });
                        }
                        if self.gallery.filtered.is_empty() {
                            self.gallery.current_index = 0;
                        } else if self.gallery.current_index >= self.gallery.filtered.len() {
                            self.gallery.current_index = self.gallery.filtered.len() - 1;
                        }
                    }
                    self.reset_view_for_new_image();
                    needs_redraw = true;
                }
            }
            Action::ToggleMarks => {
                for item_slot in &self.gallery.filtered {
                    if let ImageSlot::MetadataLoaded(item) = item_slot {
                        let path = item.path.to_string_lossy().to_string();
                        if !self.gallery.marked_files.remove(&path) {
                            self.gallery.marked_files.insert(path);
                        }
                    }
                }
                needs_redraw = true;
            }
            Action::UnmarkAll => {
                self.gallery.marked_files.clear();
                needs_redraw = true;
            }
            Action::RotateCW => {
                needs_redraw = self
                    .gallery
                    .mutate_current_image(&mut self.assets.cache, |img| img.rotate(true));
                if needs_redraw {
                    self.camera.off_x = 0;
                    self.camera.off_y = 0;
                }
            }
            Action::RotateCCW => {
                needs_redraw = self
                    .gallery
                    .mutate_current_image(&mut self.assets.cache, |img| img.rotate(false));
                if needs_redraw {
                    self.camera.off_x = 0;
                    self.camera.off_y = 0;
                }
            }
            Action::FlipHorizontal => {
                needs_redraw = self
                    .gallery
                    .mutate_current_image(&mut self.assets.cache, |img| {
                        img.flip_horizontal();
                        false
                    });
            }
            Action::FlipVertical => {
                needs_redraw = self
                    .gallery
                    .mutate_current_image(&mut self.assets.cache, |img| {
                        img.flip_vertical();
                        false
                    });
            }
            _ => {}
        }
        needs_redraw
    }

    fn handle_toggle_action(&mut self, action: Action, prefix: Option<usize>) -> bool {
        let mut needs_redraw = false;
        match action {
            Action::ToggleSlideshow => {
                if let Some(n) = prefix {
                    // User typed a number (e.g. "10s")
                    // Set delay and force ON
                    let secs = n.max(1) as u64;
                    self.playback.slideshow_delay = Duration::from_secs(secs);
                    self.playback.slideshow_on = true;
                    self.playback.last_slide_time = Instant::now();
                } else {
                    // User just typed "s"
                    // Toggle state, keep existing delay
                    self.playback.slideshow_on = !self.playback.slideshow_on;
                    self.playback.last_slide_time = Instant::now(); // Reset timer on toggle
                }
                needs_redraw = true;
            }
            Action::ToggleStatusBar => {
                self.show_status_bar = !self.show_status_bar;
                needs_redraw = true;
            }
            Action::ToggleGrid => {
                self.camera.grid_mode = !self.camera.grid_mode;
                if !self.camera.grid_mode {
                    self.reset_view_for_new_image();
                }
                if let Some(w) = &self.window {
                    let width = w.inner_size().width as f64;
                    self.cursor.refresh_icon(
                        width,
                        self.camera.grid_mode,
                        &self.input.mode,
                        Some(w),
                    );
                } else {
                    self.cursor
                        .refresh_icon(0.0, self.camera.grid_mode, &self.input.mode, None);
                }
                needs_redraw = true;
            }
            Action::ToggleAnimation => {
                self.playback.is_playing = !self.playback.is_playing;
                needs_redraw = true;
            }
            Action::ToggleAlpha => {
                self.camera.show_alpha = !self.camera.show_alpha;
                needs_redraw = true;
            }
            _ => {}
        }
        needs_redraw
    }

    fn render(&mut self) {
        let scale = self.get_current_scale();

        if !self.gallery.filtered.is_empty() {
            // Slideshow Logic
            if self.playback.slideshow_on {
                let now = Instant::now();
                if now.duration_since(self.playback.last_slide_time)
                    >= self.playback.slideshow_delay
                {
                    self.handle_navigation_action(Action::NextImage, 1);
                    self.playback.last_slide_time = now;
                }
                // Keep the loop running for slideshow
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            // Request Logic
            if self.camera.grid_mode {
                if let Some(w) = &self.window {
                    let config = crate::config::AppConfig::get();
                    let cell_size = config.options.thumbnail_size + config.options.grid_padding;
                    let buf_w = w.inner_size().width;
                    let buf_h = w.inner_size().height; // Approximate
                    let cols = (buf_w / cell_size).max(1);

                    let current_row = (self.gallery.current_index as u32) / cols;
                    let scroll_y = if current_row * cell_size > buf_h / 2 {
                        (current_row * cell_size) as i32 - (buf_h as i32 / 2)
                            + (cell_size as i32 / 2)
                    } else {
                        0
                    };

                    let start_row = scroll_y.max(0) as u32 / cell_size;
                    let rows_visible = (buf_h / cell_size) + 2;

                    let start_idx = (start_row * cols) as usize;
                    let end_idx = ((start_row + rows_visible) * cols) as usize;
                    let end_idx = end_idx.min(self.gallery.filtered.len());

                    for i in start_idx..end_idx {
                        if let ImageSlot::MetadataLoaded(item) = &self.gallery.filtered[i] {
                            // Check cache & pending
                            if self.assets.cache.get_thumbnail(&item.path).is_none()
                                && !self.assets.pending.contains(&item.path)
                            {
                                self.assets.pending.insert(item.path.clone());
                                // Request load
                                self.assets.loader.request_thumbnail(
                                    item.path.clone(),
                                    item.format,
                                    config.options.thumbnail_size,
                                );
                            }
                        }
                    }
                }
            } else {
                // Single view
                if let ImageSlot::MetadataLoaded(item) =
                    &self.gallery.filtered[self.gallery.current_index]
                {
                    let config = crate::config::AppConfig::get();
                    if self.assets.cache.get_image(&item.path).is_none()
                        && !self.assets.pending.contains(&item.path)
                    {
                        self.assets.pending.insert(item.path.clone());
                        self.assets
                            .loader
                            .request_image(item.path.clone(), item.format);
                    }

                    // Pre-fetch ahead
                    for offset in 1..=config.options.preload_ahead {
                        let idx = self.gallery.current_index + offset;
                        if idx >= self.gallery.filtered.len() {
                            break;
                        }
                        if let ImageSlot::MetadataLoaded(next) = &self.gallery.filtered[idx] {
                            if self.assets.cache.get_image(&next.path).is_none()
                                && !self.assets.pending.contains(&next.path)
                            {
                                self.assets.pending.insert(next.path.clone());
                                self.assets
                                    .loader
                                    .request_image(next.path.clone(), next.format);
                            }
                        }
                    }
                    // Pre-fetch behind
                    for offset in 1..=config.options.preload_behind {
                        let Some(idx) = self.gallery.current_index.checked_sub(offset) else {
                            break;
                        };
                        if let ImageSlot::MetadataLoaded(prev) = &self.gallery.filtered[idx] {
                            if self.assets.cache.get_image(&prev.path).is_none()
                                && !self.assets.pending.contains(&prev.path)
                            {
                                self.assets.pending.insert(prev.path.clone());
                                self.assets
                                    .loader
                                    .request_image(prev.path.clone(), prev.format);
                            }
                        }
                    }
                }
            }

            // Animation
            if !self.camera.grid_mode {
                if let ImageSlot::MetadataLoaded(item) =
                    &self.gallery.filtered[self.gallery.current_index]
                {
                    if let Some(loaded_image) = self.assets.cache.get_image(&item.path) {
                        let now = Instant::now();
                        let dt = now.duration_since(self.playback.last_update);
                        self.playback.last_update = now;

                        let frame_count = loaded_image.frame_count();

                        if self.playback.is_playing && frame_count > 1 {
                            self.playback.frame_timer += dt;
                            let current_delay =
                                loaded_image.frame_delay(self.playback.current_frame_index);
                            let effective_delay = if current_delay.is_zero() {
                                Duration::from_millis(100)
                            } else {
                                current_delay
                            };

                            if self.playback.frame_timer >= effective_delay {
                                self.playback.frame_timer = Duration::ZERO;
                                self.playback.current_frame_index =
                                    (self.playback.current_frame_index + 1) % frame_count;
                            }
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                    }
                }
            }
        }

        // Clear background and get pixels
        let Some(pixels) = &mut self.pixels else {
            return;
        };

        let frame_slice = pixels.frame_mut();
        let config = crate::config::AppConfig::get();
        let bg_color = crate::utils::parse_color(&config.ui.bg_color);
        crate::renderer::clear(frame_slice, bg_color);

        let (buf_w, buf_h) = if let Some(w) = &self.window {
            let s = w.inner_size();
            (s.width as i32, s.height as i32)
        } else {
            return;
        };

        let available_h = if self.show_status_bar {
            buf_h - self.status_bar.height as i32
        } else {
            buf_h
        };

        // Draw images/grid
        if !self.gallery.filtered.is_empty() {
            if self.camera.grid_mode {
                let colors = crate::renderer::GridColors {
                    bg: bg_color,
                    accent: crate::utils::parse_color(&config.ui.thumbnail_border_color),
                    mark: crate::utils::parse_color(&config.ui.mark_color),
                    loading: crate::utils::parse_color(&config.ui.loading_color),
                    error: crate::utils::parse_color(&config.ui.error_color),
                };

                crate::renderer::draw_grid(
                    frame_slice,
                    buf_w,
                    available_h,
                    &self.gallery.filtered,
                    &self.assets.cache,
                    self.gallery.current_index,
                    &colors,
                    &self.gallery.marked_files,
                );
            } else if let ImageSlot::MetadataLoaded(item) =
                &self.gallery.filtered[self.gallery.current_index]
            {
                if let Some(loaded_image) = self.assets.cache.get_image(&item.path) {
                    let params = crate::renderer::DrawImageParams {
                        asset: &loaded_image,
                        frame_idx: self.playback.current_frame_index,
                        scale,
                        off_x: self.camera.off_x,
                        off_y: self.camera.off_y,
                        show_alpha: self.camera.show_alpha,
                    };
                    crate::renderer::draw_image(frame_slice, buf_w, available_h, &params);
                }
            }
        }

        // Draw Status Bar
        if self.show_status_bar && buf_h > 0 {
            let mut fb =
                crate::frame_buffer::FrameBuffer::new(frame_slice, buf_w as u32, buf_h as u32);

            let error_storage;
            let (path_str, is_marked, scale_percent, index, total) =
                if self.gallery.filtered.is_empty() {
                    ("No matches", false, 100, 0, 0)
                } else {
                    match &self.gallery.filtered[self.gallery.current_index] {
                        ImageSlot::MetadataLoaded(item) => {
                            let is_marked = self
                                .gallery
                                .marked_files
                                .contains(&item.path.to_string_lossy().to_string());
                            let is_loaded = self.assets.cache.get_image(&item.path).is_some();
                            let s = if self.camera.grid_mode || !is_loaded {
                                100
                            } else {
                                (scale * 100.0) as u32
                            };
                            (
                                item.path.to_str().unwrap_or(""),
                                is_marked,
                                s,
                                self.gallery.current_index + 1,
                                self.gallery.filtered.len(),
                            )
                        }
                        ImageSlot::Error(err) => {
                            error_storage = format!("Error: {}", err);
                            (
                                error_storage.as_str(),
                                false,
                                0,
                                self.gallery.current_index + 1,
                                self.gallery.filtered.len(),
                            )
                        }
                        ImageSlot::PendingMetadata => (
                            "Discovering...",
                            false,
                            0,
                            self.gallery.current_index + 1,
                            self.gallery.filtered.len(),
                        ),
                    }
                };

            let (current_frame, total_frames) = if !self.gallery.filtered.is_empty() {
                if let ImageSlot::MetadataLoaded(item) =
                    &self.gallery.filtered[self.gallery.current_index]
                {
                    if let Some(img) = self.assets.cache.get_image(&item.path) {
                        (self.playback.current_frame_index + 1, img.frame_count())
                    } else {
                        (0, 0)
                    }
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            };

            let spinner_frame = if self.input.is_handler_running {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    / 100) as usize
            } else {
                0
            };

            let ctx = StatusContext {
                scale_percent,
                index,
                total,
                path: path_str,
                is_marked,
                input_mode: &self.input.mode,
                prefix_count: self.input.prefix_count,
                slideshow_on: self.playback.slideshow_on,
                slideshow_delay: self.playback.slideshow_delay,
                filter_text: &self.gallery.filter_text,
                current_frame,
                total_frames,
                spinner_frame,
                is_handler_running: self.input.is_handler_running,
            };

            self.status_bar.draw(&mut fb, ctx);
        }

        if let Err(err) = pixels.render() {
            crate::rsiv_err!("Pixels render error: {}", err);
        }
    }
}
