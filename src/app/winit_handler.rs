use crate::app::{App, AppEvent, CursorZone, InputMode};
use crate::image_item::ImageSlot;
use pixels::{Pixels, SurfaceTexture};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{MouseButton, MouseScrollDelta, WindowEvent};
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
                let err_str = err.to_string();
                crate::rsiv_err!("Metadata error for {:?}: {}", path, err_str);
                if let Some(slot) = self.gallery.all.get_mut(idx) {
                    *slot = ImageSlot::Error(err_str.clone());
                }
                if let Some(slot) = self.gallery.filtered.get_mut(idx) {
                    *slot = ImageSlot::Error(err_str);
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
                let err_str = err.to_string();
                crate::rsiv_err!("Failed to load image {:?}: {}", path, err_str);
                self.assets.pending.remove(&path);
                for slot in &mut self.gallery.all {
                    if let ImageSlot::MetadataLoaded(item) = slot {
                        if item.path == path {
                            *slot = ImageSlot::Error(err_str.clone());
                        }
                    }
                }
                for slot in &mut self.gallery.filtered {
                    if let ImageSlot::MetadataLoaded(item) = slot {
                        if item.path == path {
                            *slot = ImageSlot::Error(err_str.clone());
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
            AppEvent::Ipc(req, tx) => {
                match req {
                    crate::ipc::IpcRequest::RunAction(action) => {
                        let needs_redraw = self.dispatch_action(action, _el);
                        if needs_redraw {
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                        let _ = tx.send(crate::ipc::IpcResponse::Ack);
                    }
                    crate::ipc::IpcRequest::AddFile(path) => {
                        let already_exists = self.gallery.all.iter().any(|slot| {
                            if let ImageSlot::MetadataLoaded(item) = slot {
                                item.path == path
                            } else {
                                false
                            }
                        });

                        if !already_exists {
                            // Add placeholder
                            self.gallery.all.push(ImageSlot::PendingMetadata);
                            self.gallery.apply_filter();

                            if let Some(watcher) = &self.watcher {
                                watcher.watch(&path);
                            }

                            // Jump to the newly added image
                            self.gallery.current_index =
                                self.gallery.filtered.len().saturating_sub(1);
                            self.reset_view_for_new_image();

                            let p = self.proxy.clone();
                            let idx = self.gallery.all.len() - 1;

                            // Probe file in background to prevent freezing the UI
                            std::thread::spawn(move || {
                                if let Ok(format) = crate::loader::identify_format(&path) {
                                    if let Ok((width, height)) =
                                        crate::loader::probe_image(&path, format)
                                    {
                                        let item = crate::image_item::ImageItem {
                                            path: path.clone(),
                                            width,
                                            height,
                                            format,
                                        };
                                        let _ = p.send_event(AppEvent::MetadataLoaded(idx, item));
                                        return;
                                    }
                                }
                                let _ = p.send_event(AppEvent::MetadataError(
                                    idx,
                                    path.clone(),
                                    crate::loader::LoaderError::Other("Invalid image".into()),
                                ));
                            });

                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                        let _ = tx.send(crate::ipc::IpcResponse::Ack);
                    }
                    crate::ipc::IpcRequest::GetState => {
                        let state = if self.gallery.filtered.is_empty() {
                            "No images".to_string()
                        } else if let ImageSlot::MetadataLoaded(item) =
                            &self.gallery.filtered[self.gallery.current_index]
                        {
                            item.path.to_string_lossy().into_owned()
                        } else {
                            "Loading...".to_string()
                        };
                        let _ = tx.send(crate::ipc::IpcResponse::State(state));
                    }
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
                let current_pos = (position.x, position.y);
                self.cursor.pos = Some(current_pos);
                self.cursor.last_moved = std::time::Instant::now();
                if !self.cursor.is_visible {
                    self.cursor.is_visible = true;
                    if let Some(w) = &self.window {
                        w.set_cursor_visible(true);
                    }
                }

                if self.cursor.is_dragging {
                    if let (Some((start_x, start_y)), Some((cam_x, cam_y))) =
                        (self.cursor.drag_start_pos, self.cursor.camera_start_pos)
                    {
                        let dx = current_pos.0 - start_x;
                        let dy = current_pos.1 - start_y;

                        self.camera.off_x = cam_x + dx as i32;
                        self.camera.off_y = cam_y + dy as i32;
                        self.clamp_offsets();

                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
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
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor.pos = None;
                self.cursor.set_zone(CursorZone::None, self.window.as_ref());
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let config = crate::config::AppConfig::get();
                if button == MouseButton::Left {
                    if state.is_pressed() {
                        if let Some((x, y)) = self.cursor.pos {
                            let now = std::time::Instant::now();
                            let mut is_double_click = false;

                            if let (Some(last_time), Some((last_x, last_y))) = (
                                self.cursor.last_left_click_time,
                                self.cursor.last_left_click_pos,
                            ) {
                                if now.duration_since(last_time)
                                    < std::time::Duration::from_millis(300)
                                {
                                    let dx = (x - last_x).abs();
                                    let dy = (y - last_y).abs();
                                    if dx < 5.0 && dy < 5.0 {
                                        is_double_click = true;
                                    }
                                }
                            }

                            self.cursor.last_left_click_time = Some(now);
                            self.cursor.last_left_click_pos = Some((x, y));

                            if is_double_click && !self.camera.grid_mode {
                                let needs_redraw =
                                    self.dispatch_action(config.mousebindings.double_click, _el);
                                if needs_redraw {
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                }
                                self.cursor.is_dragging = false;
                            } else if self.camera.grid_mode {
                                if self.handle_grid_click(x, y) {
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                }
                            } else {
                                self.cursor.is_dragging = true;
                                self.cursor.drag_start_pos = Some((x, y));
                                self.cursor.camera_start_pos =
                                    Some((self.camera.off_x, self.camera.off_y));
                            }
                        }
                    } else if self.cursor.is_dragging {
                        if let (Some((start_x, start_y)), Some((end_x, end_y))) =
                            (self.cursor.drag_start_pos, self.cursor.pos)
                        {
                            let dx = (end_x - start_x).abs();
                            let dy = (end_y - start_y).abs();

                            if dx < 5.0 && dy < 5.0 {
                                let width = self
                                    .window
                                    .as_ref()
                                    .map(|w| w.inner_size().width as f64)
                                    .unwrap_or(0.0);
                                let mut needs_redraw = false;

                                if self.cursor.is_in_next_zone(
                                    end_x,
                                    width,
                                    self.camera.grid_mode,
                                    &self.input.mode,
                                ) {
                                    needs_redraw = self.dispatch_action(
                                        config.mousebindings.left_click_right_zone,
                                        _el,
                                    );
                                } else if self.cursor.is_in_prev_zone(
                                    end_x,
                                    width,
                                    self.camera.grid_mode,
                                    &self.input.mode,
                                ) {
                                    needs_redraw = self.dispatch_action(
                                        config.mousebindings.left_click_left_zone,
                                        _el,
                                    );
                                }

                                if needs_redraw {
                                    if let Some(w) = &self.window {
                                        w.request_redraw();
                                    }
                                }
                            }
                        }
                        self.cursor.is_dragging = false;
                        self.cursor.drag_start_pos = None;
                        self.cursor.camera_start_pos = None;
                    }
                } else if button == MouseButton::Right {
                    if state.is_pressed() {
                        let needs_redraw =
                            self.dispatch_action(config.mousebindings.right_click, _el);
                        if needs_redraw {
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                    }
                } else if button == MouseButton::Middle {
                    if state.is_pressed() {
                        let needs_redraw =
                            self.dispatch_action(config.mousebindings.middle_click, _el);
                        if needs_redraw {
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                    }
                } else if button == MouseButton::Back {
                    if state.is_pressed() {
                        let needs_redraw =
                            self.dispatch_action(config.mousebindings.back_button, _el);
                        if needs_redraw {
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                    }
                } else if button == MouseButton::Forward {
                    if state.is_pressed() {
                        let needs_redraw =
                            self.dispatch_action(config.mousebindings.forward_button, _el);
                        if needs_redraw {
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(pos) => pos.y,
                };

                if scroll_y == 0.0 {
                    return;
                }

                let config = crate::config::AppConfig::get();
                let action = if self.camera.grid_mode {
                    if scroll_y > 0.0 {
                        config.mousebindings.grid_scroll_up
                    } else {
                        config.mousebindings.grid_scroll_down
                    }
                } else if scroll_y > 0.0 {
                    config.mousebindings.scroll_up
                } else {
                    config.mousebindings.scroll_down
                };

                let count = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y.abs().ceil() as usize,
                    MouseScrollDelta::PixelDelta(pos) => (pos.y.abs() / 40.0).ceil() as usize,
                }
                .max(1);

                self.input.prefix_count = Some(count);

                let old_scale = self.get_current_scale();
                let old_off_x = self.camera.off_x;
                let old_off_y = self.camera.off_y;
                let needs_redraw = self.dispatch_action(action, _el);

                if needs_redraw {
                    let new_scale = self.get_current_scale();
                    if (new_scale - old_scale).abs() > f64::EPSILON {
                        if let (Some((buf_w, buf_h)), Some((mx, my))) =
                            (self.get_available_window_size(), self.cursor.pos)
                        {
                            let clamped_x = mx.clamp(0.0, buf_w);
                            let clamped_y = my.clamp(0.0, buf_h);
                            let dx = clamped_x - (buf_w / 2.0);
                            let dy = clamped_y - (buf_h / 2.0);
                            let ratio = new_scale / old_scale;
                            self.camera.off_x =
                                (ratio * old_off_x as f64 + (1.0 - ratio) * dx) as i32;
                            self.camera.off_y =
                                (ratio * old_off_y as f64 + (1.0 - ratio) * dy) as i32;
                            self.clamp_offsets();
                        }
                    }
                }

                if needs_redraw {
                    if let Some(w) = &self.window {
                        w.request_redraw();
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

                    let mut is_modal_input = false;

                    match self.input.mode {
                        InputMode::WaitingForHandler | InputMode::AwaitingTarget(_) => {
                            is_modal_input = true;
                            if let Key::Character(c) = &event.logical_key {
                                self.handle_modal_input(c.as_str());
                                needs_redraw = true;
                            }
                        }
                        InputMode::Filtering => {
                            is_modal_input = true;
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
                        }
                        InputMode::Normal => {}
                    }

                    if is_modal_input {
                        if needs_redraw {
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        }
                        return;
                    }

                    let action_opt = crate::keybinds::Binding::resolve(
                        &event,
                        &self.input.bindings,
                        self.input.modifiers,
                        self.camera.grid_mode,
                    );

                    if let Some(action) = action_opt {
                        needs_redraw |= self.dispatch_action(action, _el);
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let mut next_wakeup = None;
        let now = std::time::Instant::now();

        if self.cursor.is_visible {
            let hide_time = self.cursor.last_moved + std::time::Duration::from_secs(2);
            if now >= hide_time {
                if let Some(w) = &self.window {
                    w.set_cursor_visible(false);
                }
                self.cursor.is_visible = false;
            } else {
                next_wakeup = Some(hide_time);
            }
        }

        if self.playback.slideshow_on {
            let slide_time = self.playback.last_slide_time + self.playback.slideshow_delay;
            if now >= slide_time {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            } else {
                next_wakeup = match next_wakeup {
                    Some(t) => Some(t.min(slide_time)),
                    None => Some(slide_time),
                };
            }
        }

        if !self.camera.grid_mode && self.playback.is_playing && !self.gallery.filtered.is_empty() {
            if let ImageSlot::MetadataLoaded(item) =
                &self.gallery.filtered[self.gallery.current_index]
            {
                if let Some(img) = self.assets.cache.get_image(&item.path) {
                    if img.frame_count() > 1 {
                        let current_delay = img.frame_delay(self.playback.current_frame_index);
                        let effective_delay = if current_delay.is_zero() {
                            std::time::Duration::from_millis(100)
                        } else {
                            current_delay
                        };

                        let frame_time = self.playback.last_update
                            + effective_delay.saturating_sub(self.playback.frame_timer);

                        if now >= frame_time {
                            if let Some(w) = &self.window {
                                w.request_redraw();
                            }
                        } else {
                            next_wakeup = match next_wakeup {
                                Some(t) => Some(t.min(frame_time)),
                                None => Some(frame_time),
                            };
                        }
                    }
                }
            }
        }

        if let Some(wakeup) = next_wakeup {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(wakeup));
        } else {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        }
    }
}
