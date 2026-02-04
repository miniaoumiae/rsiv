use crate::cache::CacheManager;
use crate::image_item::{ImageItem, ImageSlot};
use crate::keybinds::Action;
use crate::loader::Loader;
use crate::status_bar::StatusBar;
use crate::view_mode::ViewMode;
use pixels::{Pixels, SurfaceTexture};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use winit::platform::wayland::WindowAttributesExtWayland;
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use winit::platform::x11::WindowAttributesExtX11;

#[derive(Debug)]
pub enum AppEvent {
    InitialCount(usize),
    MetadataLoaded(usize, ImageItem),
    MetadataError(usize, String),
    DiscoveryComplete,
    ImagePixelsLoaded(PathBuf, Arc<crate::image_item::LoadedImage>),
    ThumbnailLoaded(PathBuf, Arc<(u32, u32, Vec<u8>)>),
    LoadError(PathBuf, String),
}

#[derive(Debug, PartialEq, Clone)]
pub enum InputMode {
    Normal,
    WaitingForHandler,
    AwaitingTarget(String),
}

pub struct App {
    pub images: Vec<ImageSlot>,
    pub current_index: usize,
    pub mode: ViewMode,
    pub off_x: i32,
    pub off_y: i32,
    pub window: Option<Arc<Window>>,
    pub pixels: Option<Pixels<'static>>,

    // Resources
    pub loader: Loader,
    pub cache: CacheManager,
    pub pending: HashSet<PathBuf>, // Track what we've already sent to the loader

    // Animation state
    pub current_frame_index: usize,
    pub is_playing: bool,
    pub last_update: Instant,
    pub frame_timer: Duration,

    // Input state
    pub modifiers: ModifiersState,
    pub input_mode: InputMode,

    // UI
    pub status_bar: StatusBar,
    pub show_status_bar: bool,
    pub discovery_complete: bool,
    pub grid_mode: bool,
    pub marked_files: HashSet<String>,
    pub bindings: Vec<crate::keybinds::Binding>,
}

impl App {
    pub fn new(
        images: Vec<ImageSlot>,
        start_in_grid_mode: bool,
        proxy: EventLoopProxy<AppEvent>,
    ) -> Self {
        let config = crate::config::AppConfig::get();

        Self {
            images,
            current_index: 0,
            mode: config.options.default_view,
            off_x: 0,
            off_y: 0,
            window: None,
            pixels: None,
            loader: Loader::new(proxy),
            cache: CacheManager::new(
                config.options.image_cache_size,
                config.options.thumb_cache_size,
            ),
            pending: HashSet::new(),
            current_frame_index: 0,
            is_playing: true,
            last_update: Instant::now(),
            frame_timer: Duration::ZERO,
            input_mode: InputMode::Normal,
            modifiers: ModifiersState::default(),
            status_bar: StatusBar::new(),
            show_status_bar: true,
            discovery_complete: false,
            grid_mode: start_in_grid_mode,
            marked_files: HashSet::new(),
            bindings: crate::keybinds::Binding::get_all_bindings(),
        }
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
        if self.images.is_empty() {
            return 1.0;
        }
        let ImageSlot::MetadataLoaded(item) = &self.images[self.current_index] else {
            return 1.0;
        };

        let (buf_w, buf_h) = if let Some((w, h)) = self.get_available_window_size() {
            (w, h)
        } else {
            return 1.0;
        };

        if buf_w <= 0.0 || buf_h <= 0.0 {
            return 1.0;
        }

        match self.mode {
            ViewMode::Absolute => 1.0,
            ViewMode::Zoom(s) => {
                let config = crate::config::AppConfig::get();
                s.clamp(config.options.zoom_min, config.options.zoom_max)
            }
            ViewMode::FitToWindow => {
                let w_ratio = buf_w / item.width as f64;
                let h_ratio = buf_h / item.height as f64;
                w_ratio.min(h_ratio)
            }
            ViewMode::BestFit => {
                let w_ratio = buf_w / item.width as f64;
                let h_ratio = buf_h / item.height as f64;
                w_ratio.min(h_ratio).min(1.0)
            }
            ViewMode::FitWidth => buf_w / item.width as f64,
            ViewMode::FitHeight => buf_h / item.height as f64,
        }
    }

