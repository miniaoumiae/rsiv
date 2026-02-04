use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct FrameData {
    pub pixels: Vec<u8>,
    pub delay: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageFormat {
    Static,
    Gif,
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

// Separate struct for the heavy data, managed by Cache
#[derive(Clone, Debug)]
pub struct LoadedImage {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<FrameData>,
}