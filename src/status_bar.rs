use crate::frame_buffer::FrameBuffer;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Alignment, Text},
};

pub struct StatusBar {
    pub height: u32,
    background_color: Rgb888,
    text_color: Rgb888,
    font_style: MonoTextStyle<'static, Rgb888>,
}

impl StatusBar {
    pub fn new() -> Self {
        // Font height is 20. Add 4px padding (2px top, 2px bottom).
        let font_height = 20;
        let padding = 4;
        let height = font_height + padding;
        
        Self {
            height,
            background_color: Rgb888::new(40, 40, 40), // Dark gray
            text_color: Rgb888::new(255, 255, 255),    // White
            font_style: MonoTextStyle::new(&FONT_10X20, Rgb888::new(255, 255, 255)),
        }
    }

    pub fn draw(
        &self,
        target: &mut FrameBuffer,
        scale_percent: u32,
        index: usize,
        total: usize,
        path: &str,
    ) {
        let width = target.size().width;
        let target_height = target.size().height;

        // Draw background
        let bg_rect = Rectangle::new(
            Point::new(0, (target_height - self.height) as i32),
            Size::new(width, self.height),
        );
        
        bg_rect
            .into_styled(PrimitiveStyle::with_fill(self.background_color))
            .draw(target)
            .ok();

        // Prepare text
        // Left: Path
        let left_text = path;
        
        // Right: "100% 1/23"
        let right_text = format!("{}% {}/{}", scale_percent, index, total);

        // Center text vertically
        // Font baseline is usually near bottom. 
        // For FONT_10X20, character size is 10x20.
        // We have height 24. 
        // y position for Text is the top-left of the text bounding box (for standard fonts in embedded-graphics 0.8?).
        // Wait, embedded-graphics text position depends on alignment.
        // But usually Point specifies the "top-left" of the first character's bounding box?
        // No, docs say: "The position of the text is specified by the top-left corner of the text."
        
        // So we want to center 20px text in 24px box.
        // Top padding = (24 - 20) / 2 = 2px.
        // y = (target_height - height) + padding_top
        
        let bar_top = (target_height - self.height) as i32;
        let text_y = bar_top + 2 + 15; // +15 is a guess for baseline?
        // Actually embedded-graphics `Text` draws from top-left.
        // `CharacterStyle` defines how it draws.
        // `MonoTextStyle` draws from top-left.
        
        let text_pos_y = bar_top + 2; // 2px padding from top

        // Draw Left Text (Path)
        Text::with_alignment(
            left_text,
            Point::new(5, text_pos_y + 14), // +14?? TextStyle baseline is different.
            // embedded-graphics 0.8: Text position is the BASELINE.
            // "The position of the text is specified by the top-left corner..." -> NO.
            // Let's check 0.8 docs.
            // "The position argument specifies the position of the baseline of the first character."
            // FONT_10X20 baseline is usually at roughly height - descent.
            // Let's try to align it visually.
            // If font height is 20, baseline might be at ~15.
            // If box is 24, top is 0 relative to box.
            // We want text centered.
            // Let's use `baseline` variable from previous code which seemed to work for 6x10.
            // previous: `baseline = (target_height as i32 - padding_y) - 2;`
            // padding_y was `(height - font_height) / 2`.
            
            self.font_style,
            Alignment::Left,
        )
        .draw(target)
        .ok();

        // Draw Right Text (Status)
        Text::with_alignment(
            &right_text,
            Point::new((width - 5) as i32, text_pos_y + 14),
            self.font_style,
            Alignment::Right,
        )
        .draw(target)
        .ok();
    }
}
