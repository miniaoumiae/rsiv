mod app;
mod frame_buffer;
mod image_item;
mod status_bar;
mod view_mode;

use app::{App, AppEvent};
use image_item::ImageItem;
use std::env;
use std::fs;
use std::path::Path;
use std::thread;
use winit::event_loop::EventLoop;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: rsiv <path_to_image> [path_to_image...]");
        return;
    }

    // Create custom event loop
    let event_loop = EventLoop::<AppEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();

    // Start with empty app
    let mut app = App::new(vec![]);

    // Spawn loading thread
    let paths_to_load = args[1..].to_vec();
    thread::spawn(move || {
        for path_str in paths_to_load {
            let path = Path::new(&path_str);
            if path.is_dir() {
                // Read directory
                if let Ok(entries) = fs::read_dir(path) {
                    let mut file_paths: Vec<_> =
                        entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();

                    // Sort alphabetically
                    file_paths.sort();

                    for file_path in file_paths {
                        // Check if file (not dir) and try to load
                        if file_path.is_file() {
                            let file_path_str = file_path.to_string_lossy().to_string();

                            // Try to load regardless of extension, using ImageReader's guess
                            match ImageItem::from_path(&file_path_str) {
                                Ok(item) => {
                                    let _ = proxy.send_event(AppEvent::ImageLoaded(item));
                                }
                                Err(_) => {
                                    // Ignore files that are not images
                                }
                            }
                        }
                    }
                }
            } else {
                // Single file
                let p = path_str.clone();
                match ImageItem::from_path(&p) {
                    Ok(item) => {
                        let _ = proxy.send_event(AppEvent::ImageLoaded(item));
                    }
                    Err(e) => {
                        eprintln!("Failed to load {}: {}", p, e);
                    }
                }
            }
        }
        // Signal that loading is complete
        let _ = proxy.send_event(AppEvent::LoadComplete);
    });

    event_loop.run_app(&mut app).unwrap();
}

