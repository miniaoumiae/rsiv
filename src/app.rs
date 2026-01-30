use crate::image_item::ImageItem;
use crate::status_bar::StatusBar;
use crate::view_mode::ViewMode;
use pixels::{Pixels, SurfaceTexture};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, ModifiersState, NamedKey};
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
    ImageLoaded(ImageItem),
    LoadComplete,
}

pub struct App {
    pub images: Vec<ImageItem>,
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

    // UI
    pub status_bar: StatusBar,
    pub show_status_bar: bool,
    pub load_complete: bool,
}

impl App {
    pub fn new(images: Vec<ImageItem>) -> Self {
        Self {
            images,
            current_index: 0,
            mode: ViewMode::Absolute,
            off_x: 0,
            off_y: 0,
            window: None,
            pixels: None,
            current_frame_index: 0,
            is_playing: true,
            last_update: Instant::now(),
            frame_timer: Duration::ZERO,
            modifiers: ModifiersState::default(),
            status_bar: StatusBar::new(),
            show_status_bar: true,
            load_complete: false,
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
        let item = &self.images[self.current_index];
        let (buf_w, buf_h) = if let Some((w, h)) = self.get_available_window_size() {
            (w, h)
        } else {
            return 1.0;
        };

        // Safety check to avoid division by zero
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

    fn render(&mut self) {
        if self.images.is_empty() {
            return;
        }

        // Animation
        let now = Instant::now();
        let dt = now.duration_since(self.last_update);
        self.last_update = now;

        let item = &self.images[self.current_index];
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

        // Render the Image
        crate::renderer::Renderer::draw_image(
            frame_slice,
            buf_w,
            available_h,
            item,
            self.current_frame_index,
            scale,
            self.off_x,
            self.off_y,
        );

        // Draw Status Bar
        if self.show_status_bar && buf_h > 0 {
            let mut fb =
                crate::frame_buffer::FrameBuffer::new(frame_slice, buf_w as u32, buf_h as u32);
            self.status_bar.draw(
                &mut fb,
                (scale * 100.0) as u32,
                self.current_index + 1,
                self.images.len(),
                &item.path,
            );
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

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::ImageLoaded(item) => {
                self.images.push(item);

                // If this is the first image, render it immediately
                if self.images.len() == 1 {
                    self.current_index = 0;
                    self.reset_view_for_new_image();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
            AppEvent::LoadComplete => {
                self.load_complete = true;
                if self.images.is_empty() {
                    // No images loaded, exit
                    event_loop.exit();
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
                    let step = 50;
                    let old_scale = self.get_current_scale();
                    let mut changed_scale = false;
                    let mut needs_redraw = false;

                    // Handle Ctrl-a
                    if self.modifiers.control_key() {
                        if let Key::Character(c) = &event.logical_key {
                            if c.as_str() == "a" {
                                self.is_playing = !self.is_playing;
                                needs_redraw = true;
                            }
                        }
                    }

                    if !needs_redraw {
                        match event.logical_key {
                            Key::Character(c) => match c.as_str() {
                                "q" => _el.exit(),
                                "z" => {
                                    self.off_x = 0;
                                    self.off_y = 0;
                                    needs_redraw = true;
                                }
                                "f" => {
                                    self.mode = ViewMode::FitToWindow;
                                    needs_redraw = true;
                                }
                                "F" => {
                                    self.mode = ViewMode::BestFit;
                                    needs_redraw = true;
                                }
                                "W" => {
                                    self.mode = ViewMode::FitWidth;
                                    needs_redraw = true;
                                }
                                "H" => {
                                    self.mode = ViewMode::FitHeight;
                                    needs_redraw = true;
                                }
                                "h" => {
                                    self.off_x += step;
                                    needs_redraw = true;
                                }
                                "l" => {
                                    self.off_x -= step;
                                    needs_redraw = true;
                                }
                                "k" => {
                                    self.off_y += step;
                                    needs_redraw = true;
                                }
                                "j" => {
                                    self.off_y -= step;
                                    needs_redraw = true;
                                }
                                "b" => {
                                    self.show_status_bar = !self.show_status_bar;
                                    needs_redraw = true;
                                }
                                "=" => {
                                    self.mode = ViewMode::Absolute;
                                    needs_redraw = true;
                                }
                                "+" => {
                                    self.mode = ViewMode::Zoom(old_scale * 1.1);
                                    changed_scale = true;
                                }
                                "-" => {
                                    self.mode = ViewMode::Zoom(old_scale / 1.1);
                                    changed_scale = true;
                                }
                                "n" => {
                                    if !self.images.is_empty() {
                                        self.current_index =
                                            (self.current_index + 1) % self.images.len();
                                        self.reset_view_for_new_image();
                                        needs_redraw = true;
                                    }
                                }
                                "p" => {
                                    if !self.images.is_empty() {
                                        self.current_index =
                                            (self.current_index + self.images.len() - 1)
                                                % self.images.len();
                                        self.reset_view_for_new_image();
                                        needs_redraw = true;
                                    }
                                }
                                ">" => {
                                    if !self.images.is_empty() {
                                        self.images[self.current_index].rotate(true);
                                        self.reset_view_for_new_image();
                                        needs_redraw = true;
                                    }
                                }
                                "<" => {
                                    if !self.images.is_empty() {
                                        self.images[self.current_index].rotate(false);
                                        self.reset_view_for_new_image();
                                        needs_redraw = true;
                                    }
                                }
                                "_" => {
                                    if !self.images.is_empty() {
                                        self.images[self.current_index].flip_horizontal();
                                        needs_redraw = true;
                                    }
                                }
                                "?" => {
                                    if !self.images.is_empty() {
                                        self.images[self.current_index].flip_vertical();
                                        needs_redraw = true;
                                    }
                                }
                                _ => return,
                            },
                            Key::Named(k) => match k {
                                NamedKey::ArrowLeft => {
                                    self.off_x += step;
                                    needs_redraw = true;
                                }
                                NamedKey::ArrowRight => {
                                    self.off_x -= step;
                                    needs_redraw = true;
                                }
                                NamedKey::ArrowUp => {
                                    self.off_y += step;
                                    needs_redraw = true;
                                }
                                NamedKey::ArrowDown => {
                                    self.off_y -= step;
                                    needs_redraw = true;
                                }
                                _ => return,
                            },
                            _ => return,
                        }
                    }

                    if changed_scale {
                        let new_scale = self.get_current_scale();
                        self.off_x = (self.off_x as f64 * (new_scale / old_scale)) as i32;
                        self.off_y = (self.off_y as f64 * (new_scale / old_scale)) as i32;
                        needs_redraw = true;
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
                                let item = &self.images[self.current_index];
                                let scale = self.get_current_scale();
                                let img_w = (item.width as f64 * scale) as i32;
                                let img_h = (item.height as f64 * scale) as i32;

                                let limit_x = (buf_w / 2) + (img_w / 2) - 10;
                                let limit_y = (buf_h / 2) + (img_h / 2) - 10;

                                self.off_x = self.off_x.max(-limit_x).min(limit_x);
                                self.off_y = self.off_y.max(-limit_y).min(limit_y);
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
