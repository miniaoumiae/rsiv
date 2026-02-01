use crate::app::AppEvent;
use crate::image_item::{ImageFormat, ImageItem};
use rayon::prelude::*;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::thread;
use walkdir::WalkDir;
use winit::event_loop::EventLoopProxy;

pub fn identify_format(path: &Path) -> Result<ImageFormat, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut buffer = [0; 1024];
    let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
    let data = &buffer[..n];

    // Analyze via magic bytes using the 'infer' crate
    let kind = infer::get(data);
    let mime = kind.map(|k| k.mime_type()).unwrap_or("unknown/raw");

    match mime {
        "image/svg+xml" => Ok(ImageFormat::Svg),
        "image/gif" => Ok(ImageFormat::Gif),
        m if m.starts_with("image/") => Ok(ImageFormat::Static),

        // Manual sniffing for XML/SVG without a standard magic header
        _ => {
            let content = String::from_utf8_lossy(data).to_lowercase();
            if content.contains("<svg") {
                Ok(ImageFormat::Svg)
            } else {
                Err(mime.to_string())
            }
        }
    }
}

pub fn spawn_load_worker(paths: Vec<String>, recursive: bool, proxy: EventLoopProxy<AppEvent>) {
    thread::spawn(move || {
        // Discovery
        let mut files = Vec::new();
        for p in paths {
            if !Path::new(&p).exists() {
                eprintln!("Error: Path '{}' not found", p);
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

        // Sequential Identification
        let tasks: Vec<(PathBuf, ImageFormat)> = files
            .into_iter()
            .filter_map(|path| match identify_format(&path) {
                Ok(format) => Some((path, format)),
                Err(mime) => {
                    eprintln!(
                        "Skipping: Unsupported or mismatched format: {} (File: {:?})",
                        mime, path
                    );
                    None
                }
            })
            .collect();

        // Inform the UI of the total number of valid image slots to create
        let _ = proxy.send_event(AppEvent::InitialCount(tasks.len()));

        // Parallel Decoding
        tasks
            .into_par_iter()
            .enumerate()
            .for_each(|(idx, (path, format))| {
                // Each thread reads its own file data to distribute I/O and RAM usage
                let result = std::fs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|data| ImageItem::from_parts(path, format, data));

                match result {
                    Ok(item) => {
                        let _ = proxy.send_event(AppEvent::ImageLoaded(idx, item));
                    }
                    Err(e) => {
                        eprintln!("Decoding error at index {}: {}", idx, e);
                        let _ = proxy.send_event(AppEvent::ImageLoadFailed(idx, e.to_string()));
                    }
                }
            });

        let _ = proxy.send_event(AppEvent::LoadComplete);
    });
}
