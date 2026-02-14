use crate::app::{App, InputMode};
use crate::config::AppConfig;
use crate::image_item::ImageSlot;

impl App {
    pub fn execute_handler(&mut self, handler_key: &str, on_marked: bool) {
        let config = crate::config::AppConfig::get();

        let cmd_args = match config.handlers.get(handler_key) {
            Some(args) => args.clone(),
            None => return,
        };

        let current_path_str =
            if let ImageSlot::MetadataLoaded(item) = &self.images[self.current_index] {
                item.path.to_string_lossy().into_owned()
            } else {
                String::new()
            };

        let paths: Vec<String> = if on_marked {
            self.marked_files.drain().collect()
        } else {
            if current_path_str.is_empty() {
                vec![]
            } else {
                vec![current_path_str.clone()]
            }
        };

        if paths.is_empty() || cmd_args.is_empty() {
            return;
        }

        let is_bulk = cmd_args.iter().any(|arg| arg.contains("%M"));

        std::thread::spawn(move || {
            if is_bulk {
                let current_path_obj = std::path::Path::new(&current_path_str);
                let mut final_args = Vec::with_capacity(cmd_args.len() + paths.len());

                for arg in &cmd_args {
                    let formatted = format_command_arg(arg, &current_path_str, current_path_obj);

                    if formatted.contains("%M") {
                        for p in &paths {
                            final_args.push(formatted.replace("%M", p));
                        }
                    } else {
                        final_args.push(formatted);
                    }
                }

                if let Some((program, args)) = final_args.split_first() {
                    let _ = std::process::Command::new(program).args(args).status();
                }
            } else {
                for path_str in paths {
                    let path_obj = std::path::Path::new(&path_str);

                    let final_args: Vec<String> = cmd_args
                        .iter()
                        .map(|arg| format_command_arg(arg, &path_str, path_obj))
                        .collect();

                    if let Some((program, args)) = final_args.split_first() {
                        let _ = std::process::Command::new(program).args(args).status();
                    }
                }
            }
        });
    }

    pub fn handle_modal_input(&mut self, key: &str) {
        match &self.input_mode {
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
                    self.input_mode = InputMode::Normal;
                }
            }
            InputMode::AwaitingTarget(handler_key) => {
                let h_key = handler_key.clone();
                match key {
                    "c" => self.execute_handler(&h_key, false),
                    "m" => self.execute_handler(&h_key, true),
                    _ => {}
                }
                self.input_mode = InputMode::Normal;
            }
            InputMode::Normal | InputMode::Filtering => {}
        }
    }
}

fn format_command_arg(arg: &str, path_str: &str, path_obj: &std::path::Path) -> String {
    if !arg.contains('%') {
        return arg.to_string();
    }

    let mut res = String::with_capacity(arg.len() + 32);
    let mut chars = arg.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.peek() {
                Some(&'f') => {
                    res.push_str(path_str);
                    chars.next();
                }
                Some(&'d') => {
                    res.push_str(
                        &path_obj
                            .parent()
                            .unwrap_or(std::path::Path::new(""))
                            .to_string_lossy(),
                    );
                    chars.next();
                }
                Some(&'n') => {
                    res.push_str(&path_obj.file_stem().unwrap_or_default().to_string_lossy());
                    chars.next();
                }
                Some(&'e') => {
                    res.push_str(&path_obj.extension().unwrap_or_default().to_string_lossy());
                    chars.next();
                }
                Some(&'F') => {
                    res.push_str(&path_obj.file_name().unwrap_or_default().to_string_lossy());
                    chars.next();
                }
                Some(&'%') => {
                    res.push('%');
                    chars.next();
                }
                _ => res.push('%'),
            }
        } else {
            res.push(c);
        }
    }
    res
}
