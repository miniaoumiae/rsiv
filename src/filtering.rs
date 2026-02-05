use crate::app::App;
use crate::image_item::ImageSlot;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};

impl App {
    pub fn apply_filter(&mut self) {
        if self.filter_text.is_empty() {
            self.images = self.all_images.clone();
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
            .all_images
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

        self.images = scored_matches.into_iter().map(|(_, slot)| slot).collect();

        self.current_index = 0;
    }
}
