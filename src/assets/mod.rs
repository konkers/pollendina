use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread_local;

pub mod image;

pub(crate) use self::image::{add_image_to_cache, ImageData};

thread_local! {
    pub(crate) static IMAGES: RefCell<AssetStore<ImageData>> = RefCell::new(AssetStore::new());
}

pub(crate) struct AssetStore<T> {
    assets: HashMap<String, Arc<T>>,
}

impl<T> AssetStore<T> {
    pub fn new() -> AssetStore<T> {
        AssetStore {
            assets: HashMap::new(),
        }
    }

    pub fn add(&mut self, key: &String, asset: T) {
        self.assets.insert(key.clone(), Arc::new(asset));
    }

    pub fn get(&self, key: &String) -> Option<Arc<T>> {
        match self.assets.get(key) {
            None => None,
            Some(a) => Some(a.clone()),
        }
    }
}
