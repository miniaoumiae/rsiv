use crate::app::AppEvent;
use crate::image_item::ImageItem;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use winit::event_loop::EventLoopProxy;

pub fn spawn_load_worker(paths: Vec<String>, recursive: bool, proxy: EventLoopProxy<AppEvent>) {
    thread::spawn(move || {
        let mut files_to_decode: Vec<PathBuf> = Vec::new();

        for path_str in paths {
            let path = Path::new(&path_str);
            collect_files(path, recursive, &mut files_to_decode);
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

fn collect_files(path: &Path, recursive: bool, collector: &mut Vec<PathBuf>) {
    if path.is_file() {
        collector.push(path.to_path_buf());
    } else if path.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let entry_path = entry.path();
                if entry_path.is_file() {
                    collector.push(entry_path);
                } else if recursive && entry_path.is_dir() {
                    collect_files(&entry_path, recursive, collector);
                }
            }
        }
    }
}
