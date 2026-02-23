use crate::app::{App, AppEvent, CursorZone, InputMode};
use crate::image_item::ImageSlot;
use crate::keybinds::Action;
use pixels::{Pixels, SurfaceTexture};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowId;

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

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut attributes = winit::window::Window::default_attributes().with_title("rsiv");

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
        self.cursor.next_icon = crate::app::build_cursor(event_loop, true);
        self.cursor.prev_icon = crate::app::build_cursor(event_loop, false);
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::InitialCount(count) => {
                self.gallery.all = vec![ImageSlot::PendingMetadata; count];
                self.gallery.filtered = vec![ImageSlot::PendingMetadata; count];
            }
            AppEvent::MetadataLoaded(idx, item) => {
                if let Some(slot) = self.gallery.all.get_mut(idx) {
                    *slot = ImageSlot::MetadataLoaded(item.clone());
                }

                if self.gallery.filter_text.is_empty() {
                    if let Some(slot) = self.gallery.filtered.get_mut(idx) {
                        *slot = ImageSlot::MetadataLoaded(item);
                    }
                } else {
                    self.gallery.apply_filter();
                }

                if self.gallery.current_index == idx {
                    if let Some(w) = self.window.as_ref() {
                        w.request_redraw();
                    }
                }
            }
            AppEvent::MetadataError(idx, path, err) => {
                crate::rsiv_err!("Metadata error for {:?}: {}", path, err);
                if let Some(slot) = self.gallery.all.get_mut(idx) {
                    *slot = ImageSlot::Error(err.clone());
                }
                if let Some(slot) = self.gallery.filtered.get_mut(idx) {
                    *slot = ImageSlot::Error(err);
                }
            }
            AppEvent::DiscoveryComplete => {
                self.gallery.discovery_complete = true;

                let has_valid_images = self
                    .gallery
                    .all
                    .iter()
                    .any(|slot| matches!(slot, ImageSlot::MetadataLoaded(_)));

                if !has_valid_images {
                    crate::rsiv_err!("No images found. Exiting...");
                    _el.exit();
                }
            }
            AppEvent::ImagePixelsLoaded(path, image) => {
                self.assets.pending.remove(&path);
                self.assets.cache.insert_image(path.clone(), image);
                if let ImageSlot::MetadataLoaded(item) =
                    &self.gallery.filtered[self.gallery.current_index]
                {
                    if item.path == path {
                        self.window.as_ref().unwrap().request_redraw();
                    }
                }
            }
            AppEvent::ThumbnailLoaded(path, thumb) => {
                self.assets.pending.remove(&path);
                self.assets.cache.insert_thumbnail(path.clone(), thumb);
                if self.camera.grid_mode
                    && self.gallery.is_path_visible(
                        &path,
                        self.window.as_ref(),
                        self.camera.grid_mode,
                    )
                {
                    self.window.as_ref().unwrap().request_redraw();
                }
            }
            AppEvent::LoadError(path, err) => {
                crate::rsiv_err!("Failed to load image {:?}: {}", path, err);
                self.assets.pending.remove(&path);
                for slot in &mut self.gallery.all {
                    if let ImageSlot::MetadataLoaded(item) = slot {
                        if item.path == path {
                            *slot = ImageSlot::Error(err.clone());
                        }
                    }
                }
                for slot in &mut self.gallery.filtered {
                    if let ImageSlot::MetadataLoaded(item) = slot {
                        if item.path == path {
                            *slot = ImageSlot::Error(err.clone());
                        }
                    }
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            AppEvent::LoadCancelled(path) => {
                self.assets.pending.remove(&path);
            }
            AppEvent::FileChanged(new_item) => {
                let path = new_item.path.clone();

                let existing_idx = self.gallery.all.iter().position(|slot| {
                    if let ImageSlot::MetadataLoaded(item) = slot {
                        item.path == path
                    } else {
                        false
                    }
                });

                if let Some(idx) = existing_idx {
                    self.assets.cache.remove(&path);
                    self.gallery.all[idx] = ImageSlot::MetadataLoaded(new_item.clone());

                    if !self.gallery.filtered.is_empty()
                        && self.gallery.current_index < self.gallery.filtered.len()
                    {
                        if let ImageSlot::MetadataLoaded(current_item) =
                            &self.gallery.filtered[self.gallery.current_index]
                        {
                            if current_item.path == path {
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                            }
                        }
                    }
                } else {
                    let insert_pos = self.gallery.all.partition_point(|slot| {
                        if let ImageSlot::MetadataLoaded(item) = slot {
                            item.path < path
                        } else {
                            true
                        }
                    });
                    self.gallery
                        .all
                        .insert(insert_pos, ImageSlot::MetadataLoaded(new_item));
                }

                self.gallery.apply_filter();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            AppEvent::FileDeleted(path) => {
                self.assets.cache.remove(&path);

                self.gallery.all.retain(|slot| {
                    if let ImageSlot::MetadataLoaded(item) = slot {
                        item.path != path
                    } else {
                        true
                    }
                });

                let was_current = if !self.gallery.filtered.is_empty()
                    && self.gallery.current_index < self.gallery.filtered.len()
                {
                    if let ImageSlot::MetadataLoaded(item) =
                        &self.gallery.filtered[self.gallery.current_index]
                    {
                        item.path == path
                    } else {
                        false
                    }
                } else {
                    false
                };

                self.gallery.apply_filter();

                if self.gallery.current_index >= self.gallery.filtered.len() {
                    self.gallery.current_index = self.gallery.filtered.len().saturating_sub(1);
                }

                if was_current || self.camera.grid_mode {
                    self.reset_view_for_new_image();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }
            AppEvent::HandlerFinished => {
                self.input.is_handler_running = false;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
        }
    }

    fn window_event(&mut self, _el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => _el.exit(),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.input.modifiers = modifiers.state();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor.pos = Some((position.x, position.y));
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
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor.pos = None;
                self.cursor.set_zone(CursorZone::None, self.window.as_ref());
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state.is_pressed() && button == MouseButton::Left {
                    if let Some((x, _)) = self.cursor.pos {
                        let width = self
                            .window
                            .as_ref()
                            .map(|w| w.inner_size().width as f64)
                            .unwrap_or(0.0);
                        if self.cursor.is_in_next_zone(
                            x,
                            width,
                            self.camera.grid_mode,
                            &self.input.mode,
                        ) {
                            let needs_redraw = self.handle_navigation_action(Action::NextImage, 1);
                            if needs_redraw {
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                            }
                        } else if self.cursor.is_in_prev_zone(
                            x,
                            width,
                            self.camera.grid_mode,
                            &self.input.mode,
                        ) {
                            let needs_redraw = self.handle_navigation_action(Action::PrevImage, 1);
                            if needs_redraw {
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => self.render(),
            WindowEvent::Resized(new_size) => {
                if let Some(pixels) = &mut self.pixels {
                    if new_size.width > 0 && new_size.height > 0 {
                        let _ = pixels.resize_surface(new_size.width, new_size.height);
                        let _ = pixels.resize_buffer(new_size.width, new_size.height);
                    }
                }
                self.clamp_offsets();
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
                    let mut needs_redraw = false;

                    if self.input.is_handler_running {
                        let is_ctrl_c = match &event.logical_key {
                            Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                                self.input.modifiers.control_key()
                            }
                            _ => false,
                        };

                        if is_ctrl_c {
                            self.input
                                .handler_cancel_flag
                                .store(true, std::sync::atomic::Ordering::Relaxed);
                            return;
                        }
                    }

                    if event.logical_key == Key::Named(NamedKey::Escape) {
                        if self.input.prefix_count.is_some() {
                            self.input.prefix_count = None;
                            needs_redraw = true;
                        }

                        match self.input.mode {
                            InputMode::Filtering => {
                                if !self.gallery.filter_text.is_empty() {
                                    self.gallery.filter_text.clear();
                                    self.gallery.apply_filter();
                                }
                                self.input.mode = InputMode::Normal;
                                needs_redraw = true;
                            }
                            InputMode::WaitingForHandler | InputMode::AwaitingTarget(_) => {
                                self.input.mode = InputMode::Normal;
                                needs_redraw = true;
                            }
                            InputMode::Normal => {}
                        }

                        if needs_redraw {
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                        return;
                    }

                    match self.input.mode {
                        InputMode::WaitingForHandler | InputMode::AwaitingTarget(_) => {
                            if let Key::Character(c) = &event.logical_key {
                                self.handle_modal_input(c.as_str());
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                                return;
                            }
                        }
                        InputMode::Filtering => {
                            match event.logical_key {
                                Key::Named(NamedKey::Enter) => {
                                    self.input.mode = InputMode::Normal;
                                    needs_redraw = true;
                                }
                                Key::Named(NamedKey::Backspace) => {
                                    self.gallery.filter_text.pop();
                                    self.gallery.apply_filter();
                                    needs_redraw = true;
                                }
                                Key::Named(NamedKey::Space) => {
                                    self.gallery.filter_text.push(' ');
                                    self.gallery.apply_filter();
                                    needs_redraw = true;
                                }
                                Key::Character(ref c) => {
                                    self.gallery.filter_text.push_str(c);
                                    self.gallery.apply_filter();
                                    needs_redraw = true;
                                }
                                _ => {}
                            }
                            if needs_redraw {
                                if let Some(w) = &self.window {
                                    w.request_redraw();
                                }
                            }
                            return;
                        }
                        InputMode::Normal => {}
                    }

                    let old_scale = self.get_current_scale();
                    if let Some(action) = crate::keybinds::Binding::resolve(
                        &event,
                        &self.input.bindings,
                        self.input.modifiers,
                        self.camera.grid_mode,
                    ) {
                        match action {
                            Action::Quit => _el.exit(),
                            Action::FilterMode => {
                                self.input.mode = InputMode::Filtering;
                                needs_redraw = true;
                            }
                            Action::ScriptHandlerPrefix => {
                                self.input.mode = InputMode::WaitingForHandler;
                                needs_redraw = true;
                            }
                            Action::Digit(d) => {
                                if d == 0 && self.input.prefix_count.is_none() {
                                    self.handle_navigation_action(Action::FirstImage, 1);
                                    needs_redraw = true;
                                } else {
                                    let current = self.input.prefix_count.unwrap_or(0);
                                    let new_count = current.saturating_mul(10).saturating_add(d);
                                    self.input.prefix_count = Some(new_count);
                                    needs_redraw = true;
                                }
                            }
                            other_action => {
                                let raw_prefix = self.input.prefix_count;
                                let count = self.pop_count();

                                if self.handle_navigation_action(other_action, count)
                                    || self.handle_grid_movement_action(other_action, count)
                                    || self.handle_image_ops_action(other_action, count)
                                    || self.handle_view_action(other_action, old_scale)
                                    || self.handle_toggle_action(other_action, raw_prefix)
                                {
                                    needs_redraw = true;
                                }
                                if matches!(other_action, Action::RemoveImage)
                                    && self.gallery.all.is_empty()
                                {
                                    _el.exit();
                                }
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
