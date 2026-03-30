use crate::app::{App, InputMode};
use crate::image_item::ImageSlot;
use crate::keybinds::Action;
use crate::view_mode::ViewMode;
use std::time::{Duration, Instant};
use winit::event_loop::ActiveEventLoop;

impl App {
    pub fn handle_grid_click(&mut self, mouse_x: f64, mouse_y: f64) -> bool {
        if !self.camera.grid_mode || self.gallery.filtered.is_empty() {
            return false;
        }

        let Some(w) = &self.window else {
            return false;
        };
        let size = w.inner_size();
        let buf_w = size.width as u32;
        let mut buf_h = size.height as u32;

        if self.show_status_bar {
            buf_h = buf_h.saturating_sub(self.status_bar.height);
        }
        if mouse_y >= buf_h as f64 {
            return false;
        }

        let config = crate::config::AppConfig::get();
        let cell_size = config.options.thumbnail_size + config.options.grid_padding;
        let cols = (buf_w / cell_size).max(1);
        let grid_width = cols * cell_size;
        let margin_x = (buf_w.saturating_sub(grid_width)) / 2 + config.options.grid_padding / 2;

        let current_row = (self.gallery.current_index as u32) / cols;
        let scroll_y = if current_row * cell_size > buf_h / 2 {
            (current_row * cell_size) as i32 - (buf_h as i32 / 2) + (cell_size as i32 / 2)
        } else {
            0
        };

        let x_rel = mouse_x as i32 - margin_x as i32;
        let y_rel = mouse_y as i32 + scroll_y - (config.options.grid_padding as i32 / 2);

        if x_rel < 0 || x_rel >= (cols * cell_size) as i32 || y_rel < 0 {
            return false;
        }

        let col = x_rel as u32 / cell_size;
        let row = y_rel as u32 / cell_size;
        let clicked_idx = (row * cols + col) as usize;

        if clicked_idx < self.gallery.filtered.len() {
            self.gallery.current_index = clicked_idx;
            self.camera.grid_mode = false;
            self.reset_view_for_new_image();
            return true;
        }

        false
    }

    pub fn reset_view_for_new_image(&mut self) {
        self.camera.off_x = 0;
        self.camera.off_y = 0;
        self.playback.reset();
    }

    // Returns true if a redraw is needed.
    pub fn dispatch_action(&mut self, action: Action, el: &ActiveEventLoop) -> bool {
        let old_scale = self.get_current_scale();

        match action {
            Action::Quit => {
                let pid = std::process::id();
                let pid_socket = crate::ipc::get_pid_socket(pid);
                let _ = std::fs::remove_file(pid_socket);

                el.exit();
                false
            }
            Action::FilterMode => {
                self.input.mode = InputMode::Filtering;
                true
            }
            Action::ScriptHandlerPrefix => {
                self.input.mode = InputMode::WaitingForHandler;
                true
            }
            Action::Digit(d) => {
                if d == 0 && self.input.prefix_count.is_none() {
                    self.handle_navigation_action(Action::FirstImage, 1)
                } else {
                    let current = self.input.prefix_count.unwrap_or(0);
                    self.input
                        .prefix_count
                        .replace(current.saturating_mul(10).saturating_add(d));
                    true
                }
            }
            other_action => {
                let raw_prefix = self.input.prefix_count;
                let count = self.pop_count();

                let redraw = self.handle_navigation_action(other_action, count)
                    || self.handle_grid_movement_action(other_action, count)
                    || self.handle_image_ops_action(other_action, count)
                    || self.handle_view_action(other_action, old_scale)
                    || self.handle_toggle_action(other_action, raw_prefix);

                if matches!(other_action, Action::RemoveImage) && self.gallery.all.is_empty() {
                    el.exit();
                }

                redraw
            }
        }
    }

    pub fn handle_navigation_action(&mut self, action: Action, count: usize) -> bool {
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

    pub fn handle_grid_movement_action(&mut self, action: Action, count: usize) -> bool {
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

    pub fn handle_view_action(&mut self, action: Action, old_scale: f64) -> bool {
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

    pub fn handle_image_ops_action(&mut self, action: Action, count: usize) -> bool {
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

    pub fn handle_toggle_action(&mut self, action: Action, prefix: Option<usize>) -> bool {
        let mut needs_redraw = false;
        match action {
            Action::ToggleSlideshow => {
                if let Some(n) = prefix {
                    let secs = n.max(1) as u64;
                    self.playback.slideshow_delay = Duration::from_secs(secs);
                    self.playback.slideshow_on = true;
                    self.playback.last_slide_time = Instant::now();
                } else {
                    self.playback.slideshow_on = !self.playback.slideshow_on;
                    self.playback.last_slide_time = Instant::now();
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
}
