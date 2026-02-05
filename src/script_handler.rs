use crate::app::{App, InputMode};
use crate::config::AppConfig;
use crate::image_item::{ImageItem, ImageSlot};
use std::path::PathBuf;

impl App {
    pub fn execute_handler(&mut self, handler_key: &str, on_marked: bool) {
        let config = crate::config::AppConfig::get();
        let cmd_args = match config.handlers.get(handler_key) {
            Some(args) => args,
            None => return,
        };

        let paths: Vec<String> = if on_marked {
            self.marked_files.iter().cloned().collect()
        } else if let ImageSlot::MetadataLoaded(item) = &self.images[self.current_index] {
            vec![item.path.to_string_lossy().to_string()]
        } else {
            vec![]
        };

        for path_str in paths {
            if on_marked {
                self.marked_files.remove(&path_str);
            }
            let mut final_args: Vec<String> = cmd_args
                .iter()
                .map(|arg| arg.replace("%f", &path_str))
                .collect();

            if final_args.is_empty() {
                continue;
            }
            let program = final_args.remove(0);

            // Execute synchronously to ensure file availability for refresh
            let _ = std::process::Command::new(program)
                .args(final_args)
                .status();

            self.refresh_specific_path(&path_str);
        }
    }

    pub fn refresh_specific_path(&mut self, path: &str) {
        let path_buf = PathBuf::from(path);

        // Check if file was deleted
        if !path_buf.exists() {
            self.cache.remove(&path_buf);
            self.images.retain(|slot| {
                if let ImageSlot::MetadataLoaded(item) = slot {
                    item.path != path_buf
                } else {
                    true
                }
            });
            // Adjust index if out of bounds
            if self.current_index >= self.images.len() && !self.images.is_empty() {
                self.current_index = self.images.len() - 1;
            }
            return;
        }

        // Re-load from disk (invalidate cache)
        self.cache.remove(&path_buf);

        if let Some(idx) = self.images.iter().position(|s| {
            if let ImageSlot::MetadataLoaded(item) = s {
                item.path == path_buf
            } else {
                false
            }
        }) {
            if let Ok(format) = crate::loader::identify_format(&path_buf) {
                if let Ok((width, height)) = crate::loader::probe_image(&path_buf, format) {
                    let item = ImageItem {
                        path: path_buf.clone(),
                        width,
                        height,
                        format,
                    };
                    self.images[idx] = ImageSlot::MetadataLoaded(item);
                }
            }
        }
    }

    pub fn handle_modal_input(&mut self, key: &str) {
        if key == "Esc" {
            self.input_mode = InputMode::Normal;
            return;
        }

        match self.input_mode.clone() {
            InputMode::WaitingForHandler => {
                let config = AppConfig::get();
                if config.handlers.contains_key(key) {
                    if self.marked_files.is_empty() {
                        self.execute_handler(key, false);
                        self.input_mode = InputMode::Normal;
                    } else {
                        self.input_mode = InputMode::AwaitingTarget(key.to_string());
                    }
                } else {
                    // Invalid handler key cancels the mode
                    self.input_mode = InputMode::Normal;
                }
            }
            InputMode::AwaitingTarget(handler_key) => {
                match key {
                    "c" => self.execute_handler(&handler_key, false),
                    "m" => self.execute_handler(&handler_key, true),
                    _ => {} // Other keys are ignored
                }
                self.input_mode = InputMode::Normal;
            }
            InputMode::Normal => {}
            InputMode::Filtering => {}
        }
    }
}
