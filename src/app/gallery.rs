use crate::cache::CacheManager;
use crate::image_item::{ImageSlot, LoadedAsset};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};
use std::path::PathBuf;
use std::sync::Arc;
use winit::window::Window;

pub struct Gallery {
    pub all: Vec<ImageSlot>,
    pub filtered: Vec<ImageSlot>,
    pub current_index: usize,
    pub filter_text: String,
    pub marked_files: std::collections::HashSet<String>,
    pub discovery_complete: bool,
}

impl Gallery {
    pub fn mutate_current_image<F>(&mut self, cache: &mut CacheManager, f: F) -> bool
    where
        F: FnOnce(&mut LoadedAsset) -> bool,
    {
        let Some(ImageSlot::MetadataLoaded(item)) = self.filtered.get_mut(self.current_index)
        else {
            return false;
        };

        let path = item.path.clone();
        if let Some(mut loaded_asset) = cache.get_image(&path) {
            cache.remove(&path);
            let inner = Arc::make_mut(&mut loaded_asset);

            let dimensions_changed = f(inner);

            if dimensions_changed {
                match inner {
                    LoadedAsset::Raster { width, height, .. } => {
                        item.width = *width;
                        item.height = *height;
                    }
                    LoadedAsset::Vector {
                        base_width,
                        base_height,
                        ..
                    } => {
                        item.width = *base_width;
                        item.height = *base_height;
                    }
                }
            }

            cache.insert_image(path, loaded_asset);
            return true;
        }
        false
    }

    pub fn is_path_visible(
        &self,
        path: &PathBuf,
        window: Option<&Arc<Window>>,
        grid_mode: bool,
    ) -> bool {
        if !grid_mode {
            if let ImageSlot::MetadataLoaded(item) = &self.filtered[self.current_index] {
                return &item.path == path;
            }
            return false;
        }

        if let Some(w) = window {
            let config = crate::config::AppConfig::get();
            let cell_size = config.options.thumbnail_size + config.options.grid_padding;
            let buf_w = w.inner_size().width;
            let buf_h = w.inner_size().height;
            let cols = (buf_w / cell_size).max(1);

            let current_row = (self.current_index as u32) / cols;
            let scroll_y = if current_row * cell_size > buf_h / 2 {
                (current_row * cell_size) as i32 - (buf_h as i32 / 2) + (cell_size as i32 / 2)
            } else {
                0
            };

            let start_row = scroll_y.max(0) as u32 / cell_size;
            let rows_visible = (buf_h / cell_size) + 2;

            let start_idx = (start_row * cols) as usize;
            let end_idx = ((start_row + rows_visible) * cols) as usize;
            let end_idx = end_idx.min(self.filtered.len());

            for i in start_idx..end_idx {
                if let ImageSlot::MetadataLoaded(item) = &self.filtered[i] {
                    if &item.path == path {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn apply_filter(&mut self) {
        if self.filter_text.is_empty() {
            self.filtered = self.all.clone();
            return;
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(
            &self.filter_text,
            CaseMatching::Ignore,
            Normalization::Smart,
        );

        let mut buf = Vec::new();

        let mut scored_matches: Vec<(u32, ImageSlot)> = self
            .all
            .iter()
            .filter_map(|slot| {
                if let ImageSlot::MetadataLoaded(item) = slot {
                    let path_str = item.path.to_string_lossy();
                    let haystack = Utf32Str::new(&path_str, &mut buf);

                    pattern
                        .score(haystack, &mut matcher)
                        .map(|score| (score, slot.clone()))
                } else {
                    None
                }
            })
            .collect();

        scored_matches.sort_by(|a, b| b.0.cmp(&a.0));

        self.filtered = scored_matches.into_iter().map(|(_, slot)| slot).collect();
        self.current_index = 0;
    }
}
