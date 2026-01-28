use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::Rgb888,
    prelude::*,
};
use std::convert::Infallible;

pub struct FrameBuffer<'a> {
    pub frame: &'a mut [u8],
    pub width: u32,
    pub height: u32,
}

impl<'a> FrameBuffer<'a> {
    pub fn new(frame: &'a mut [u8], width: u32, height: u32) -> Self {
        Self {
            frame,
            width,
            height,
        }
    }
}

impl OriginDimensions for FrameBuffer<'_> {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

impl DrawTarget for FrameBuffer<'_> {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels.into_iter() {
            if point.x >= 0
                && point.y >= 0
                && point.x < self.width as i32
                && point.y < self.height as i32
            {
                let idx = ((point.y as u32 * self.width + point.x as u32) * 4) as usize;
                if idx + 3 < self.frame.len() {
                    self.frame[idx] = color.r();
                    self.frame[idx + 1] = color.g();
                    self.frame[idx + 2] = color.b();
                    self.frame[idx + 3] = 255; // Alpha
                }
            }
        }
        Ok(())
    }
}
