use crate::image_item::LoadedImage;
use moka::sync::Cache;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use sysinfo::System;

pub struct CacheManager {
    pub image_cache: Cache<PathBuf, Arc<LoadedImage>>,
    pub thumb_cache: Cache<PathBuf, Arc<(u32, u32, Vec<u8>)>>,
    image_limit_kb: u64,
    oversized_images: Mutex<Vec<(PathBuf, Arc<LoadedImage>)>>,
}

impl CacheManager {
    pub fn new(max_memory_percent: f64) -> Self {
        let mut sys = System::new();
        sys.refresh_memory();
        let total_ram_kb = sys.total_memory() / 1024;
        let limit_kb = ((total_ram_kb as f64) * (max_memory_percent / 100.0)) as u64;

        let image_limit_kb = (limit_kb as f64 * 0.8).max(1024.0) as u64;
        let thumb_limit_kb = (limit_kb as f64 * 0.2).max(1024.0) as u64;

        Self {
            image_cache: Cache::builder()
                .max_capacity(image_limit_kb)
                .weigher(|_key, value: &Arc<LoadedImage>| -> u32 { value.size_in_kb() })
                .build(),
            thumb_cache: Cache::builder()
                .max_capacity(thumb_limit_kb)
                .weigher(|_key, value: &Arc<(u32, u32, Vec<u8>)>| -> u32 {
                    ((value.2.len() / 1024) as u32).max(1)
                })
                .build(),
            image_limit_kb,
            oversized_images: Mutex::new(Vec::with_capacity(3)),
        }
    }

    pub fn get_image(&self, path: &PathBuf) -> Option<Arc<LoadedImage>> {
        if let Ok(mut oversized) = self.oversized_images.lock() {
            if let Some(pos) = oversized.iter().position(|(p, _)| p == path) {
                let item = oversized.remove(pos);
                let img = item.1.clone();
                oversized.push(item);
                return Some(img);
            }
        }
        self.image_cache.get(path)
    }

    pub fn insert_image(&self, path: PathBuf, image: Arc<LoadedImage>) {
        let size_kb = image.size_in_kb() as u64;
        let safe_moka_limit = self.image_limit_kb / 64;

        if size_kb > safe_moka_limit {
            if let Ok(mut oversized) = self.oversized_images.lock() {
                oversized.retain(|(p, _)| p != &path);
                oversized.push((path, image));
                if oversized.len() > 3 {
                    oversized.remove(0);
                }
            }
            return;
        }
        self.image_cache.insert(path, image);
    }

    pub fn get_thumbnail(&self, path: &PathBuf) -> Option<Arc<(u32, u32, Vec<u8>)>> {
        self.thumb_cache.get(path)
    }

    pub fn insert_thumbnail(&self, path: PathBuf, thumb: Arc<(u32, u32, Vec<u8>)>) {
        self.thumb_cache.insert(path, thumb);
    }

    pub fn remove(&self, path: &PathBuf) {
        if let Ok(mut oversized) = self.oversized_images.lock() {
            oversized.retain(|(p, _)| p != path);
        }
        self.image_cache.invalidate(path);
        self.thumb_cache.invalidate(path);
    }
}
