use image::{AnimationDecoder, GenericImageView, ImageReader};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

pub struct FrameData {
    // Debug for Vec<u8> might be verbose, but it's fine for now or we can implement manual Debug
    // to skip pixels. Let's just derive for simplicity as requested.
    // Actually Vec<u8> debug output is huge.
    // Let's implement manual Debug for FrameData to avoid dumping pixels.
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
    pub fn from_path(path: &str) -> Self {
        let path_obj = Path::new(path);
        
        // Detect format
        let format = ImageReader::open(path_obj)
            .expect("Failed to read image")
            .format();
        
        let mut frames = Vec::new();
        let width;
        let height;

        // Note: We prioritize GIF animation. Other formats could be added here.
        if Some(image::ImageFormat::Gif) == format {
             let file = File::open(path_obj).expect("Failed to open file");
             let decoder = image::codecs::gif::GifDecoder::new(BufReader::new(file)).expect("Failed to create GIF decoder");
             let gif_frames = decoder.into_frames().collect_frames().expect("Failed to collect GIF frames");
             
             if !gif_frames.is_empty() {
                 let first = gif_frames[0].buffer();
                 width = first.width();
                 height = first.height();
                 
                 for frame in gif_frames {
                     let (numer, denom) = frame.delay().numer_denom_ms();
                     
                     // Let's rely on Duration::from_millis(numer / denom) if denom != 0
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
                 // Empty GIF? Fallback
                 let img = image::open(path).expect("Failed to open image");
                 width = img.width();
                 height = img.height();
                 frames.push(FrameData {
                     pixels: img.to_rgba8().into_raw(),
                     delay: Duration::MAX,
                 });
             }
        } else {
             // Static image
             let img = image::open(path).expect("Failed to open image");
             width = img.width();
             height = img.height();
             frames.push(FrameData {
                 pixels: img.to_rgba8().into_raw(),
                 delay: Duration::MAX,
             });
        }

        Self {
            path: path.to_string(),
            width,
            height,
            frames,
        }
    }
}