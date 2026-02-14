use image::{ImageBuffer, Rgba};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct FrameData {
    pub pixels: Vec<u8>,
    pub delay: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageFormat {
    Raster,
    Svg,
}

#[derive(Clone)]
pub enum ImageSlot {
    PendingMetadata,
    MetadataLoaded(ImageItem),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ImageItem {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
}

#[derive(Clone, Debug)]
pub struct LoadedImage {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<FrameData>,
}

impl LoadedImage {
    pub fn rotate(&mut self, clockwise: bool) {
        let mut new_size = None;
        for frame in &mut self.frames {
            if let Some(img_buf) = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
                self.width,
                self.height,
                std::mem::take(&mut frame.pixels),
            ) {
                let rotated = if clockwise {
                    image::imageops::rotate90(&img_buf)
                } else {
                    image::imageops::rotate270(&img_buf)
                };
                new_size = Some((rotated.width(), rotated.height()));
                frame.pixels = rotated.into_raw();
            }
        }
        if let Some((w, h)) = new_size {
            self.width = w;
            self.height = h;
        }
    }

    pub fn flip_horizontal(&mut self) {
        for frame in &mut self.frames {
            if let Some(img_buf) = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
                self.width,
                self.height,
                std::mem::take(&mut frame.pixels),
            ) {
                frame.pixels = image::imageops::flip_horizontal(&img_buf).into_raw();
            }
        }
    }

    pub fn flip_vertical(&mut self) {
        for frame in &mut self.frames {
            if let Some(img_buf) = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
                self.width,
                self.height,
                std::mem::take(&mut frame.pixels),
            ) {
                frame.pixels = image::imageops::flip_vertical(&img_buf).into_raw();
            }
        }
    }
}
