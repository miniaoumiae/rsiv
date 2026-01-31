use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct AppConfig {
    pub keybindings: Keybindings,
    pub ui: Ui,
    pub options: Options,
}

impl AppConfig {
    pub fn get() -> &'static AppConfig {
        CONFIG.get_or_init(Self::load)
    }

    fn load() -> Self {
        let config_path = Self::find_config_path();

        if let Some(path) = config_path {
            if path.exists() {
                match fs::read_to_string(&path) {
                    Ok(contents) => match toml::from_str(&contents) {
                        Ok(config) => return config,
                        Err(e) => eprintln!("Failed to parse config at {:?}: {}", path, e),
                    },
                    Err(e) => eprintln!("Failed to read config at {:?}: {}", path, e),
                }
            }
        }

        Self::default()
    }

    fn find_config_path() -> Option<PathBuf> {
        // Check XDG_CONFIG_HOME first
        if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
            let path = PathBuf::from(xdg_config).join("rsiv/config.toml");
            return Some(path);
        }

        // Fallback to ~/.config/rsiv/config.toml
        if let Ok(home) = env::var("HOME") {
            let path = PathBuf::from(home).join(".config/rsiv/config.toml");
            return Some(path);
        }

        None
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            keybindings: Keybindings::default(),
            ui: Ui::default(),
            options: Options::default(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Keybindings {
    pub quit: Vec<String>,
    pub image_flip_horizontal: Vec<String>,
    pub image_flip_vertical: Vec<String>,
    pub image_next: Vec<String>,
    pub image_previous: Vec<String>,
    pub rotate_cw: Vec<String>,
    pub rotate_ccw: Vec<String>,
    pub zoom_in: Vec<String>,
    pub zoom_out: Vec<String>,
    pub zoom_reset: Vec<String>,
    pub fit_width: Vec<String>,
    pub fit_height: Vec<String>,
    pub fit_best: Vec<String>,
    pub fit_best_no_upscale: Vec<String>,
    pub view_reset_pan: Vec<String>,
    pub view_pan_left: Vec<String>,
    pub view_pan_down: Vec<String>,
    pub view_pan_up: Vec<String>,
    pub view_pan_right: Vec<String>,
    pub toggle_status_bar: Vec<String>,
    pub toggle_animation: Vec<String>,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            quit: vec!["q".into()],
            image_flip_horizontal: vec!["_".into()],
            image_flip_vertical: vec!["?".into()],
            image_next: vec!["n".into()],
            image_previous: vec!["p".into()],
            rotate_cw: vec![">".into()],
            rotate_ccw: vec!["<".into()],
            zoom_in: vec!["+".into()],
            zoom_out: vec!["-".into()],
            zoom_reset: vec!["=".into()],
            fit_width: vec!["W".into()],
            fit_height: vec!["H".into()],
            fit_best: vec!["f".into()],
            fit_best_no_upscale: vec!["F".into()],
            view_reset_pan: vec!["z".into()],
            view_pan_left: vec!["h".into(), "Left".into()],
            view_pan_down: vec!["j".into(), "Down".into()],
            view_pan_up: vec!["k".into(), "Up".into()],
            view_pan_right: vec!["l".into(), "Right".into()],
            toggle_status_bar: vec!["b".into()],
            toggle_animation: vec!["Ctrl+a".into()],
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Ui {
    pub bg_color: String,
    pub status_bar_bg: String,
    pub status_bar_fg: String,
    pub font_family: String,
    pub font_size: u8,
    pub thumbnail_border_color: String,
    pub mark_color: String,
    pub loading_color: String,
    pub error_color: String,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            bg_color: "#000000".into(),
            status_bar_bg: "#303030".into(),
            status_bar_fg: "#FFFFFF".into(),
            font_family: "monospace".into(),
            font_size: 13,
            thumbnail_border_color: "#FFFFFF".into(),
            mark_color: "#FF0000".into(),
            loading_color: "#3c3c3c".into(),
            error_color: "#FF0000".into(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Options {
    pub auto_center: bool,
    pub pan_limit: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            auto_center: false,
            pan_limit: false,
        }
    }
}