    fn reset_view_for_new_image(&mut self) {
        self.off_x = 0;
        self.off_y = 0;
        self.current_frame_index = 0;
        self.frame_timer = Duration::ZERO;
        self.is_playing = true;
    }

    fn mutate_current_image<F>(&mut self, f: F) -> bool
    where
        F: FnOnce(&mut crate::image_item::LoadedImage) -> bool,
    {
        let Some(ImageSlot::MetadataLoaded(item)) = self.images.get_mut(self.current_index) else {
            return false;
        };

        // Get the pixels from cache
        if let Some(arc_image) = self.cache.image_cache.get(&item.path) {
            // Clone the Arc so we can mutate the inner data (Copy-on-Write)
            let mut loaded_image = (**arc_image).clone();

            // Apply the transformation
            let dimensions_changed = f(&mut loaded_image);

            // If rotation happened, update the metadata dimensions
            if dimensions_changed {
                item.width = loaded_image.width;
                item.height = loaded_image.height;
            }

            // Update the Cache with new pixels
            let path = item.path.clone();
            self.cache
                .insert_image(path.clone(), Arc::new(loaded_image));

            // IMPORTANT: Invalidate the thumbnail so it gets re-generated correctly
            self.cache.thumb_cache.pop(&path);

            return true;
        }
        false
    }

