use image::{AnimationDecoder, ImageBuffer, ImageReader, Rgba};
use resvg::usvg::{self, Options, Tree};
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tiny_skia::Pixmap;

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
    pub thumb: Option<(u32, u32, Vec<u8>)>,
}

impl ImageItem {
    pub fn from_path(path: &str) -> Result<Self, String> {
        let path_obj = Path::new(path);
        let file_data = std::fs::read(path_obj).map_err(|e| format!("Read error: {}", e))?;

        let kind = infer::get(&file_data);
        let mime = kind
            .map(|k| k.mime_type())
            .unwrap_or("application/octet-stream");

        let is_svg_content = || {
            let header = &file_data[..file_data.len().min(1024)];
            let content = String::from_utf8_lossy(header);
            content.to_lowercase().contains("<svg")
        };

        match mime {
            "image/svg+xml" => Self::decode_svg(&file_data, path_obj),

            "text/xml" | "application/xml" | "text/plain" | "application/octet-stream" => {
                if is_svg_content() {
                    Self::decode_svg(&file_data, path_obj)
                } else {
                    Err(format!(
                        "File is {}, but no SVG data found (File: {})",
                        mime, path
                    ))
                }
            }

            "image/gif" => Self::decode_gif(&file_data, path),

            m if m.starts_with("image/") => Self::decode_static(&file_data, path),

            _ => Err(format!(
                "Unsupported or mismatched format: {} (File: {})",
                mime, path
            )),
        }
    }

    fn decode_svg(file_data: &[u8], path_obj: &Path) -> Result<Self, String> {
        let mut opt = Options::default();
        opt.resources_dir = path_obj.parent().map(|p| p.to_path_buf());

        opt.fontdb = Arc::new(crate::utils::get_svg_font_db().clone());

        let tree =
            Tree::from_data(file_data, &opt).map_err(|e| format!("SVG Parse Error: {}", e))?;

        let size = tree.size().to_int_size();
        let (width, height) = (size.width(), size.height());

        let mut pixmap = Pixmap::new(width, height).ok_or("Failed to create pixmap")?;
        resvg::render(&tree, usvg::Transform::default(), &mut pixmap.as_mut());

        Ok(Self {
            path: path_obj.to_string_lossy().into(),
            width,
            height,
            frames: vec![FrameData {
                pixels: pixmap.take(),
                delay: std::time::Duration::MAX,
            }],
            thumb: None,
        })
    }

    fn decode_gif(file_data: &[u8], path: &str) -> Result<Self, String> {
        let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(file_data))
            .map_err(|e| format!("GIF Decoder error: {}", e))?;

        let gif_frames = decoder
            .into_frames()
            .collect_frames()
            .map_err(|e| format!("GIF Frame error: {}", e))?;

        if gif_frames.is_empty() {
            return Self::decode_static(file_data, path);
        }

        let first = gif_frames[0].buffer();
        let (width, height) = (first.width(), first.height());

        let frames = gif_frames
            .into_iter()
            .map(|f| {
                let (n, d) = f.delay().numer_denom_ms();
                let delay = if d == 0 {
                    Duration::from_millis(100)
                } else {
                    Duration::from_millis(n as u64 / d as u64)
                };
                FrameData {
                    pixels: f.into_buffer().into_raw(),
                    delay,
                }
            })
            .collect();

        Ok(Self {
            path: path.into(),
            width,
            height,
            frames,
            thumb: None,
        })
    }

    fn decode_static(file_data: &[u8], path: &str) -> Result<Self, String> {
        let img = ImageReader::new(Cursor::new(file_data))
            .with_guessed_format()
            .map_err(|e| e.to_string())?
            .decode()
            .map_err(|e| e.to_string())?;

        let (width, height) = (img.width(), img.height());

        Ok(Self {
            path: path.into(),
            width,
            height,
            frames: vec![FrameData {
                pixels: img.to_rgba8().into_raw(),
                delay: Duration::MAX,
            }],
            thumb: None,
        })
    }

    pub fn get_thumbnail(&mut self, size: u32) -> Option<(u32, u32, &[u8])> {
        if self.thumb.is_none() {
            if let Some(first_frame) = self.frames.first() {
                if let Some(img_buf) = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
                    self.width,
                    self.height,
                    first_frame.pixels.clone(),
                ) {
                    // We avoid using `image::imageops::thumbnail` because it distort the image
                    let aspect = self.width as f64 / self.height as f64;
                    let (nwidth, nheight) = if aspect >= 1.0 {
                        (size, (size as f64 / aspect) as u32)
                    } else {
                        ((size as f64 * aspect) as u32, size)
                    };

                    let nwidth = nwidth.max(1);
                    let nheight = nheight.max(1);

                    let thumb = image::imageops::resize(
                        &img_buf,
                        nwidth,
                        nheight,
                        image::imageops::FilterType::Triangle,
                    );
                    self.thumb = Some((thumb.width(), thumb.height(), thumb.into_raw()));
                }
            }
        }
        self.thumb
            .as_ref()
            .map(|(w, h, data)| (*w, *h, data.as_slice()))
    }

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
