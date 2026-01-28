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
                    let mut file_paths: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.path())
                        .collect();
                    
                    // Sort alphabetically
                    file_paths.sort();

                    for file_path in file_paths {
                        // Check if file (not dir) and try to load
                        if file_path.is_file() {
                             // Simple check for extension or try load
                             // We'll just try load. ImageItem::from_path panics on fail currently?
                             // We should change ImageItem to return Result or handle panic.
                             // For now, let's catch_unwind or rely on ImageReader logic inside to specificy.
                             // Actually ImageItem::from_path panics. We should probably fix that for robustness,
                             // but for now let's just use catch_unwind or check extension.
                             
                             // Let's do a simple extension check to avoid spamming errors/panics on non-images
                             if let Some(ext) = file_path.extension() {
                                 let ext_str = ext.to_string_lossy().to_lowercase();
                                 if matches!(ext_str.as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" | "tiff") {
                                      // It panics on fail, so we might crash the thread if we hit a corrupted image.
                                      // To be safe we should wrap.
                                      // But `from_path` implementation in previous turn: `expect("Failed to open image")`.
                                      // Ideally we refactor ImageItem::from_path to return Option/Result.
                                      // I will wrap in catch_unwind for now to be safe against panics in the thread.
                                      
                                      let file_path_str = file_path.to_string_lossy().to_string();
                                      let p = file_path_str.clone();
                                      let result = std::panic::catch_unwind(move || {
                                          ImageItem::from_path(&p)
                                      });
                                      
                                      if let Ok(item) = result {
                                          let _ = proxy.send_event(AppEvent::ImageLoaded(item));
                                      }
                                 }
                             }
                        }
                    }
                }
            } else {
                 // Single file
                 let p = path_str.clone();
                 let result = std::panic::catch_unwind(move || {
                      ImageItem::from_path(&p)
                 });
                 if let Ok(item) = result {
                      let _ = proxy.send_event(AppEvent::ImageLoaded(item));
                 }
            }
        }
    });

    event_loop.run_app(&mut app).unwrap();
}
