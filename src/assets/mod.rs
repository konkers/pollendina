use std::collections::HashMap;
use std::sync::Arc;

use lazy_static::lazy_static;

pub mod image;

use self::image::{add_image_to_cache, ImageData};

macro_rules! add_image {
    ($c:ident, $id:expr) => {
        add_image_to_cache(
            &mut $c,
            $id,
            include_bytes!(concat!("../../assets/key-items/", $id, ".png")),
        );
    };
}

lazy_static! {
    pub(crate) static ref IMAGES: AssetStore<ImageData> = {
        let mut c = AssetStore::new();
        add_image!(c, "adamant");
        add_image!(c, "baron-key");
        add_image!(c, "crystal");
        add_image!(c, "darkness-crystal");
        add_image!(c, "earth-crystal");
        add_image!(c, "hook");
        add_image!(c, "legend-sword");
        add_image!(c, "luca-key");
        add_image!(c, "magma-key");
        add_image!(c, "package");
        add_image!(c, "pan");
        add_image!(c, "pass");
        add_image!(c, "pink-tail");
        add_image!(c, "rat-tail");
        add_image!(c, "sand-ruby");
        add_image!(c, "spoon");
        add_image!(c, "tower-key");
        add_image!(c, "twin-harp");
        c
    };
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
