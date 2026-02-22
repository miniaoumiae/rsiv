use image::{ImageBuffer, Rgba};
use resvg::usvg;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tiny_skia::Transform;

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

#[derive(Clone)]
pub enum LoadedAsset {
    Raster {
        width: u32,
        height: u32,
        frames: Vec<FrameData>,
    },
    Vector {
        tree: Arc<usvg::Tree>,
        base_width: u32,
        base_height: u32,
        internal_transform: Transform,
    },
}

impl std::fmt::Debug for LoadedAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raster {
                width,
                height,
                frames,
            } => f
                .debug_struct("Raster")
                .field("width", width)
                .field("height", height)
                .field("frames_count", &frames.len())
                .finish(),
            Self::Vector {
                base_width,
                base_height,
                ..
            } => f
                .debug_struct("Vector")
                .field("base_width", base_width)
                .field("base_height", base_height)
                .finish(),
        }
    }
}

impl LoadedAsset {
    pub fn size_in_kb(&self) -> u32 {
        match self {
            LoadedAsset::Raster { frames, .. } => {
                let bytes: usize = frames.iter().map(|f| f.pixels.len()).sum();
                ((bytes / 1024) as u32).max(1)
            }
            LoadedAsset::Vector { .. } => 100,
        }
    }

    pub fn frame_count(&self) -> usize {
        match self {
            LoadedAsset::Raster { frames, .. } => frames.len(),
            LoadedAsset::Vector { .. } => 1,
        }
    }

    pub fn frame_delay(&self, idx: usize) -> Duration {
        match self {
            LoadedAsset::Raster { frames, .. } => frames[idx % frames.len()].delay,
            LoadedAsset::Vector { .. } => Duration::MAX,
        }
    }

    pub fn rotate(&mut self, clockwise: bool) -> bool {
        match self {
            LoadedAsset::Raster {
                width,
                height,
                frames,
            } => {
                let mut new_size = None;
                for frame in frames {
                    if let Some(img_buf) = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
                        *width,
                        *height,
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
                    *width = w;
                    *height = h;
                }
                true
            }
            LoadedAsset::Vector {
                base_width,
                base_height,
                internal_transform,
                ..
            } => {
                let old_w = *base_width as f32;
                let old_h = *base_height as f32;
                std::mem::swap(base_width, base_height);
                let new_w = *base_width as f32;
                let new_h = *base_height as f32;

                let angle = if clockwise { 90.0 } else { -90.0 };
                let mut ts = Transform::from_translate(new_w / 2.0, new_h / 2.0);
                ts = ts.pre_rotate(angle);
                ts = ts.pre_translate(-old_w / 2.0, -old_h / 2.0);

                *internal_transform = ts.post_concat(*internal_transform);
                true
            }
        }
    }

    pub fn flip_horizontal(&mut self) -> bool {
        match self {
            LoadedAsset::Raster {
                width,
                height,
                frames,
            } => {
                for frame in frames {
                    if let Some(img_buf) = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
                        *width,
                        *height,
                        std::mem::take(&mut frame.pixels),
                    ) {
                        frame.pixels = image::imageops::flip_horizontal(&img_buf).into_raw();
                    }
                }
                true
            }
            LoadedAsset::Vector {
                base_width,
                internal_transform,
                ..
            } => {
                let w = *base_width as f32;
                let mut ts = Transform::from_translate(w / 2.0, 0.0);
                ts = ts.pre_scale(-1.0, 1.0);
                ts = ts.pre_translate(-w / 2.0, 0.0);
                *internal_transform = ts.post_concat(*internal_transform);
                true
            }
        }
    }

    pub fn flip_vertical(&mut self) -> bool {
        match self {
            LoadedAsset::Raster {
                width,
                height,
                frames,
            } => {
                for frame in frames {
                    if let Some(img_buf) = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
                        *width,
                        *height,
                        std::mem::take(&mut frame.pixels),
                    ) {
                        frame.pixels = image::imageops::flip_vertical(&img_buf).into_raw();
                    }
                }
                true
            }
            LoadedAsset::Vector {
                base_height,
                internal_transform,
                ..
            } => {
                let h = *base_height as f32;
                let mut ts = Transform::from_translate(0.0, h / 2.0);
                ts = ts.pre_scale(1.0, -1.0);
                ts = ts.pre_translate(0.0, -h / 2.0);
                *internal_transform = ts.post_concat(*internal_transform);
                true
            }
        }
    }
}
