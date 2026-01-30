use crate::app::AppEvent;
use crate::image_item::ImageItem;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::thread;
use walkdir::WalkDir;
use winit::event_loop::EventLoopProxy;

pub fn spawn_load_worker(paths: Vec<String>, recursive: bool, proxy: EventLoopProxy<AppEvent>) {
    thread::spawn(move || {
        let mut files_to_decode: Vec<PathBuf> = Vec::new();

        for path_str in paths {
            let path = Path::new(&path_str);
            let mut walker = WalkDir::new(path);
            if !recursive {
                walker = walker.max_depth(1);
            }

            for entry in walker.into_iter().filter_map(|e| e.ok()) {
                if entry.path().is_file() {
                    files_to_decode.push(entry.path().to_path_buf());
                }
            }
        }

        files_to_decode.sort();

        files_to_decode.into_par_iter().for_each(|path| {
            let path_str = path.to_string_lossy().to_string();

            match ImageItem::from_path(&path_str) {
                Ok(item) => {
                    let _ = proxy.send_event(AppEvent::ImageLoaded(item));
                }
                Err(e) => {
                    eprintln!("Error loading {:?}: {}", path, e);
                }
            }
        });

        // Signal completion
        let _ = proxy.send_event(AppEvent::LoadComplete);
    });
}
