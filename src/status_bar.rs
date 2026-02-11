use crate::app::InputMode;
use crate::config::AppConfig;
use crate::frame_buffer::FrameBuffer;
use crate::utils;
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
use std::fmt::Write;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

static UI_FONT_SYSTEM: OnceLock<Mutex<FontSystem>> = OnceLock::new();
static UI_SWASH_CACHE: OnceLock<Mutex<SwashCache>> = OnceLock::new();

#[derive(Debug, Clone)]
enum StatusToken {
    Literal(String),
    Path,
    Prefix,
    Slideshow,
    Zoom,
    Index,
    Mark,
}

pub struct StatusContext<'a> {
    pub scale_percent: u32,
    pub index: usize,
    pub total: usize,
    pub path: &'a str,
    pub is_marked: bool,
    pub input_mode: &'a InputMode,
    pub prefix_count: Option<usize>,
    pub slideshow_on: bool,
    pub slideshow_delay: Duration,
    pub filter_text: &'a str,
}

pub struct StatusBar {
    pub height: u32,
    base_font_size: f32,
    scale_factor: f32,
    background_color: (u8, u8, u8),
    left_buffer: Buffer,
    right_buffer: Buffer,

    // COMPILED INSTRUCTIONS
    left_tokens: Vec<StatusToken>,
    right_tokens: Vec<StatusToken>,

    // Optimization: Reusable buffer for text generation
    scratch_buffer: String,

    // Caching for path truncation
    cached_raw_path: String,
    cached_max_width: u32,
    cached_display_text: String,
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

        // Compile the formats from config
        let left_tokens = Self::compile_format(&config.ui.status_format_left);
        let right_tokens = Self::compile_format(&config.ui.status_format_right);

        Self {
            height,
            base_font_size,
            scale_factor,
            background_color: utils::parse_color(&config.ui.status_bar_bg),
            left_buffer,
            right_buffer,
            left_tokens,
            right_tokens,
            scratch_buffer: String::with_capacity(128),
            cached_raw_path: String::new(),
            cached_max_width: 0,
            cached_display_text: String::new(),
        }
    }

    fn compile_format(fmt: &str) -> Vec<StatusToken> {
        let mut tokens = Vec::new();
        let mut literal_buffer = String::new();
        let mut chars = fmt.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '%' {
                // Flush any pending literal text
                if !literal_buffer.is_empty() {
                    tokens.push(StatusToken::Literal(literal_buffer.clone()));
                    literal_buffer.clear();
                }

                // Check the next character for the token type
                if let Some(next) = chars.next() {
                    match next {
                        'p' => tokens.push(StatusToken::Path),
                        'P' => tokens.push(StatusToken::Prefix),
                        's' => tokens.push(StatusToken::Slideshow),
                        'z' => tokens.push(StatusToken::Zoom),
                        'i' => tokens.push(StatusToken::Index),
                        'm' => tokens.push(StatusToken::Mark),
                        '%' => literal_buffer.push('%'), // Escaped %% becomes literal %
                        c => {
                            // Unknown specifier, treat as literal text
                            literal_buffer.push('%');
                            literal_buffer.push(c);
                        }
                    }
                } else {
                    // Trailing % at end of string
                    literal_buffer.push('%');
                }
            } else {
                literal_buffer.push(c);
            }
        }

        // Final flush
        if !literal_buffer.is_empty() {
            tokens.push(StatusToken::Literal(literal_buffer));
        }

        tokens
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

        // Invalidate cache
        self.cached_max_width = 0;
    }

    fn render_tokens(target: &mut String, tokens: &[StatusToken], ctx: &StatusContext) {
        // Use write! macro to append directly to target
        for token in tokens {
            match token {
                StatusToken::Literal(s) => {
                    let _ = write!(target, "{}", s);
                }
                StatusToken::Path => match ctx.input_mode {
                    InputMode::Normal => {
                        let _ = write!(target, "{}", ctx.path);
                    }
                    InputMode::Filtering => {
                        let _ = write!(target, "/{}█", ctx.filter_text);
                    }
                    InputMode::WaitingForHandler => {
                        let _ = write!(target, "[Handler] Press key... (Esc to cancel)");
                    }
                    InputMode::AwaitingTarget(_) => {
                        let _ = write!(target, "[Target] (c)urrent/(m)arked? (Esc to cancel)");
                    }
                },
                StatusToken::Prefix => {
                    if let Some(n) = ctx.prefix_count {
                        let _ = write!(target, "{}", n);
                    }
                }
                StatusToken::Slideshow => {
                    if ctx.slideshow_on {
                        let _ = write!(target, "{}s", ctx.slideshow_delay.as_secs());
                    }
                }
                StatusToken::Zoom => {
                    let _ = write!(target, "{}%", ctx.scale_percent);
                }
                StatusToken::Index => {
                    let _ = write!(target, "{}/{}", ctx.index, ctx.total);
                }
                StatusToken::Mark => {
                    if ctx.is_marked {
                        let _ = write!(target, "*");
                    }
                }
            }
        }
    }

    pub fn draw(&mut self, target: &mut FrameBuffer, ctx: StatusContext) {
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

        // --- Render Right Side ---
        self.scratch_buffer.clear();
        Self::render_tokens(&mut self.scratch_buffer, &self.right_tokens, &ctx);

        self.right_buffer.set_text(
            &mut font_system,
            &self.scratch_buffer,
            &attrs,
            Shaping::Advanced,
            None,
        );
        self.right_buffer
            .shape_until_scroll(&mut font_system, false);

        let right_w = Self::measure_width(&self.right_buffer) as u32;
        let right_x = (width as i32) - (right_w as i32) - 5;

        // Render Left Side
        self.scratch_buffer.clear();
        Self::render_tokens(&mut self.scratch_buffer, &self.left_tokens, &ctx);

        let left_full_text = self.scratch_buffer.clone();

        // Calculate available width for the path/prompt on the left
        // Leave a margin of roughly 5 chars (estimated by font size)
        let margin_px = (config.ui.font_size as u32 * 5).max(50);
        let max_path_w = (right_x - 5 - margin_px as i32).max(0) as u32;

        // Cache Check
        let needs_recalc =
            left_full_text != self.cached_raw_path || max_path_w != self.cached_max_width;

        if needs_recalc {
            self.cached_raw_path = left_full_text.clone();
            self.cached_max_width = max_path_w;

            self.left_buffer.set_text(
                &mut font_system,
                &left_full_text,
                &attrs,
                Shaping::Advanced,
                None,
            );
            self.left_buffer.shape_until_scroll(&mut font_system, false);

            // Binary Search Truncation
            if Self::measure_width(&self.left_buffer) > max_path_w as f32 {
                let full_path_chars: Vec<char> = left_full_text.chars().collect();
                let n = full_path_chars.len();

                let mut low = 0;
                let mut high = n;
                let mut best_str = String::from("…");

                while low <= high {
                    let mid = (low + high) / 2;
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
                        high = mid.saturating_sub(1);
                    } else {
                        low = mid + 1;
                    }
                }
                self.cached_display_text = best_str;
            } else {
                self.cached_display_text = left_full_text;
            }
        }

        // Always set text from cache to ensure buffer is ready for drawing
        self.left_buffer.set_text(
            &mut font_system,
            &self.cached_display_text,
            &attrs,
            Shaping::Advanced,
            None,
        );
        self.left_buffer.shape_until_scroll(&mut font_system, false);

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
