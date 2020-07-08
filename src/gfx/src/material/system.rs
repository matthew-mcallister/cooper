use std::sync::Arc;

use crate::{Globals, ImageSubresources, SystemState};
use super::*;

#[derive(Debug)]
crate struct MaterialSystem {
    materials: EnumMap<MaterialProgram, Arc<dyn MaterialFactory>>,
}

impl MaterialSystem {
    crate fn new(_state: &SystemState, globals: &Arc<Globals>) -> Self {
        let [checker, depth, normal] =
            SimpleMaterialFactory::new(_state, globals);
        let materials = unsafe { std::mem::transmute([
             Arc::new(checker),  // Checker
             Arc::new(depth),    // FragDepth
             Arc::new(normal),   // FragNormal
        ]: [Arc<dyn MaterialFactory>; 3]) };
        Self {
            materials,
        }
    }

    crate fn create_material(
        &self,
        system: &SystemState,
        globals: &Globals,
        program: MaterialProgram,
        images: MaterialImageBindings,
    ) -> Arc<Material> {
        let images = create_image_bindings(images);
        let renderer = Arc::clone(&self.materials[program]);
        let desc = renderer.create_descriptor_set(system, globals, &images);
        Arc::new(Material {
            renderer,
            program,
            images,
            desc,
        })
    }
}

fn create_image_bindings(bindings: MaterialImageBindings) -> MaterialImageState
{
    bindings.iter().map(|(name, binding)| {
        // TODO: We currently create a new ImageView for every image,
        // but they could conceivably be cached and shared.
        let ty = image_view_type(
            binding.image.ty(), binding.flags, &binding.subresources);
        let view = unsafe { Arc::new(ImageView::new(
            Arc::clone(&binding.image),
            ty,
            binding.image.format(),
            Default::default(),
            binding.subresources,
        )) };
        (name, ImageBindingState {
            view,
            sampler: Arc::clone(&binding.sampler),
        })
    }).collect()
}

fn image_view_type(
    ty: ImageType,
    flags: ImageBindingFlags,
    subresources: &ImageSubresources,
) -> vk::ImageViewType {
    if flags.contains(ImageBindingFlags::CUBE) {
        if subresources.layer_count() > 6 {
            vk::ImageViewType::CUBE_ARRAY
        } else {
            vk::ImageViewType::CUBE
        }
    } else {
        match (ty, subresources.layer_count()) {
            (ImageType::Dim1, 1) => vk::ImageViewType::_1D,
            (ImageType::Dim1, _) => vk::ImageViewType::_1D_ARRAY,
            (ImageType::Dim2 | ImageType::Cube, 1) =>
                vk::ImageViewType::_2D,
            (ImageType::Dim2 | ImageType::Cube, _) =>
                vk::ImageViewType::_2D_ARRAY,
            (ImageType::Dim3, _) => vk::ImageViewType::_3D,
        }
    }
}
