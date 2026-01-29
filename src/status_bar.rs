use crate::config::AppConfig;
use crate::frame_buffer::FrameBuffer;
use crate::utils;
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};

pub struct StatusBar {
    pub height: u32,
    base_font_size: f32,
    scale_factor: f32,
    background_color: (u8, u8, u8),
    font_system: FontSystem,
    swash_cache: SwashCache,
    left_buffer: Buffer,
    right_buffer: Buffer,
}

impl StatusBar {
    pub fn new() -> Self {
        let config = AppConfig::get();
        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();

        // Base sizes in "points" (approximate)
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
            font_system,
            swash_cache,
            left_buffer,
            right_buffer,
        }
    }

    pub fn set_scale(&mut self, scale: f32) {
        if (self.scale_factor - scale).abs() < f32::EPSILON {
            return;
        }

        self.scale_factor = scale;
        let base_line_height = self.base_font_size * 1.2;

        let metrics = Metrics::new(self.base_font_size * scale, base_line_height * scale);
        self.height = (base_line_height * scale) as u32;

        self.left_buffer.set_metrics(&mut self.font_system, metrics);
        self.right_buffer
            .set_metrics(&mut self.font_system, metrics);

        self.left_buffer
            .set_size(&mut self.font_system, None, Some(self.height as f32));
        self.right_buffer
            .set_size(&mut self.font_system, None, Some(self.height as f32));
    }

    pub fn draw(
        &mut self,
        target: &mut FrameBuffer,
        scale_percent: u32,
        index: usize,
        total: usize,
        path: &str,
    ) {
        let width = target.width;
        let target_height = target.height;
        let bar_top = (target_height - self.height) as i32;

        // Borrow fields individually
        let font_system = &mut self.font_system;
        let swash_cache = &mut self.swash_cache;
        let left_buffer = &mut self.left_buffer;
        let right_buffer = &mut self.right_buffer;

        let config = AppConfig::get();
        let text_color_rgb = utils::parse_color(&config.ui.status_bar_fg);
        let family_name = Family::Name(&config.ui.font_family);
        let attrs = Attrs::new().family(family_name);

        // Update Left Text (Path)
        left_buffer.set_text(font_system, path, &attrs, Shaping::Advanced, None);

        // Update Right Text (Status)
        let right_text = format!("{}% {}/{}", scale_percent, index, total);
        right_buffer.set_text(font_system, &right_text, &attrs, Shaping::Advanced, None);

        // Shape buffers
        left_buffer.shape_until_scroll(font_system, false);
        right_buffer.shape_until_scroll(font_system, false);

        // Calculate positions
        let text_y = bar_top;
        let left_x = 5;

        let right_w = Self::measure_width(right_buffer) as u32;
        let right_x = (width as i32) - (right_w as i32) - 5;

        // Draw Full-width Background Bar
        target.draw_rect(0, bar_top, width, self.height, self.background_color);

        // Draw Text
        Self::draw_buffer(
            font_system,
            swash_cache,
            target,
            left_buffer,
            left_x,
            text_y,
            text_color_rgb,
        );
        Self::draw_buffer(
            font_system,
            swash_cache,
            target,
            right_buffer,
            right_x,
            text_y,
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
                    target.frame[idx + 3] = 255;
                }
            },
        );
    }
}
