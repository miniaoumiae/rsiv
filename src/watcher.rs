use crate::app::AppEvent;
use crate::image_item::ImageItem;
use crate::loader::{identify_format, probe_image};
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use winit::event_loop::EventLoopProxy;

pub fn spawn_watcher(paths: Vec<String>, recursive: bool, proxy: EventLoopProxy<AppEvent>) {
    thread::spawn(move || {
        let (tx, rx) = mpsc::channel();

        // Waits for the file to finish writing before telling the app.
        let mut debouncer = new_debouncer(Duration::from_millis(200), tx).unwrap();

        for path_str in paths {
            let path = Path::new(&path_str);
            if path.exists() {
                let mode = if recursive {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                };

                if let Err(e) = debouncer.watcher().watch(path, mode) {
                    eprintln!("Watcher error for {:?}: {}", path, e);
                }
            }
        }

        // Listen for events
        for result in rx {
            match result {
                Ok(events) => {
                    for event in events {
                        use notify_debouncer_mini::DebouncedEventKind;

                        // Filter out non-image
                        if !is_likely_image(&event.path) {
                            continue;
                        }

                        match event.kind {
                            DebouncedEventKind::Any => {
                                // Fallback/Generic change
                                handle_change(&event.path, &proxy);
                            }
                            DebouncedEventKind::AnyContinuous => {} // Ignore continuous updates
                            _ => {}
                        }
                    }
                }
                Err(e) => eprintln!("Watch error: {:?}", e),
            }
        }
    });
}

fn is_likely_image(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        matches!(
            ext_str.as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" | "tiff"
        )
    } else {
        false
    }
}

fn handle_change(path: &PathBuf, proxy: &EventLoopProxy<AppEvent>) {
    if path.exists() {
        match identify_format(path) {
            Ok(format) => match probe_image(path, format) {
                Ok((width, height)) => {
                    let item = ImageItem {
                        path: path.clone(),
                        width,
                        height,
                        format,
                    };
                    let _ = proxy.send_event(AppEvent::FileChanged(item));
                }
                Err(_) => {
                    // Could be a file change that made it invalid, treat as delete/error
                    let _ = proxy.send_event(AppEvent::FileDeleted(path.clone()));
                }
            },
            Err(_) => {
                // Not a recognized image format
            }
        }
    } else {
        // File Deleted
        let _ = proxy.send_event(AppEvent::FileDeleted(path.clone()));
    }
}
