use crate::app::InputMode;
use crate::config::AppConfig;
use crate::frame_buffer::FrameBuffer;
use crate::utils;
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
use std::sync::{Mutex, OnceLock};

static UI_FONT_SYSTEM: OnceLock<Mutex<FontSystem>> = OnceLock::new();
static UI_SWASH_CACHE: OnceLock<Mutex<SwashCache>> = OnceLock::new();

pub struct StatusBar {
    pub height: u32,
    base_font_size: f32,
    scale_factor: f32,
    background_color: (u8, u8, u8),
    left_buffer: Buffer,
    right_buffer: Buffer,
}

impl StatusBar {
    pub fn new() -> Self {
        let config = AppConfig::get();

        let mut font_system = UI_FONT_SYSTEM
            .get_or_init(|| Mutex::new(FontSystem::new()))
            .lock()
            .unwrap();

        let base_font_size = config.ui.font_size as f32;
        let base_line_height = base_font_size * 1.2;
        let scale_factor = 1.0;

        let metrics = Metrics::new(
            base_font_size * scale_factor,
            base_line_height * scale_factor,
        );
        let height = (base_line_height * scale_factor) as u32;

        let mut left_buffer = Buffer::new(&mut font_system, metrics);
        let mut right_buffer = Buffer::new(&mut font_system, metrics);

        left_buffer.set_size(&mut font_system, None, Some(height as f32));
        right_buffer.set_size(&mut font_system, None, Some(height as f32));

        Self {
            height,
            base_font_size,
            scale_factor,
            background_color: utils::parse_color(&config.ui.status_bar_bg),
            left_buffer,
            right_buffer,
        }
    }

    pub fn set_scale(&mut self, scale: f32) {
        if (self.scale_factor - scale).abs() < f32::EPSILON {
            return;
        }

        let mut font_system = UI_FONT_SYSTEM.get().unwrap().lock().unwrap();

        self.scale_factor = scale;
        let base_line_height = self.base_font_size * 1.2;

        let metrics = Metrics::new(self.base_font_size * scale, base_line_height * scale);
        self.height = (base_line_height * scale) as u32;

        self.left_buffer.set_metrics(&mut font_system, metrics);
        self.right_buffer.set_metrics(&mut font_system, metrics);

        self.left_buffer
            .set_size(&mut font_system, None, Some(self.height as f32));
        self.right_buffer
            .set_size(&mut font_system, None, Some(self.height as f32));
    }

