use image::{AnimationDecoder, ImageBuffer, ImageReader, Rgba};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

pub struct FrameData {
    pub pixels: Vec<u8>,
    pub delay: Duration,
}

impl std::fmt::Debug for FrameData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameData")
            .field("pixels_len", &self.pixels.len())
            .field("delay", &self.delay)
            .finish()
    }
}

#[derive(Debug)]
pub struct ImageItem {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub frames: Vec<FrameData>,
}

impl ImageItem {
    pub fn rotate(&mut self, clockwise: bool) {
        let mut new_width = 0;
        let mut new_height = 0;

        for frame in &mut self.frames {
            let pixels = std::mem::take(&mut frame.pixels);
            if let Some(img_buf) =
                ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(self.width, self.height, pixels)
            {
                let rotated = if clockwise {
                    image::imageops::rotate90(&img_buf)
                } else {
                    image::imageops::rotate270(&img_buf)
                };

                new_width = rotated.width();
                new_height = rotated.height();
                frame.pixels = rotated.into_raw();
            }
        }

        if new_width != 0 && new_height != 0 {
            self.width = new_width;
            self.height = new_height;
        }
    }

    pub fn flip_horizontal(&mut self) {
        for frame in &mut self.frames {
            let pixels = std::mem::take(&mut frame.pixels);
            if let Some(img_buf) =
                ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(self.width, self.height, pixels)
            {
                let flipped = image::imageops::flip_horizontal(&img_buf);
                frame.pixels = flipped.into_raw();
            }
        }
    }

    pub fn flip_vertical(&mut self) {
        for frame in &mut self.frames {
            let pixels = std::mem::take(&mut frame.pixels);
            if let Some(img_buf) =
                ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(self.width, self.height, pixels)
            {
                let flipped = image::imageops::flip_vertical(&img_buf);
                frame.pixels = flipped.into_raw();
            }
        }
    }

    pub fn from_path(path: &str) -> Result<Self, String> {
        let path_obj = Path::new(path);

        // Use with_guessed_format to support files without extensions or wrong extensions
        let reader = ImageReader::open(path_obj)
            .map_err(|e| format!("Failed to open file: {}", e))?
            .with_guessed_format()
            .map_err(|e| format!("Failed to guess format: {}", e))?;

        let format = reader.format();
        let mut frames = Vec::new();
        let width;
        let height;

        // NOTE: We prioritize GIF animation. Other formats could be added here.
        if Some(image::ImageFormat::Gif) == format {
            // Re-open for the decoder because ImageReader consumes ownership or we want a buffered reader specifically for GifDecoder
            // Actually, we can try to use the reader if possible, but GifDecoder takes a Read.
            // Let's just open again safely.
            let file = File::open(path_obj).map_err(|e| e.to_string())?;
            let decoder = image::codecs::gif::GifDecoder::new(BufReader::new(file))
                .map_err(|e| format!("Failed to create GIF decoder: {}", e))?;

            // collect_frames can fail
            let gif_frames = decoder
                .into_frames()
                .collect_frames()
                .map_err(|e| format!("Failed to collect GIF frames: {}", e))?;

            if !gif_frames.is_empty() {
                let first = gif_frames[0].buffer();
                width = first.width();
                height = first.height();

                for frame in gif_frames {
                    let (numer, denom) = frame.delay().numer_denom_ms();
                    let d = if denom == 0 {
                        Duration::from_millis(100)
                    } else {
                        Duration::from_millis((numer as u64) / (denom as u64))
                    };

                    frames.push(FrameData {
                        pixels: frame.into_buffer().into_raw(),
                        delay: d,
                    });
                }
            } else {
                // Empty GIF? Fallback to static decode
                let img = reader
                    .decode()
                    .map_err(|e| format!("Failed to decode image: {}", e))?;
                width = img.width();
                height = img.height();
                frames.push(FrameData {
                    pixels: img.to_rgba8().into_raw(),
                    delay: Duration::MAX,
                });
            }
        } else {
            // Static image
            let img = reader
                .decode()
                .map_err(|e| format!("Failed to decode image: {}", e))?;
            width = img.width();
            height = img.height();
            frames.push(FrameData {
                pixels: img.to_rgba8().into_raw(),
                delay: Duration::MAX,
            });
        }

        Ok(Self {
            path: path.to_string(),
            width,
            height,
            frames,
        })
    }
}
