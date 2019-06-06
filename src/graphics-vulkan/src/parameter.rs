//! A shader parameter set connects resources, samplers, and attachments
//! to a pipeline. It is a generalization of a "material", as compute
//! pipeline inputs, image lights, and particle effects are parameter
//! sets, in addition to materials. The name "parameter set" is also
//! fitting because parameter sets are often in one-to-one
//! correspondence with descriptor sets.
// TODO: Need a way to free parameters at the same time as the
// underlying resources.
use std::ptr;

use slab::Slab;

use crate::*;

macro_rules! decl_id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd,
        )]
        pub struct $name {
            idx: usize,
        }
        impl $name {
            fn new(idx: usize) -> Self {
                $name { idx }
            }
        }
    }
}

decl_id!(MaterialId);

#[derive(Clone, Copy, Debug)]
pub struct SampledImage {
    pub image: ImageId,
    pub sampler: SamplerId,
}

// TODO: Additional material types
#[derive(Clone, Copy, Debug)]
crate struct Material {
    crate albedo: SampledImage,
    crate normal: SampledImage,
    crate metro: SampledImage,
    crate desc: vk::DescriptorSet,
}

#[derive(Clone, Copy, Debug)]
pub struct MaterialCreateInfo {
    pub albedo: SampledImage,
    pub normal: SampledImage,
    pub metro: SampledImage,
}

impl Material {
    unsafe fn destroy(self, params: &mut ParameterStorage) {
        params.material_descs.free(self.desc);
    }
}

#[derive(Debug)]
pub struct ParameterStorage {
    materials: Slab<Material>,
    material_descs: DescriptorSetPool,
}

impl ParameterStorage {
    pub fn new(sys: &System) -> Self {
        let dt = Arc::clone(&sys.dt);
        let pbr_mat_layout = sys.set_layouts["pbr_material"].obj;
        ParameterStorage {
            materials: Slab::new(),
            material_descs: DescriptorSetPool::new(dt, pbr_mat_layout),
        }
    }

    pub fn create_material(
        &mut self,
        sys: &System,
        create_info: &MaterialCreateInfo,
    ) -> MaterialId {
        let albedo_img = sys.resources.get(create_info.albedo.image);
        let albedo_sampler = sys.samplers.get(create_info.albedo.sampler);
        let normal_img = sys.resources.get(create_info.normal.image);
        let normal_sampler = sys.samplers.get(create_info.normal.sampler);
        let metro_img = sys.resources.get(create_info.metro.image);
        let metro_sampler = sys.samplers.get(create_info.metro.sampler);

        let desc = self.material_descs.allocate();
        let img_infos = [
            vk::DescriptorImageInfo {
                sampler: albedo_sampler.sampler,
                image_view: albedo_image.view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            },
            vk::DescriptorImageInfo {
                sampler: normal_sampler.sampler,
                image_view: normal_image.view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            },
            vk::DescriptorImageInfo {
                sampler: metro_sampler.sampler,
                image_view: metro_image.view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            },
        ];
        let write = vk::WriteDescriptorSet {
            dst_set: desc,
            dst_binding: 0,
            descriptor_count: 3,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            p_image_info: img_infos.as_ptr(),
            ..Default::default()
        };
        sys.dt.update_descriptor_sets(1, &write as _, 0, ptr::null());

        MaterialId::new(self.materials.insert(Material {
            albedo: create_info.albedo,
            normal: create_info.normal,
            metro: create_info.metro,
            desc,
        }))
    }

    pub fn destroy_material(&mut self, material: MaterialId) {
        let mat = self.materials.remove(material.idx);
        mat.destroy(self);
    }
}
