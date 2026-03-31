use crate::app::AppEvent;
use crate::image_item::ImageItem;
use crate::loader::{identify_format, probe_image};
use notify_debouncer_mini::{
    new_debouncer,
    notify::{RecursiveMode, Watcher},
};
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use winit::event_loop::EventLoopProxy;

pub struct FileWatcher {
    debouncer:
        Arc<Mutex<notify_debouncer_mini::Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>>>,
    recursive: bool,
}

impl FileWatcher {
    pub fn new(paths: Vec<String>, recursive: bool, proxy: EventLoopProxy<AppEvent>) -> Option<Self> {
        let (tx, rx) = mpsc::channel();

        // Waits for the file to finish writing before telling the app.
        let mut debouncer = match new_debouncer(Duration::from_millis(100), tx) {
            Ok(d) => d,
            Err(e) => {
                crate::rsiv_warn!("Failed to initialize watcher: {}", e);
                return None;
            }
        };

        let mode = if recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        for path_str in paths {
            let path = Path::new(&path_str);
            if path.exists() {
                if let Err(e) = debouncer.watcher().watch(path, mode) {
                    crate::rsiv_warn!("Watcher error for {:?}: {}", path, e);
                }
            }
        }

        // Listen for events
        thread::spawn(move || {
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

        Some(Self {
            debouncer: Arc::new(Mutex::new(debouncer)),
            recursive,
        })
    }

    pub fn watch(&self, path: &Path) {
        let mode = if self.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        if let Ok(mut lock) = self.debouncer.lock() {
            if let Err(e) = lock.watcher().watch(path, mode) {
                crate::rsiv_warn!("Failed to watch added file {:?}: {}", path, e);
            }
        }
    }
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
