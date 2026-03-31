use crate::app::App;
use crate::image_item::ImageSlot;
use crate::keybinds::Action;
use crate::status_bar::StatusContext;
use std::time::{Duration, Instant};

impl App {
    pub fn render(&mut self) {
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
        crate::renderer::clear(frame_slice, bg_color.into());

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
                    bg: bg_color.into(),
                    accent: crate::utils::parse_color(&config.ui.thumbnail_border_color).into(),
                    mark: crate::utils::parse_color(&config.ui.mark_color).into(),
                    loading: crate::utils::parse_color(&config.ui.loading_color).into(),
                    error: crate::utils::parse_color(&config.ui.error_color).into(),
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

            let (current_frame, total_frames) = self
                .gallery
                .filtered
                .get(self.gallery.current_index)
                .and_then(|slot| match slot {
                    ImageSlot::MetadataLoaded(item) => self.assets.cache.get_image(&item.path),
                    _ => None,
                })
                .map(|img| (self.playback.current_frame_index + 1, img.frame_count()))
                .unwrap_or((0, 0));

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
