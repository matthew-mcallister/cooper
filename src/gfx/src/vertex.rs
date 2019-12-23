use std::sync::Arc;

use enum_map::{Enum, EnumMap};
use fnv::FnvHashMap;

use crate::*;

/// Describes the vertex attributes present in a mesh and how they are
/// laid out in memory.
#[derive(Debug)]
crate struct VertexLayout {
    crate topology: vk::PrimitiveTopology,
    crate bindings: Vec<VertexLayoutBinding>,
    crate attrs: EnumMap<VertexAttrName, Option<VertexLayoutAttr>>,
}

crate type VertexLayoutBinding = vk::VertexInputBindingDescription;

#[derive(Clone, Copy, Debug, Default)]
crate struct VertexLayoutAttr {
    crate binding: u32,
    crate format: vk::Format,
    crate offset: u32,
}

/// Semantic names for vertex attributes which may be mapped to shader
/// inputs.
#[derive(Clone, Copy, Debug, Enum, Eq, Hash, PartialEq)]
crate enum VertexAttrName {
    Position,
    Normal,
    Tangent,
    QTangent,
    Texcoord0,
    Texcoord1,
    Color,
    Joints,
    Weights,
    Velocity,
}

impl VertexLayout {
    crate fn topology(&self) -> vk::PrimitiveTopology {
        self.topology
    }

    crate fn bindings(&self) -> &[VertexLayoutBinding] {
        &self.bindings
    }

    crate fn attrs(&self) -> &EnumMap<VertexAttrName, Option<VertexLayoutAttr>>
    {
        &self.attrs
    }

    /// Maps vertex attributes to vertex shader inputs. Fails if the
    /// vertex layout and shader input variables mismatch.
    crate fn input_attrs(&self, shader: &Shader) ->
        Result<Vec<vk::VertexInputAttributeDescription>, ()>
    {
        shader.inputs().iter().map(|input| {
            let attr = self.attrs[input.attr.unwrap()].ok_or(())?;
            // TODO: Verify that input type is compatible with format
            Ok(vk::VertexInputAttributeDescription {
                location: input.location,
                binding: attr.binding,
                format: attr.format,
                offset: attr.offset,
            })
        }).collect()
    }

    crate fn from_attrs_unpacked(
        // TODO: Use custom format enum so we can compute size
        tuples: &[(VertexAttrName, vk::Format, u32)],
    ) -> Self {
        let mut attrs = EnumMap::new();
        let mut bindings = Vec::with_capacity(attrs.len());
        for (binding, &(attr, format, stride)) in tuples.iter().enumerate() {
            let binding = binding as u32;
            attrs[attr] = Some(VertexLayoutAttr {
                binding,
                format,
                offset: 0,
            });
            bindings.push(VertexLayoutBinding {
                binding,
                stride,
                ..Default::default()
            });
        }
        Self {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            bindings,
            attrs,
        }
    }
}

#[cfg(test)]
mod tests {
    use enum_map::enum_map;
    use crate::*;
    use super::*;

    unsafe fn create_test_vertex_layouts(_device: &Arc<Device>) ->
        fnv::FnvHashMap<String, Arc<VertexLayout>>
    {
        use VertexAttrName as Attr;

        let mut map = FnvHashMap::default();

        let attrs = &[(Attr::Position, vk::Format::R32G32B32_SFLOAT, 12)];
        let layout = VertexLayout::from_attrs_unpacked(attrs);
        map.insert("simple".to_owned(), Arc::new(layout));

        let attrs = &[
            (Attr::Position,    vk::Format::R32G32B32_SFLOAT,       12  ),
            (Attr::QTangent,    vk::Format::R32G32B32A32_SFLOAT,    16  ),
            (Attr::Texcoord0,   vk::Format::R16G16_UNORM,           4   ),
            (Attr::Color,       vk::Format::R8G8B8_UNORM,           3   ),
            (Attr::Joints,      vk::Format::R8G8B8A8_UINT,          4   ),
            (Attr::Weights,     vk::Format::R8G8B8A8_UNORM,         4   ),
        ];
        let layout = VertexLayout::from_attrs_unpacked(attrs);
        map.insert("full".to_owned(), Arc::new(layout));

        map
    }

    unsafe fn smoke_test(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device);
        let _layouts = create_test_vertex_layouts(&device);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
