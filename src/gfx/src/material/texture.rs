use std::sync::Arc;

use base::partial_map;
use device as dev;

use crate::{Globals, ShaderConst, SystemState};
use crate::util::SmallVec;
use super::*;

/// Texture visualization materials.
#[derive(Debug)]
pub(super) struct TextureVisMaterialFactory {
    globals: Arc<Globals>,
    layout: Arc<dev::DescriptorSetLayout>,
    vert_shader: Arc<dev::ShaderSpec>,
    frag_shader: Arc<dev::ShaderSpec>,
}

impl TextureVisMaterialFactory {
    pub(super) fn new(
        state: &SystemState,
        globals: &Arc<Globals>,
        slot: MaterialImage,
    ) -> Self {
        let count = MaterialImage::SIZE as u32;
        let layout = state.set_layouts.get_or_create(&dev::set_layout_desc![
            (0, CombinedImageSampler[count], FRAGMENT_BIT),
        ]).into_owned();

        // TODO: This could easily be made into a macro. Or a function
        // taking an iterator. Or, better yet, ShaderSpec could just
        // accept a hashmap as input.
        let specialize = |shader| {
            let mut spec = dev::ShaderSpec::new(Arc::clone(shader));
            spec.set(ShaderConst::TextureVisSlot as _, &(slot as u32));
            Arc::new(spec)
        };

        Self {
            globals: Arc::clone(globals),
            layout,
            vert_shader: specialize(&globals.shaders.static_vert),
            frag_shader: specialize(&globals.shaders.texture_vis_frag),
        }
    }
}

impl MaterialFactory for TextureVisMaterialFactory {
    fn process_image_bindings(&self, images: &mut MaterialImageBindings) {
        for k in MaterialImage::values() {
            if images.contains_key(k) { continue; }

            let image = Arc::clone(&self.globals.empty_image_2d);
            images.insert(k, ImageBindingDesc {
                subresources: image.all_subresources(),
                image,
                flags: Default::default(),
                sampler: Arc::clone(&self.globals.empty_sampler),
            });
        }
    }

    fn create_descriptor_set(
        &self,
        state: &SystemState,
        images: &MaterialImageState,
    ) -> Option<dev::DescriptorSet> {
        let mut set =
            state.descriptors.alloc(dev::Lifetime::Static, &self.layout);
        let views: SmallVec<_, {MaterialImage::SIZE}> =
            images.values().map(|img| &img.view).collect();
        let samplers: SmallVec<_, {MaterialImage::SIZE}> =
            images.values().map(|img| &img.sampler).collect();
        unsafe {
            set.write_images(
                0, 0,
                &views,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                Some(&samplers),
            );
        }
        Some(set)
    }

    fn select_shaders(&self) -> dev::ShaderStageMap {
        partial_map! {
            dev::ShaderStage::Vertex => Arc::clone(&self.vert_shader),
            dev::ShaderStage::Fragment => Arc::clone(&self.frag_shader),
        }
    }
}
