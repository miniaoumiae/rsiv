use crate::image_item::LoadedImage;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

pub struct CacheManager {
    pub image_cache: LruCache<PathBuf, Arc<LoadedImage>>,
    pub thumb_cache: LruCache<PathBuf, Arc<(u32, u32, Vec<u8>)>>, // w, h, pixels
}

impl CacheManager {
    pub fn new(image_limit: usize, thumb_limit: usize) -> Self {
        Self {
            image_cache: LruCache::new(NonZeroUsize::new(image_limit).unwrap_or(NonZeroUsize::new(8).unwrap())),
            thumb_cache: LruCache::new(NonZeroUsize::new(thumb_limit).unwrap_or(NonZeroUsize::new(200).unwrap())),
        }
    }

    pub fn get_image(&mut self, path: &PathBuf) -> Option<Arc<LoadedImage>> {
        self.image_cache.get(path).cloned()
    }

    pub fn insert_image(&mut self, path: PathBuf, image: Arc<LoadedImage>) {
        self.image_cache.put(path, image);
    }

    pub fn get_thumbnail(&mut self, path: &PathBuf) -> Option<Arc<(u32, u32, Vec<u8>)>> {
        self.thumb_cache.get(path).cloned()
    }

    pub fn insert_thumbnail(&mut self, path: PathBuf, thumb: Arc<(u32, u32, Vec<u8>)>) {
        self.thumb_cache.put(path, thumb);
    }
    
    pub fn clear(&mut self) {
        self.image_cache.clear();
        self.thumb_cache.clear();
    }

    pub fn remove(&mut self, path: &PathBuf) {
        self.image_cache.pop(path);
        self.thumb_cache.pop(path);
    }
}
