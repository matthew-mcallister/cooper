use std::sync::Arc;

use device::*;
use fehler::throws;
use fnv::FnvHashMap as HashMap;
use gfx::RenderLoop;
use image::GenericImageView;
use log::trace;

use crate::gltf::load_gltf;
use crate::scene::*;
use crate::Error;

#[derive(Debug)]
pub struct AssetCache {
    images: HashMap<String, Arc<ImageDef>>,
    scenes: HashMap<String, SceneCollection>,
    default_normal_map: Arc<ImageDef>,
}

impl AssetCache {
    pub fn new(rloop: &mut RenderLoop) -> Self {
        Self {
            images: Default::default(),
            scenes: Default::default(),
            default_normal_map: create_monochrome_image(rloop, [128, 128, 255, 0]),
        }
    }

    #[throws]
    pub fn get_or_load_image(&mut self, rloop: &mut RenderLoop, path: &str) -> &Arc<ImageDef> {
        if !self.images.contains_key(path) {
            trace!("AssetCache: loading image {}", path);
            let image = image::open(path)?;
            let name = path.to_owned();
            let image = load_image(rloop, image, Some(name.clone()));
            self.images.insert(name, Arc::clone(&image));
        }
        &self.images[path]
    }

    pub fn get_image(&self, path: &str) -> Option<&Arc<ImageDef>> {
        self.images.get(path)
    }

    #[throws]
    pub fn get_or_load_scene(&mut self, rloop: &mut RenderLoop, path: &str) -> &SceneCollection {
        if !self.images.contains_key(path) {
            trace!("AssetCache: loading scene {}", path);
            let scene = load_gltf(rloop, self, path)?;
            self.scenes.insert(path.into(), scene);
        }
        &self.scenes[path]
    }

    pub fn get_scene(&self, path: &str) -> Option<&SceneCollection> {
        self.scenes.get(path)
    }

    pub(crate) fn default_normal_map(&self) -> &Arc<ImageDef> {
        &self.default_normal_map
    }
}

pub(crate) fn load_image(
    rloop: &mut gfx::RenderLoop,
    image: image::DynamicImage,
    name: Option<String>,
) -> Arc<ImageDef> {
    use image::DynamicImage::*;

    let dims = image.dimensions();
    let (format, data) = match image {
        // TODO: Should we convert BGRA to RGBA?
        ImageBgra8(image) => (Format::BGRA8, image.into_raw()),
        image => (Format::RGBA8, image.into_rgba().into_raw()),
    };

    let image = rloop.define_image(
        Default::default(),
        ImageType::Dim2,
        format,
        dims.into(),
        1,
        1,
        name,
    );
    rloop.upload_image(&image, Arc::new(data), 0);

    image
}

pub(crate) fn create_monochrome_image(
    rloop: &mut RenderLoop,
    [r, g, b, a]: [u8; 4],
) -> Arc<ImageDef> {
    let name = format!("color_{}_{}_{}_{}", r, g, b, a);
    let image = rloop.define_image(
        Default::default(),
        ImageType::Dim2,
        Format::RGBA8,
        (1, 1).into(),
        1,
        1,
        Some(name),
    );
    let data = Arc::new(vec![r, g, b, a]);
    rloop.upload_image(&image, data, 0);
    image
}
