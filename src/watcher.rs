use crate::app::AppEvent;
use crate::image_item::ImageItem;
use crate::loader::{identify_format, probe_image};
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode};
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use winit::event_loop::EventLoopProxy;

pub fn spawn_watcher(paths: Vec<String>, recursive: bool, proxy: EventLoopProxy<AppEvent>) {
    thread::spawn(move || {
        let (tx, rx) = mpsc::channel();

        // Waits for the file to finish writing before telling the app.
        let mut debouncer = new_debouncer(Duration::from_millis(100), tx).unwrap();

        for path_str in paths {
            let path = Path::new(&path_str);
            if path.exists() {
                let mode = if recursive {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                };

                if let Err(e) = debouncer.watcher().watch(path, mode) {
                    crate::rsiv_warn!("Watcher error for {:?}: {}", path, e);
                }
            }
        }

        // Listen for events
        for result in rx {
            match result {
                Ok(events) => {
                    for event in events {
                        use notify_debouncer_mini::DebouncedEventKind;

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
                Err(e) => crate::rsiv_warn!("Watch error: {:?}", e),
            }
        }
    });
}

fn handle_change(path: &Path, proxy: &EventLoopProxy<AppEvent>) {
    if path.exists() {
        match identify_format(path) {
            Ok(format) => match probe_image(path, format) {
                Ok((width, height)) => {
                    let item = ImageItem {
                        path: path.to_path_buf(),
                        width,
                        height,
                        format,
                    };
                    let _ = proxy.send_event(AppEvent::FileChanged(item));
                }
                Err(_) => {
                    // Could be a file change that made it invalid, treat as delete/error
                    let _ = proxy.send_event(AppEvent::FileDeleted(path.to_path_buf()));
                }
            },
            Err(_) => {
                // Not a recognized image format
            }
        }
    } else {
        // File Deleted
        let _ = proxy.send_event(AppEvent::FileDeleted(path.to_path_buf()));
    }
}