    pub fn draw(
        &mut self,
        target: &mut FrameBuffer,
        scale_percent: u32,
        index: usize,
        total: usize,
        path: &str,
        is_marked: bool,
        input_mode: &InputMode,
    ) {
        // Lock both globals for the duration of the draw
        let mut font_system = UI_FONT_SYSTEM.get().unwrap().lock().unwrap();
        let mut swash_cache = UI_SWASH_CACHE
            .get_or_init(|| Mutex::new(SwashCache::new()))
            .lock()
            .unwrap();

        let width = target.width;
        let target_height = target.height;
        let bar_top = (target_height - self.height) as i32;

        let config = AppConfig::get();
        let text_color_rgb = utils::parse_color(&config.ui.status_bar_fg);
        let family_name = Family::Name(&config.ui.font_family);
        let attrs = Attrs::new().family(family_name);

        // Calculate right text position and width
        let mark = if is_marked { "* " } else { "" };
        let right_text = format!("{}{}% {}/{}", mark, scale_percent, index, total);
        self.right_buffer.set_text(
            &mut font_system,
            &right_text,
            &attrs,
            Shaping::Advanced,
            None,
        );
        self.right_buffer
            .shape_until_scroll(&mut font_system, false);

        let right_w = Self::measure_width(&self.right_buffer) as u32;
        let right_x = (width as i32) - (right_w as i32) - 5;

        // Calculate available width for the path on the left
        // Leave a margin of roughly 5 chars (estimated by font size)
        let margin_px = (config.ui.font_size as u32 * 5).max(50);
        let max_path_w = (right_x - 5 - margin_px as i32).max(0) as u32;

        let display_text = match input_mode {
            InputMode::Normal => path.to_string(),
            InputMode::WaitingForHandler => "[Handler] Press key... (Esc to cancel)".to_string(),
            InputMode::AwaitingTarget(_) => {
                "[Target] (c)urrent or (m)arked? (Esc to cancel)".to_string()
            }
        };

        self.left_buffer.set_text(
            &mut font_system,
            &display_text,
            &attrs,
            Shaping::Advanced,
            None,
        );
        self.left_buffer.shape_until_scroll(&mut font_system, false);

        // Binary Search Truncation
        if Self::measure_width(&self.left_buffer) > max_path_w as f32 {
            let full_path_chars: Vec<char> = path.chars().collect();
            let n = full_path_chars.len();

            let mut low = 0;
            let mut high = n;
            let mut best_str = String::from("…");

            while low <= high {
                let mid = (low + high) / 2;
                // Create suffix from mid to end
                let suffix: String = full_path_chars[mid..].iter().collect();
                let test_str = format!("…{}", suffix);

                self.left_buffer.set_text(
                    &mut font_system,
                    &test_str,
                    &attrs,
                    Shaping::Advanced,
                    None,
                );
                self.left_buffer.shape_until_scroll(&mut font_system, false);

                if Self::measure_width(&self.left_buffer) <= max_path_w as f32 {
                    best_str = test_str;
                    high = mid.saturating_sub(1); // Try to include more chars (move left)
                } else {
                    low = mid + 1; // Need to exclude more chars (move right)
                }
            }

            // Final update with the best fitting string found
            self.left_buffer
                .set_text(&mut font_system, &best_str, &attrs, Shaping::Advanced, None);
            self.left_buffer.shape_until_scroll(&mut font_system, false);
        }

        // Draw Full-width Background Bar
        target.draw_rect(0, bar_top, width, self.height, self.background_color);

        // Draw Buffers using global engines
        Self::draw_buffer(
            &mut font_system,
            &mut swash_cache,
            target,
            &self.left_buffer,
            5,
            bar_top,
            text_color_rgb,
        );
        Self::draw_buffer(
            &mut font_system,
            &mut swash_cache,
            target,
            &self.right_buffer,
            right_x,
            bar_top,
            text_color_rgb,
        );
    }

    fn measure_width(buffer: &Buffer) -> f32 {
        buffer
            .layout_runs()
            .map(|run| run.line_w)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0)
    }

    fn draw_buffer(
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
        target: &mut FrameBuffer,
        buffer: &Buffer,
        start_x: i32,
        start_y: i32,
        text_color_rgb: (u8, u8, u8),
    ) {
        let (r, g, b) = text_color_rgb;
        let text_color = Color::rgb(r, g, b);

        buffer.draw(
            font_system,
            swash_cache,
            text_color,
            |x, y, _w, _h, color| {
                let abs_x = start_x + x;
                let abs_y = start_y + y;

                if abs_x < 0
                    || abs_y < 0
                    || abs_x >= target.width as i32
                    || abs_y >= target.height as i32
                {
                    return;
                }

                let alpha = color.a();
                if alpha == 0 {
                    return;
                }

                let idx = ((abs_y as u32 * target.width + abs_x as u32) * 4) as usize;

                if idx + 3 < target.frame.len() {
                    let bg_r = target.frame[idx] as u32;
                    let bg_g = target.frame[idx + 1] as u32;
                    let bg_b = target.frame[idx + 2] as u32;

                    let fg_r = color.r() as u32;
                    let fg_g = color.g() as u32;
                    let fg_b = color.b() as u32;
                    let a = alpha as u32;

                    let r = (fg_r * a + bg_r * (255 - a)) / 255;
                    let g = (fg_g * a + bg_g * (255 - a)) / 255;
                    let b = (fg_b * a + bg_b * (255 - a)) / 255;

                    target.frame[idx] = r as u8;
                    target.frame[idx + 1] = g as u8;
                    target.frame[idx + 2] = b as u8;
                }
            },
        );
    }
}
