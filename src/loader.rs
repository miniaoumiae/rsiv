use crate::app::AppEvent;
use crate::image_item::{FrameData, ImageFormat, ImageItem, LoadedImage};
use crossbeam_channel::{unbounded, Receiver, Sender};
use image::{AnimationDecoder, ImageBuffer, ImageReader, Rgba};
use memmap2::Mmap;
use rayon::prelude::*;
use resvg::usvg::{self, Options, Tree};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;
use tiny_skia::Pixmap;
use walkdir::WalkDir;
use winit::event_loop::EventLoopProxy;

// Discovery

pub fn identify_format(path: &Path) -> Result<ImageFormat, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut buffer = [0; 1024];
    let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
    let data = &buffer[..n];

    let kind = infer::get(data);
    let mime = kind.map(|k| k.mime_type()).unwrap_or("unknown/raw");

    match mime {
        "image/svg+xml" => Ok(ImageFormat::Svg),
        "image/gif" => Ok(ImageFormat::Gif),
        m if m.starts_with("image/") => Ok(ImageFormat::Static),
        _ => {
            // Manual sniffing
            let content = String::from_utf8_lossy(data).to_lowercase();
            if content.contains("<svg") {
                Ok(ImageFormat::Svg)
            } else {
                Err(mime.to_string())
            }
        }
    }
}

pub fn probe_image(path: &Path, format: ImageFormat) -> Result<(u32, u32), String> {
    match format {
        ImageFormat::Svg => {
            let opt = Options {
                resources_dir: path.parent().map(|p| p.to_path_buf()),
                fontdb: Arc::new(crate::utils::get_svg_font_db().clone()),
                ..Default::default()
            };
            let data = std::fs::read(path).map_err(|e| e.to_string())?;
            let tree = Tree::from_data(&data, &opt).map_err(|e| e.to_string())?;
            let size = tree.size().to_int_size();
            Ok((size.width(), size.height()))
        }
        ImageFormat::Gif | ImageFormat::Static => {
            let reader = ImageReader::open(path)
                .map_err(|e| e.to_string())?
                .with_guessed_format()
                .map_err(|e| e.to_string())?;

            let dims = reader.into_dimensions().map_err(|e| e.to_string())?;
            Ok(dims)
        }
    }
}

pub fn spawn_discovery_worker(
    paths: Vec<String>,
    recursive: bool,
    proxy: EventLoopProxy<AppEvent>,
) {
    thread::spawn(move || {
        let mut files = Vec::new();
        for p in paths {
            if !Path::new(&p).exists() {
                continue;
            }
            let mut walker = WalkDir::new(p);
            if !recursive {
                walker = walker.max_depth(1);
            }
            for entry in walker
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
            {
                files.push(entry.path().to_path_buf());
            }
        }
        files.sort();

        // 1. Identify Format
        let tasks: Vec<(PathBuf, ImageFormat)> = files
            .into_iter()
            .filter_map(|path| match identify_format(&path) {
                Ok(format) => Some((path, format)),
                Err(_) => None,
            })
            .collect();

        let _ = proxy.send_event(AppEvent::InitialCount(tasks.len()));

        // 2. Probe Dimensions (Parallel)
        tasks
            .into_par_iter()
            .enumerate()
            .for_each(|(idx, (path, format))| match probe_image(&path, format) {
                Ok((width, height)) => {
                    let item = ImageItem {
                        path,
                        width,
                        height,
                        format,
                    };
                    let _ = proxy.send_event(AppEvent::MetadataLoaded(idx, item));
                }
                Err(e) => {
                    let _ = proxy.send_event(AppEvent::MetadataError(idx, e));
                }
            });

        let _ = proxy.send_event(AppEvent::DiscoveryComplete);
    });
}

// --- Loading ---

pub enum LoadRequest {
    LoadImage(PathBuf, ImageFormat),
    LoadThumbnail(PathBuf, ImageFormat, u32), // path, format, target_size
}

pub struct Loader {
    urgent_tx: Sender<LoadRequest>,
    background_stack: Arc<(Mutex<VecDeque<LoadRequest>>, Condvar)>,
}

impl Loader {
    pub fn new(proxy: EventLoopProxy<AppEvent>) -> Self {
        let (urgent_tx, urgent_rx) = unbounded();
        let background_stack = Arc::new((Mutex::new(VecDeque::new()), Condvar::new()));

        // Spawn multiple workers based on CPU count
        let num_workers = (num_cpus::get() / 2).max(1);
        for _ in 0..num_workers {
            let u_rx = urgent_rx.clone();
            let b_stack = background_stack.clone();
            let p = proxy.clone();
            thread::spawn(move || worker_loop(u_rx, b_stack, p));
        }

        Self {
            urgent_tx,
            background_stack,
        }
    }

    pub fn request_image(&self, path: PathBuf, format: ImageFormat) {
        let _ = self.urgent_tx.send(LoadRequest::LoadImage(path, format));
    }

    pub fn request_thumbnail(&self, path: PathBuf, format: ImageFormat, size: u32) {
        let (lock, cvar) = &*self.background_stack;
        let mut stack = lock.lock().unwrap();

        // LIFO: Push to the front so the newest scroll target is handled first
        stack.push_front(LoadRequest::LoadThumbnail(path, format, size));

        // If the stack gets too huge (e.g. > 200), drop the oldest requests
        // Removing from the back drops the oldest (least priority) items
        if stack.len() > 200 {
            stack.pop_back();
        }

        cvar.notify_one();
    }
}

