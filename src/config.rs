use crate::view_mode::ViewMode;
use serde::de::Deserializer;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct AppConfig {
    pub keybindings: Keybindings,
    pub ui: Ui,
    pub options: Options,
    pub handlers: std::collections::HashMap<String, Vec<String>>,
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
                        Err(e) => crate::rsiv_warn!("Failed to parse config at {:?}: {}", path, e),
                    },
                    Err(e) => crate::rsiv_warn!("Failed to read config at {:?}: {}", path, e),
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

#[derive(Debug, Clone, Default)]
pub struct BindingList(pub Vec<String>);

impl<'de> Deserialize<'de> for BindingList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrVec {
            String(String),
            Vec(Vec<String>),
        }

        match StringOrVec::deserialize(deserializer)? {
            StringOrVec::String(s) => {
                if s.eq_ignore_ascii_case("none") {
                    Ok(BindingList(vec![]))
                } else {
                    Ok(BindingList(vec![s]))
                }
            }
            StringOrVec::Vec(v) => Ok(BindingList(v)),
        }
    }
}

// Helper to construct BindingList
impl<I, S> From<I> for BindingList
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    fn from(iter: I) -> Self {
        BindingList(iter.into_iter().map(|s| s.into()).collect())
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Keybindings {
    pub quit: BindingList,
    pub image_flip_horizontal: BindingList,
    pub image_flip_vertical: BindingList,
    pub image_next: BindingList,
    pub image_previous: BindingList,
    pub rotate_cw: BindingList,
    pub rotate_ccw: BindingList,
    pub zoom_in: BindingList,
    pub zoom_out: BindingList,
    pub zoom_reset: BindingList,
    pub fit_width: BindingList,
    pub fit_height: BindingList,
    pub fit_best: BindingList,
    pub fit_best_no_upscale: BindingList,
    pub fit_cover: BindingList,
    pub view_reset_pan: BindingList,
    pub view_pan_left: BindingList,
    pub view_pan_down: BindingList,
    pub view_pan_up: BindingList,
    pub view_pan_right: BindingList,
    pub view_pan_left_edge: BindingList,
    pub view_pan_right_edge: BindingList,
    pub view_pan_top_edge: BindingList,
    pub view_pan_bottom_edge: BindingList,
    pub grid_page_up: BindingList,
    pub grid_page_down: BindingList,
    pub toggle_status_bar: BindingList,
    pub toggle_animation: BindingList,
    pub toggle_slideshow: BindingList,
    pub toggle_grid: BindingList,
    pub mark_file: BindingList,
    pub unmark_all: BindingList,
    pub remove_image: BindingList,
    pub mark_all: BindingList,
    pub first_image: BindingList,
    pub last_image: BindingList,
    pub next_mark: BindingList,
    pub prev_mark: BindingList,
    pub handler_prefix: BindingList,
    pub filter_mode: BindingList,
    pub toggle_alpha: BindingList,
    pub next_frame: BindingList,
    pub prev_frame: BindingList,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            quit: vec!["q"].into(),
            image_flip_horizontal: vec!["_"].into(),
            image_flip_vertical: vec!["?"].into(),
            image_next: vec!["n"].into(),
            image_previous: vec!["p"].into(),
            rotate_cw: vec![">"].into(),
            rotate_ccw: vec!["<"].into(),
            zoom_in: vec!["+"].into(),
            zoom_out: vec!["-"].into(),
            zoom_reset: vec!["="].into(),
            fit_width: vec!["W"].into(),
            fit_height: vec!["V"].into(),
            fit_best: vec!["f"].into(),
            fit_best_no_upscale: vec!["F"].into(),
            fit_cover: vec!["C"].into(),
            view_reset_pan: vec!["z"].into(),
            view_pan_left: vec!["h", "Left"].into(),
            view_pan_down: vec!["j", "Down"].into(),
            view_pan_up: vec!["k", "Up"].into(),
            view_pan_right: vec!["l", "Right"].into(),
            view_pan_left_edge: vec!["H", "Shift+Left"].into(),
            view_pan_bottom_edge: vec!["J", "Shift+Down"].into(),
            view_pan_top_edge: vec!["K", "Shift+Up"].into(),
            view_pan_right_edge: vec!["L", "Shift+Right"].into(),
            grid_page_up: vec!["Ctrl+u"].into(),
            grid_page_down: vec!["Ctrl+d"].into(),
            toggle_status_bar: vec!["b"].into(),
            toggle_animation: vec!["Ctrl+a"].into(),
            toggle_slideshow: vec!["s"].into(),
            toggle_grid: vec!["Enter"].into(),
            mark_file: vec!["m"].into(),
            unmark_all: vec!["u"].into(),
            remove_image: vec!["D"].into(),
            mark_all: vec!["M"].into(),
            first_image: vec!["g"].into(),
            last_image: vec!["G"].into(),
            next_mark: vec!["N"].into(),
            prev_mark: vec!["P"].into(),
            handler_prefix: vec!["Ctrl+x"].into(),
            filter_mode: vec!["/"].into(),
            toggle_alpha: vec!["A"].into(),
            next_frame: vec!["."].into(),
            prev_frame: vec![","].into(),
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
    pub selected_border_width: u32,
    pub selected_border_padding: u32,
    pub mark_indicator_size: u32,
    pub checkerboard_size: u32,
    pub checkerboard_color_1: String,
    pub checkerboard_color_2: String,
    pub mark_color: String,
    pub loading_color: String,
    pub error_color: String,
    pub status_format_left: String,
    pub status_format_right: String,
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
            selected_border_width: 4,
            selected_border_padding: 1,
            mark_indicator_size: 12,
            checkerboard_size: 16,
            checkerboard_color_1: "#CCCCCC".into(),
            checkerboard_color_2: "#999999".into(),
            mark_color: "#FF0000".into(),
            loading_color: "#3c3c3c".into(),
            error_color: "#FF0000".into(),
            status_format_left: "%p".into(),
            status_format_right: "%P %s %f %m %z %i".into(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Options {
    pub default_view: ViewMode,
    pub auto_center: bool,
    pub clamp_pan: bool,
    pub thumbnail_size: u32,
    pub grid_padding: u32,
    pub zoom_step: f64,
    pub pan_step: i32,
    pub zoom_max: f64,
    pub zoom_min: f64,
    pub max_memory_percent: f64,
    pub min_free_memory_percent: f64,
    pub autoplay_animations: bool,
    pub preload_ahead: usize,
    pub preload_behind: usize,
    pub slideshow_default_delay: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            default_view: ViewMode::FitToWindow,
            auto_center: true,
            clamp_pan: false,
            thumbnail_size: 160,
            grid_padding: 30,
            zoom_step: 1.1,
            pan_step: 50,
            zoom_max: 8.0,
            zoom_min: 0.1,
            max_memory_percent: 15.0,
            min_free_memory_percent: 5.0,
            autoplay_animations: true,
            preload_ahead: 1,
            preload_behind: 1,
            slideshow_default_delay: 5,
        }
    }
}