    fn handle_navigation_action(&mut self, action: Action) -> bool {
        let mut needs_redraw = false;
        match action {
            Action::NextImage => {
                if !self.images.is_empty() {
                    self.current_index = (self.current_index + 1) % self.images.len();
                    self.reset_view_for_new_image();
                    needs_redraw = true;
                }
            }
            Action::PrevImage => {
                if !self.images.is_empty() {
                    self.current_index =
                        (self.current_index + self.images.len() - 1) % self.images.len();
                    self.reset_view_for_new_image();
                    needs_redraw = true;
                }
            }
            Action::FirstImage => {
                if !self.images.is_empty() {
                    self.current_index = 0;
                    self.reset_view_for_new_image();
                    needs_redraw = true;
                }
            }
            Action::LastImage => {
                if !self.images.is_empty() {
                    self.current_index = self.images.len() - 1;
                    self.reset_view_for_new_image();
                    needs_redraw = true;
                }
            }
            Action::NextMark => {
                if !self.images.is_empty() && !self.marked_files.is_empty() {
                    for i in 1..self.images.len() {
                        let idx = (self.current_index + i) % self.images.len();
                        if let ImageSlot::MetadataLoaded(item) = &self.images[idx] {
                            if self
                                .marked_files
                                .contains(&item.path.to_string_lossy().to_string())
                            {
                                self.current_index = idx;
                                self.reset_view_for_new_image();
                                needs_redraw = true;
                                break;
                            }
                        }
                    }
                }
            }
            Action::PrevMark => {
                if !self.images.is_empty() && !self.marked_files.is_empty() {
                    for i in 1..self.images.len() {
                        let idx = (self.current_index + self.images.len() - i) % self.images.len();
                        if let ImageSlot::MetadataLoaded(item) = &self.images[idx] {
                            if self
                                .marked_files
                                .contains(&item.path.to_string_lossy().to_string())
                            {
                                self.current_index = idx;
                                self.reset_view_for_new_image();
                                needs_redraw = true;
                                break;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        needs_redraw
    }

    fn handle_grid_movement_action(&mut self, action: Action) -> bool {
        let mut needs_redraw = false;
        match action {
            Action::GridMoveLeft => {
                if self.current_index > 0 {
                    self.current_index -= 1;
                    needs_redraw = true;
                }
            }
            Action::GridMoveRight => {
                if self.current_index < self.images.len() - 1 {
                    self.current_index += 1;
                    needs_redraw = true;
                }
            }
            Action::GridMoveUp => {
                if let Some(w) = &self.window {
                    let config = crate::config::AppConfig::get();
                    let cell_size = config.options.thumbnail_size + config.options.grid_pading;
                    let width = w.inner_size().width;
                    let cols = (width / cell_size).max(1);
                    if self.current_index >= cols as usize {
                        self.current_index -= cols as usize;
                        needs_redraw = true;
                    }
                }
            }
            Action::GridMoveDown => {
                if let Some(w) = &self.window {
                    let config = crate::config::AppConfig::get();
                    let cell_size = config.options.thumbnail_size + config.options.grid_pading;
                    let width = w.inner_size().width;
                    let cols = (width / cell_size).max(1);
                    if self.current_index + (cols as usize) < self.images.len() {
                        self.current_index += cols as usize;
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
        let step = 50;
        let config = crate::config::AppConfig::get();

        match action {
            Action::ResetView => {
                self.off_x = 0;
                self.off_y = 0;
                needs_redraw = true;
            }
            Action::FitToWindow => {
                self.mode = ViewMode::FitToWindow;
                if config.options.auto_center {
                    self.off_x = 0;
                    self.off_y = 0;
                }
                needs_redraw = true;
            }
            Action::BestFit => {
                self.mode = ViewMode::BestFit;
                if config.options.auto_center {
                    self.off_x = 0;
                    self.off_y = 0;
                }
                needs_redraw = true;
            }
            Action::FitWidth => {
                self.mode = ViewMode::FitWidth;
                if config.options.auto_center {
                    self.off_x = 0;
                    self.off_y = 0;
                }
                needs_redraw = true;
            }
            Action::FitHeight => {
                self.mode = ViewMode::FitHeight;
                if config.options.auto_center {
                    self.off_x = 0;
                    self.off_y = 0;
                }
                needs_redraw = true;
            }
            Action::PanLeft => {
                self.off_x += step;
                needs_redraw = true;
            }
            Action::PanRight => {
                self.off_x -= step;
                needs_redraw = true;
            }
            Action::PanUp => {
                self.off_y += step;
                needs_redraw = true;
            }
            Action::PanDown => {
                self.off_y -= step;
                needs_redraw = true;
            }
            Action::ZoomReset => {
                self.mode = ViewMode::Absolute;
                if config.options.auto_center {
                    self.off_x = 0;
                    self.off_y = 0;
                }
                needs_redraw = true;
            }
            Action::ZoomIn => {
                self.mode = ViewMode::Zoom((old_scale * 1.1).min(config.options.zoom_max));
                changed_scale = true;
            }
            Action::ZoomOut => {
                self.mode = ViewMode::Zoom((old_scale / 1.1).max(config.options.zoom_min));
                changed_scale = true;
            }
            _ => {}
        }

        if changed_scale {
            let new_scale = self.get_current_scale();
            self.off_x = (self.off_x as f64 * (new_scale / old_scale)) as i32;
            self.off_y = (self.off_y as f64 * (new_scale / old_scale)) as i32;
            needs_redraw = true;
        }

        needs_redraw
    }

    fn handle_image_ops_action(&mut self, action: Action) -> bool {
        let mut needs_redraw = false;
        match action {
            Action::MarkFile => {
                if !self.images.is_empty() {
                    if let ImageSlot::MetadataLoaded(item) = &self.images[self.current_index] {
                        let path = item.path.to_string_lossy().to_string();
                        if self.marked_files.contains(&path) {
                            self.marked_files.remove(&path);
                        } else {
                            self.marked_files.insert(path);
                        }
                        needs_redraw = true;
                    }
                }
            }
            Action::RemoveImage => {
                if !self.images.is_empty() {
                    if let ImageSlot::MetadataLoaded(item) = &self.images[self.current_index] {
                        self.marked_files
                            .remove(&item.path.to_string_lossy().to_string());
                    }
                    self.images.remove(self.current_index);
                    if self.images.is_empty() {
                        self.current_index = 0;
                    } else if self.current_index >= self.images.len() {
                        self.current_index = self.images.len() - 1;
                    }
                    self.reset_view_for_new_image();
                    needs_redraw = true;
                }
            }
            Action::ToggleMarks => {
                for item_slot in &self.images {
                    if let ImageSlot::MetadataLoaded(item) = item_slot {
                        let path = item.path.to_string_lossy().to_string();
                        if !self.marked_files.remove(&path) {
                            self.marked_files.insert(path);
                        }
                    }
                }
                needs_redraw = true;
            }
            Action::UnmarkAll => {
                self.marked_files.clear();
                needs_redraw = true;
            }
            Action::RotateCW => {
                needs_redraw = self.mutate_current_image(|img| {
                    img.rotate(true);
                    true // dimensions changed
                });
                if needs_redraw {
                    self.reset_view_for_new_image();
                }
            }
            Action::RotateCCW => {
                needs_redraw = self.mutate_current_image(|img| {
                    img.rotate(false);
                    true // dimensions changed
                });
                if needs_redraw {
                    self.reset_view_for_new_image();
                }
            }
            Action::FlipHorizontal => {
                needs_redraw = self.mutate_current_image(|img| {
                    img.flip_horizontal();
                    false // dimensions didn't change
                });
            }
            Action::FlipVertical => {
                needs_redraw = self.mutate_current_image(|img| {
                    img.flip_vertical();
                    false // dimensions didn't change
                });
            }
            _ => {}
        }
        needs_redraw
    }

    fn handle_toggle_action(&mut self, action: Action) -> bool {
        let mut needs_redraw = false;
        match action {
            Action::ToggleStatusBar => {
                self.show_status_bar = !self.show_status_bar;
                needs_redraw = true;
            }
            Action::ToggleGrid => {
                self.grid_mode = !self.grid_mode;
                if !self.grid_mode {
                    self.reset_view_for_new_image();
                }
                needs_redraw = true;
            }
            Action::ToggleAnimation => {
                self.is_playing = !self.is_playing;
                needs_redraw = true;
            }
            _ => {}
        }
        needs_redraw
    }

    fn render(&mut self) {
        if self.images.is_empty() {
            return;
        }

        // --- 1. Request Logic (Pull Architecture) ---
        if self.grid_mode {
            if let Some(w) = &self.window {
                let config = crate::config::AppConfig::get();
                let cell_size = config.options.thumbnail_size + config.options.grid_pading;
                let buf_w = w.inner_size().width;
                let buf_h = w.inner_size().height; // Approximate
                let cols = (buf_w / cell_size).max(1);

                // Calculate visible rows
                // We need to know the scroll_y which is computed in draw_grid based on current_index
                // For now, let's replicate the logic or approximate it.
                // In draw_grid:
                // let current_row = (selected_idx as u32) / cols;
                // let scroll_y = if current_row * cell_size > buf_h as u32 / 2 { ... }

                let current_row = (self.current_index as u32) / cols;
                let scroll_y = if current_row * cell_size > buf_h / 2 {
                    (current_row * cell_size) as i32 - (buf_h as i32 / 2) + (cell_size as i32 / 2)
                } else {
                    0
                };

                let start_row = scroll_y.max(0) as u32 / cell_size;
                let rows_visible = (buf_h / cell_size) + 2;

                let start_idx = (start_row * cols) as usize;
                let end_idx = ((start_row + rows_visible) * cols) as usize;
                let end_idx = end_idx.min(self.images.len());

                for i in start_idx..end_idx {
                    if let ImageSlot::MetadataLoaded(item) = &self.images[i] {
                        // Check cache & pending
                        if self.cache.get_thumbnail(&item.path).is_none()
                            && !self.pending.contains(&item.path)
                        {
                            self.pending.insert(item.path.clone());
                            // Request load
                            self.loader.request_thumbnail(
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
            if let ImageSlot::MetadataLoaded(item) = &self.images[self.current_index] {
                if self.cache.get_image(&item.path).is_none() && !self.pending.contains(&item.path)
                {
                    self.pending.insert(item.path.clone());
                    self.loader.request_image(item.path.clone(), item.format);
                }

                // Pre-fetch next
                if self.current_index + 1 < self.images.len() {
                    if let ImageSlot::MetadataLoaded(next) = &self.images[self.current_index + 1] {
                        if self.cache.get_image(&next.path).is_none()
                            && !self.pending.contains(&next.path)
                        {
                            self.pending.insert(next.path.clone());
                            self.loader.request_image(next.path.clone(), next.format);
                        }
                    }
                }
                // Pre-fetch prev
                if self.current_index > 0 {
                    if let ImageSlot::MetadataLoaded(prev) = &self.images[self.current_index - 1] {
                        if self.cache.get_image(&prev.path).is_none()
                            && !self.pending.contains(&prev.path)
                        {
                            self.pending.insert(prev.path.clone());
                            self.loader.request_image(prev.path.clone(), prev.format);
                        }
                    }
                }
            }
        }

        // Animation
        if !self.grid_mode {
            if let ImageSlot::MetadataLoaded(item) = &self.images[self.current_index] {
                if let Some(loaded_image) = self.cache.get_image(&item.path) {
                    let now = Instant::now();
                    let dt = now.duration_since(self.last_update);
                    self.last_update = now;

                    let frame_count = loaded_image.frames.len();

                    if self.is_playing && frame_count > 1 {
                        self.frame_timer += dt;
                        let current_delay = loaded_image.frames[self.current_frame_index].delay;
                        let effective_delay = if current_delay.is_zero() {
                            Duration::from_millis(100)
                        } else {
                            current_delay
                        };

                        if self.frame_timer >= effective_delay {
                            self.frame_timer = Duration::ZERO;
                            self.current_frame_index = (self.current_frame_index + 1) % frame_count;
                        }
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                }
            }
        }

        // Draw
        let scale = self.get_current_scale();
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

        if self.grid_mode {
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
                &self.images,
                &mut self.cache,
                self.current_index,
                &colors,
                &self.marked_files,
            );
        } else {
            if let ImageSlot::MetadataLoaded(item) = &self.images[self.current_index] {
                // Check if loaded
                if let Some(loaded_image) = self.cache.get_image(&item.path) {
                    let params = crate::renderer::DrawImageParams {
                        image: &loaded_image,
                        frame_idx: self.current_frame_index,
                        scale,
                        off_x: self.off_x,
                        off_y: self.off_y,
                    };
                    crate::renderer::draw_image(frame_slice, buf_w, available_h, &params);
                } else {
                    // Draw Loading Text (using Status Bar renderer or simple center text?)
                    // For now, we rely on Status Bar "Loading..."
                }
            }
        }

        if self.show_status_bar && buf_h > 0 {
            let mut fb =
                crate::frame_buffer::FrameBuffer::new(frame_slice, buf_w as u32, buf_h as u32);

            match &self.images[self.current_index] {
                ImageSlot::MetadataLoaded(item) => {
                    let is_marked = self
                        .marked_files
                        .contains(&item.path.to_string_lossy().to_string());

                    // Check if pixels are actually loaded for the status text
                    let is_loaded = self.cache.get_image(&item.path).is_some();

                    if is_loaded || self.grid_mode {
                        self.status_bar.draw(
                            &mut fb,
                            if self.grid_mode {
                                100
                            } else {
                                (scale * 100.0) as u32
                            },
                            self.current_index + 1,
                            self.images.len(),
                            &item.path.to_string_lossy(),
                            is_marked,
                            &self.input_mode,
                        );
                    } else {
                        self.status_bar.draw(
                            &mut fb,
                            0,
                            self.current_index + 1,
                            self.images.len(),
                            "Loading...",
                            is_marked,
                            &self.input_mode,
                        );
                    }
                }
                ImageSlot::Error(err) => {
                    self.status_bar.draw(
                        &mut fb,
                        0,
                        self.current_index + 1,
                        self.images.len(),
                        &format!("Error: {}", err),
                        false,
                        &self.input_mode,
                    );
                }
                ImageSlot::PendingMetadata => {
                    self.status_bar.draw(
                        &mut fb,
                        0,
                        self.current_index + 1,
                        self.images.len(),
                        "Discovering...",
                        false,
                        &self.input_mode,
                    );
                }
            }
        }

        if let Err(err) = pixels.render() {
            eprintln!("Pixels render error: {}", err);
        }
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut attributes = Window::default_attributes().with_title("rsiv");

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            attributes = WindowAttributesExtWayland::with_name(attributes, "rsiv", "rsiv");
            attributes = WindowAttributesExtX11::with_name(attributes, "rsiv", "rsiv");
        }

        let window = Arc::new(event_loop.create_window(attributes).unwrap());

        let size = window.inner_size();
        let surface_texture = SurfaceTexture::new(size.width, size.height, window.clone());
        let pixels = Pixels::new(size.width, size.height, surface_texture).unwrap();

        self.window = Some(window.clone());
        self.pixels = Some(pixels);

        let scale_factor = window.scale_factor();
        self.status_bar.set_scale(scale_factor as f32);
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::InitialCount(count) => {
                self.images = vec![ImageSlot::PendingMetadata; count];
            }
            AppEvent::MetadataLoaded(idx, item) => {
                if let Some(slot) = self.images.get_mut(idx) {
                    *slot = ImageSlot::MetadataLoaded(item);
                }
                if self.current_index == idx {
                    self.window.as_ref().unwrap().request_redraw();
                }
            }
            AppEvent::MetadataError(idx, err) => {
                if let Some(slot) = self.images.get_mut(idx) {
                    *slot = ImageSlot::Error(err);
                }
            }
            AppEvent::DiscoveryComplete => {
                self.discovery_complete = true;
                if self.images.is_empty() {
                    eprintln!("No images found. Exiting...");
                    _el.exit();
                }
            }
            AppEvent::ImagePixelsLoaded(path, image) => {
                self.pending.remove(&path);
                self.cache.insert_image(path.clone(), image);
                if let ImageSlot::MetadataLoaded(item) = &self.images[self.current_index] {
                    if item.path == path {
                        self.window.as_ref().unwrap().request_redraw();
                    }
                }
            }
            AppEvent::ThumbnailLoaded(path, thumb) => {
                self.pending.remove(&path);
                self.cache.insert_thumbnail(path, thumb);
                if self.grid_mode {
                    self.window.as_ref().unwrap().request_redraw();
                }
            }
            AppEvent::LoadError(path, _err) => {
                self.pending.remove(&path);
            }
        }
    }

    fn window_event(&mut self, _el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => _el.exit(),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::RedrawRequested => self.render(),
            WindowEvent::Resized(new_size) => {
                if let Some(pixels) = &mut self.pixels {
                    if new_size.width > 0 && new_size.height > 0 {
                        let _ = pixels.resize_surface(new_size.width, new_size.height);
                        let _ = pixels.resize_buffer(new_size.width, new_size.height);
                    }
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.status_bar.set_scale(scale_factor as f32);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    if self.input_mode != InputMode::Normal {
                        use winit::keyboard::{Key, NamedKey};

                        let key_to_process = match &event.logical_key {
                            Key::Named(NamedKey::Escape) => Some("Esc"),
                            Key::Character(c) => Some(c.as_str()),
                            _ => None,
                        };

                        if let Some(k) = key_to_process {
                            self.handle_modal_input(k);
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                            return; // Block normal keybindings
                        }
                    }

                    let old_scale = self.get_current_scale();
                    let mut needs_redraw = false;

                    if let Some(action) = crate::keybinds::Binding::resolve(
                        &event,
                        &self.bindings,
                        self.modifiers,
                        self.grid_mode,
                    ) {
                        match action {
                            Action::Quit => _el.exit(),
                            Action::ScriptHandlerPrefix => {
                                self.input_mode = InputMode::WaitingForHandler;
                                needs_redraw = true;
                            }
                            a @ (Action::NextImage
                            | Action::PrevImage
                            | Action::FirstImage
                            | Action::LastImage
                            | Action::NextMark
                            | Action::PrevMark) => {
                                needs_redraw = self.handle_navigation_action(a);
                            }
                            a @ (Action::GridMoveLeft
                            | Action::GridMoveRight
                            | Action::GridMoveUp
                            | Action::GridMoveDown) => {
                                needs_redraw = self.handle_grid_movement_action(a);
                            }
                            a @ (Action::ResetView
                            | Action::FitToWindow
                            | Action::BestFit
                            | Action::FitWidth
                            | Action::FitHeight
                            | Action::ZoomReset
                            | Action::ZoomIn
                            | Action::ZoomOut
                            | Action::PanLeft
                            | Action::PanRight
                            | Action::PanUp
                            | Action::PanDown) => {
                                needs_redraw = self.handle_view_action(a, old_scale);
                            }
                            a @ (Action::RotateCW
                            | Action::RotateCCW
                            | Action::FlipHorizontal
                            | Action::FlipVertical
                            | Action::MarkFile
                            | Action::UnmarkAll
                            | Action::RemoveImage
                            | Action::ToggleMarks) => {
                                needs_redraw = self.handle_image_ops_action(a);
                                if self.images.is_empty() {
                                    _el.exit();
                                }
                            }
                            a @ (Action::ToggleStatusBar
                            | Action::ToggleGrid
                            | Action::ToggleAnimation) => {
                                needs_redraw = self.handle_toggle_action(a);
                            }
                        }
                    }

                    if needs_redraw {
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                }
            }
            _ => (),
        }
    }
}

