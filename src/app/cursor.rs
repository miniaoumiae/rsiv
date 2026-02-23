use crate::app::InputMode;
use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;
use winit::window::{CursorIcon, CustomCursor, Window};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum CursorZone {
    None,
    Prev,
    Next,
}

pub struct CursorState {
    pub pos: Option<(f64, f64)>,
    pub zone: CursorZone,
    pub next_icon: Option<CustomCursor>,
    pub prev_icon: Option<CustomCursor>,
}

impl CursorState {
    pub fn is_in_next_zone(
        &self,
        x: f64,
        window_width: f64,
        grid_mode: bool,
        input_mode: &InputMode,
    ) -> bool {
        if grid_mode || *input_mode != InputMode::Normal || window_width <= 0.0 {
            return false;
        }
        x >= window_width * 0.8
    }

    pub fn is_in_prev_zone(
        &self,
        x: f64,
        window_width: f64,
        grid_mode: bool,
        input_mode: &InputMode,
    ) -> bool {
        if grid_mode || *input_mode != InputMode::Normal || window_width <= 0.0 {
            return false;
        }
        x <= window_width * 0.2
    }

    pub fn set_zone(&mut self, zone: CursorZone, window: Option<&Arc<Window>>) {
        if self.zone == zone {
            return;
        }
        self.zone = zone;
        if let Some(w) = window {
            match zone {
                CursorZone::Next => {
                    if let Some(cursor) = &self.next_icon {
                        w.set_cursor(cursor.clone());
                    } else {
                        w.set_cursor(CursorIcon::Default);
                    }
                }
                CursorZone::Prev => {
                    if let Some(cursor) = &self.prev_icon {
                        w.set_cursor(cursor.clone());
                    } else {
                        w.set_cursor(CursorIcon::Default);
                    }
                }
                CursorZone::None => w.set_cursor(CursorIcon::Default),
            };
        }
    }

    pub fn refresh_icon(
        &mut self,
        window_width: f64,
        grid_mode: bool,
        input_mode: &InputMode,
        window: Option<&Arc<Window>>,
    ) {
        if let Some((x, _)) = self.pos {
            if self.is_in_next_zone(x, window_width, grid_mode, input_mode) {
                self.set_zone(CursorZone::Next, window);
            } else if self.is_in_prev_zone(x, window_width, grid_mode, input_mode) {
                self.set_zone(CursorZone::Prev, window);
            } else {
                self.set_zone(CursorZone::None, window);
            }
        } else {
            self.set_zone(CursorZone::None, window);
        }
    }

    pub fn build_cursor(event_loop: &ActiveEventLoop, is_next: bool) -> Option<CustomCursor> {
        let config = crate::config::AppConfig::get();
        let size = config.ui.cursor_size.max(16);
        let inner_color = crate::utils::parse_color_rgba(&config.ui.cursor_inner_color);
        let border_color = crate::utils::parse_color_rgba(&config.ui.cursor_border_color);
        let use_shadow = config.ui.cursor_shadow;
        let border_thickness = config.ui.cursor_border_width as i32;

        let w = size as i32;
        let h = size as i32;

        let tip_x = if is_next { w * 7 / 8 } else { w * 1 / 8 };
        let mid_y = h / 2;
        let head_len = w * 3 / 8;
        let head_base_x = if is_next { tip_x - head_len } else { tip_x + head_len };
        let head_half_height = h * 5 / 16;
        let tail_half_height = h * 2 / 16;
        let tail_end_x = if is_next { w * 2 / 8 } else { w * 6 / 8 };

        let hot_x = tip_x as u16;
        let hot_y = mid_y as u16;

        let mut fill = vec![false; (w * h) as usize];

        for y in 0..h {
            let dy = (y - mid_y).abs();
            if dy <= head_half_height {
                let dx = dy * head_len / head_half_height;
                let (min_x, max_x) = if is_next {
                    (head_base_x, tip_x - dx)
                } else {
                    (tip_x + dx, head_base_x)
                };
                for x in min_x..=max_x {
                    if x >= 0 && x < w {
                        fill[(y * w + x) as usize] = true;
                    }
                }
            }
            if dy <= tail_half_height {
                let (min_x, max_x) = if is_next {
                    (tail_end_x, head_base_x)
                } else {
                    (head_base_x, tail_end_x)
                };
                for x in min_x..=max_x {
                    if x >= 0 && x < w {
                        fill[(y * w + x) as usize] = true;
                    }
                }
            }
        }

        let mut border = vec![false; (w * h) as usize];
        if border_thickness > 0 {
            for y in 0..h {
                for x in 0..w {
                    if fill[(y * w + x) as usize] {
                        continue;
                    }
                    let mut is_border = false;
                    for ny in (y - border_thickness)..=(y + border_thickness) {
                        for nx in (x - border_thickness)..=(x + border_thickness) {
                            if nx >= 0 && ny >= 0 && nx < w && ny < h {
                                if fill[(ny * w + nx) as usize] {
                                    is_border = true;
                                    break;
                                }
                            }
                        }
                        if is_border {
                            break;
                        }
                    }
                    border[(y * w + x) as usize] = is_border;
                }
            }
        }

        let mut shadow = vec![0.0f32; (w * h) as usize];
        if use_shadow {
            let shadow_radius = (size / 12).max(2) as i32;
            let shadow_offset_y = (size / 24).max(1) as i32;
            for y in 0..h {
                for x in 0..w {
                    if fill[(y * w + x) as usize] || border[(y * w + x) as usize] {
                        for sy in (y - shadow_radius)..=(y + shadow_radius) {
                            for sx in (x - shadow_radius)..=(x + shadow_radius) {
                                let target_y = sy + shadow_offset_y;
                                let target_x = sx;
                                if target_x >= 0 && target_y >= 0 && target_x < w && target_y < h {
                                    let dist = (((x - sx).pow(2) + (y - sy).pow(2)) as f32).sqrt();
                                    if dist <= shadow_radius as f32 {
                                        let intensity = 1.0 - (dist / shadow_radius as f32);
                                        let idx = (target_y * w + target_x) as usize;
                                        shadow[idx] = shadow[idx].max(intensity * 0.6);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                if border[idx] {
                    rgba.extend_from_slice(&[
                        border_color.0,
                        border_color.1,
                        border_color.2,
                        border_color.3,
                    ]);
                } else if fill[idx] {
                    rgba.extend_from_slice(&[
                        inner_color.0,
                        inner_color.1,
                        inner_color.2,
                        inner_color.3,
                    ]);
                } else if use_shadow && shadow[idx] > 0.0 {
                    let alpha = (shadow[idx] * 255.0) as u8;
                    rgba.extend_from_slice(&[0, 0, 0, alpha]);
                } else {
                    rgba.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
        }

        let source = CustomCursor::from_rgba(rgba, size, size, hot_x, hot_y).ok()?;
        Some(event_loop.create_custom_cursor(source))
    }
}
