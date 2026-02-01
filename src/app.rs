use crate::image_item::{ImageItem, ImageSlot};
use crate::keybinds::Action;
use crate::status_bar::StatusBar;
use crate::view_mode::ViewMode;
use pixels::{Pixels, SurfaceTexture};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
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
    ImageLoaded(usize, ImageItem),
    ImageLoadFailed(usize, String),
    LoadComplete,
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
    pub load_complete: bool,
    pub grid_mode: bool,
    pub marked_files: HashSet<String>,
    pub bindings: Vec<crate::keybinds::Binding>,
}

impl App {
    pub fn new(images: Vec<ImageSlot>, start_in_grid_mode: bool) -> Self {
        let config = crate::config::AppConfig::get();

        Self {
            images,
            current_index: 0,
            mode: config.options.default_view,
            off_x: 0,
            off_y: 0,
            window: None,
            pixels: None,
            current_frame_index: 0,
            is_playing: true,
            last_update: Instant::now(),
            frame_timer: Duration::ZERO,
            input_mode: InputMode::Normal,
            modifiers: ModifiersState::default(),
            status_bar: StatusBar::new(),
            show_status_bar: true,
            load_complete: false,
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
        let ImageSlot::Loaded(item) = &self.images[self.current_index] else {
            return 1.0;
        };

        let (buf_w, buf_h) = if let Some((w, h)) = self.get_available_window_size() {
            (w, h)
        } else {
            return 1.0;
        };

        // Avoid division by zero
        if buf_w <= 0.0 || buf_h <= 0.0 {
            return 1.0;
        }

        match self.mode {
            ViewMode::Absolute => 1.0,
            ViewMode::Zoom(s) => s,
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

        match action {
            Action::ResetView => {
                self.off_x = 0;
                self.off_y = 0;
                needs_redraw = true;
            }
            Action::FitToWindow => {
                self.mode = ViewMode::FitToWindow;
                needs_redraw = true;
            }
            Action::BestFit => {
                self.mode = ViewMode::BestFit;
                needs_redraw = true;
            }
            Action::FitWidth => {
                self.mode = ViewMode::FitWidth;
                needs_redraw = true;
            }
            Action::FitHeight => {
                self.mode = ViewMode::FitHeight;
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
                needs_redraw = true;
            }
            Action::ZoomIn => {
                self.mode = ViewMode::Zoom(old_scale * 1.1);
                changed_scale = true;
            }
            Action::ZoomOut => {
                self.mode = ViewMode::Zoom(old_scale / 1.1);
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
                    if let ImageSlot::Loaded(item) = &self.images[self.current_index] {
                        let path = item.path.clone();
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
                    if let ImageSlot::Loaded(item) = item_slot {
                        if !self.marked_files.remove(&item.path) {
                            self.marked_files.insert(item.path.clone());
                        }
                    }
                }
                needs_redraw = true;
            }
            Action::RotateCW => {
                if !self.images.is_empty() {
                    if let ImageSlot::Loaded(item) = &mut self.images[self.current_index] {
                        item.rotate(true);
                        self.reset_view_for_new_image();
                        needs_redraw = true;
                    }
                }
            }
            Action::RotateCCW => {
                if !self.images.is_empty() {
                    if let ImageSlot::Loaded(item) = &mut self.images[self.current_index] {
                        item.rotate(false);
                        self.reset_view_for_new_image();
                        needs_redraw = true;
                    }
                }
            }
            Action::FlipHorizontal => {
                if !self.images.is_empty() {
                    if let ImageSlot::Loaded(item) = &mut self.images[self.current_index] {
                        item.flip_horizontal();
                        needs_redraw = true;
                    }
                }
            }
            Action::FlipVertical => {
                if !self.images.is_empty() {
                    if let ImageSlot::Loaded(item) = &mut self.images[self.current_index] {
                        item.flip_vertical();
                        needs_redraw = true;
                    }
                }
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

        // Animation
        if !self.grid_mode {
            if let ImageSlot::Loaded(item) = &self.images[self.current_index] {
                let now = Instant::now();
                let dt = now.duration_since(self.last_update);
                self.last_update = now;

                let frame_count = item.frames.len();

                if self.is_playing && frame_count > 1 {
                    self.frame_timer += dt;
                    let current_delay = item.frames[self.current_frame_index].delay;
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

        // Prepare Drawing Surface
        let scale = self.get_current_scale();
        let Some(pixels) = &mut self.pixels else {
            return;
        };

        let frame_slice = pixels.frame_mut();
        let config = crate::config::AppConfig::get();
        let bg_color = crate::utils::parse_color(&config.ui.bg_color);

        // Clear Background
        crate::renderer::Renderer::clear(frame_slice, bg_color);

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

        // Render Content
        if self.grid_mode {
            let colors = crate::renderer::GridColors {
                bg: bg_color,
                accent: crate::utils::parse_color(&config.ui.thumbnail_border_color),
                mark: crate::utils::parse_color(&config.ui.mark_color),
                loading: crate::utils::parse_color(&config.ui.loading_color),
                error: crate::utils::parse_color(&config.ui.error_color),
            };

            crate::renderer::Renderer::draw_grid(
                frame_slice,
                buf_w,
                available_h,
                &mut self.images,
                self.current_index,
                &colors,
                &self.marked_files,
            );
        } else {
            // Render the Image
            if let ImageSlot::Loaded(item) = &self.images[self.current_index] {
                let params = crate::renderer::DrawImageParams {
                    item,
                    frame_idx: self.current_frame_index,
                    scale,
                    off_x: self.off_x,
                    off_y: self.off_y,
                };
                crate::renderer::Renderer::draw_image(frame_slice, buf_w, available_h, &params);
            }
        }

        // Draw Status Bar
        if self.show_status_bar && buf_h > 0 {
            let mut fb =
                crate::frame_buffer::FrameBuffer::new(frame_slice, buf_w as u32, buf_h as u32);

            match &self.images[self.current_index] {
                ImageSlot::Loaded(item) => {
                    let is_marked = self.marked_files.contains(&item.path);

                    self.status_bar.draw(
                        &mut fb,
                        if self.grid_mode {
                            100
                        } else {
                            (scale * 100.0) as u32
                        },
                        self.current_index + 1,
                        self.images.len(),
                        &item.path,
                        is_marked,
                        &self.input_mode,
                    );
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
                ImageSlot::Loading => {
                    let message = if self.load_complete {
                        "Error Loading Image"
                    } else {
                        "Loading..."
                    };
                    self.status_bar.draw(
                        &mut fb,
                        0,
                        self.current_index + 1,
                        self.images.len(),
                        message,
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
                self.images = vec![ImageSlot::Loading; count];
            }
            AppEvent::ImageLoaded(idx, item) => {
                if let Some(slot) = self.images.get_mut(idx) {
                    *slot = ImageSlot::Loaded(item);
                }
                if self.current_index == idx {
                    self.window.as_ref().unwrap().request_redraw();
                }
            }
            AppEvent::ImageLoadFailed(idx, err) => {
                if let Some(slot) = self.images.get_mut(idx) {
                    *slot = ImageSlot::Error(err);
                }
                if self.current_index == idx {
                    self.window.as_ref().unwrap().request_redraw();
                }
            }
            AppEvent::LoadComplete => {
                self.load_complete = true;
                if self.images.is_empty() {
                    eprintln!("No images found. Exiting...");
                    _el.exit();
                }
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
                            | Action::LastImage) => {
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
                            // Clamping logic
                            let size = w.inner_size();
                            let buf_w = size.width as i32;
                            // Available height for clamping logic should also consider status bar
                            let buf_h = if self.show_status_bar {
                                size.height as i32 - self.status_bar.height as i32
                            } else {
                                size.height as i32
                            };

                            if !self.images.is_empty() {
                                if let ImageSlot::Loaded(item) = &self.images[self.current_index] {
                                    let scale = self.get_current_scale();
                                    let img_w = (item.width as f64 * scale) as i32;
                                    let img_h = (item.height as f64 * scale) as i32;

                                    let limit_x = (buf_w / 2) + (img_w / 2) - 10;
                                    let limit_y = (buf_h / 2) + (img_h / 2) - 10;

                                    self.off_x = self.off_x.max(-limit_x).min(limit_x);
                                    self.off_y = self.off_y.max(-limit_y).min(limit_y);
                                }
                            }

                            w.request_redraw();
                        }
                    }
                }
            }
            _ => (),
        }
    }
}