fn worker_loop(
    urgent_rx: Receiver<LoadRequest>,
    background_stack: Arc<(Mutex<VecDeque<LoadRequest>>, Condvar)>,
    proxy: EventLoopProxy<AppEvent>,
) {
    loop {
        // Try to get an URGENT task
        if let Ok(req) = urgent_rx.try_recv() {
            process_request(req, &proxy);
            continue; // Go back to top to check for more urgent tasks
        }

        // No urgent task? Check the BACKGROUND stack
        let req = {
            let (lock, cvar) = &*background_stack;
            let mut stack = lock.lock().unwrap();

            if stack.is_empty() {
                // If stack is empty, wait for a signal OR a short timeout
                // (the timeout allows us to go back to the top and check urgent_rx)
                let _ = cvar
                    .wait_timeout(stack, Duration::from_millis(100))
                    .unwrap();
                None
            } else {
                stack.pop_front()
            }
        };

        if let Some(r) = req {
            process_request(r, &proxy);
        }
    }
}

fn process_request(req: LoadRequest, proxy: &EventLoopProxy<AppEvent>) {
    match req {
        LoadRequest::LoadImage(path, format) => match load_full_image(&path, format) {
            Ok(img) => {
                let _ = proxy.send_event(AppEvent::ImagePixelsLoaded(path, Arc::new(img)));
            }
            Err(e) => {
                let _ = proxy.send_event(AppEvent::LoadError(path, e));
            }
        },
        LoadRequest::LoadThumbnail(path, format, size) => {
            match load_thumbnail(&path, format, size) {
                Ok(thumb) => {
                    let _ = proxy.send_event(AppEvent::ThumbnailLoaded(path, Arc::new(thumb)));
                }
                Err(_) => {
                    let _ =
                        proxy.send_event(AppEvent::LoadError(path, "Thumbnail Error".to_string()));
                }
            }
        }
    }
}

fn load_full_image(path: &Path, format: ImageFormat) -> Result<LoadedImage, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mmap = unsafe { Mmap::map(&file).map_err(|e| e.to_string())? };
    let data = &mmap[..];

    match format {
        ImageFormat::Svg => decode_svg(data, path),
        ImageFormat::Gif => decode_gif(data, path),
        ImageFormat::Static => decode_static(data),
    }
}

fn load_thumbnail(
    path: &Path,
    format: ImageFormat,
    size: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    // Optimization: For static images, decode and resize immediately to save memory.
    if format == ImageFormat::Static {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let mmap = unsafe { Mmap::map(&file).map_err(|e| e.to_string())? };
        let data = &mmap[..];

        let mut reader = ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;

        // Safety limits
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(32768);
        limits.max_image_height = Some(32768);
        limits.max_alloc = Some(4 * 1024 * 1024 * 1024);
        reader.limits(limits);

        // Decode and resize
        let img = reader.decode().map_err(|e| e.to_string())?;
        // Use `thumbnail` which downscales to fit within size x size, preserving aspect ratio
        let thumb = img.thumbnail(size, size);
        return Ok((thumb.width(), thumb.height(), thumb.to_rgba8().into_raw()));
    }

    // Fallback for others: load full and resize
    let img = load_full_image(path, format)?;
    if let Some(first_frame) = img.frames.first() {
        if let Some(img_buf) = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
            img.width,
            img.height,
            first_frame.pixels.clone(),
        ) {
            let aspect = img.width as f64 / img.height as f64;
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
            return Ok((thumb.width(), thumb.height(), thumb.into_raw()));
        }
    }
    Err("No frames".to_string())
}

// Decoding Helpers

fn decode_svg(file_data: &[u8], path_obj: &Path) -> Result<LoadedImage, String> {
    let opt = Options {
        resources_dir: path_obj.parent().map(|p| p.to_path_buf()),
        fontdb: Arc::new(crate::utils::get_svg_font_db().clone()),
        ..Default::default()
    };

    let tree = Tree::from_data(file_data, &opt).map_err(|e| format!("SVG Parse Error: {}", e))?;
    let size = tree.size().to_int_size();
    let (width, height) = (size.width(), size.height());

    let mut pixmap = Pixmap::new(width, height).ok_or("Failed to create pixmap")?;
    resvg::render(&tree, usvg::Transform::default(), &mut pixmap.as_mut());

    Ok(LoadedImage {
        width,
        height,
        frames: vec![FrameData {
            pixels: pixmap.take(),
            delay: Duration::MAX,
        }],
    })
}

fn decode_gif(file_data: &[u8], _path: &Path) -> Result<LoadedImage, String> {
    let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(file_data))
        .map_err(|e| format!("GIF Decoder error: {}", e))?;

    let gif_frames = decoder
        .into_frames()
        .collect_frames()
        .map_err(|e| format!("GIF Frame error: {}", e))?;

    if gif_frames.is_empty() {
        return decode_static(file_data);
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

    Ok(LoadedImage {
        width,
        height,
        frames,
    })
}

fn decode_static(file_data: &[u8]) -> Result<LoadedImage, String> {
    let mut reader = ImageReader::new(Cursor::new(file_data))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;

    // Safety limits
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    limits.max_alloc = Some(4 * 1024 * 1024 * 1024); // 4GB limit for decompression
    reader.limits(limits);

    let img = reader.decode().map_err(|e| e.to_string())?;

    let (width, height) = (img.width(), img.height());

    Ok(LoadedImage {
        width,
        height,
        frames: vec![FrameData {
            pixels: img.to_rgba8().into_raw(),
            delay: Duration::MAX,
        }],
    })
}
