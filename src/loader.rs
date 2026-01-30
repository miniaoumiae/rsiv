use crate::app::AppEvent;
use crate::image_item::ImageItem;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use winit::event_loop::EventLoopProxy;

pub fn spawn_load_worker(paths: Vec<String>, proxy: EventLoopProxy<AppEvent>) {
    thread::spawn(move || {
        let mut files_to_decode: Vec<PathBuf> = Vec::new();

        for path_str in paths {
            let path = Path::new(&path_str);
            if path.is_dir() {
                if let Ok(entries) = fs::read_dir(path) {
                    let mut dir_files: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.path())
                        .filter(|p| p.is_file())
                        .collect();
                    dir_files.sort();
                    files_to_decode.extend(dir_files);
                }
            } else {
                files_to_decode.push(path.to_path_buf());
            }
        }

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
