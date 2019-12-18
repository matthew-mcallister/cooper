
use std::sync::Arc;

use crate::*;

macro_rules! bindings {
    ($(($($binding:tt)*))*) => {
        [$(bindings!(@binding ($($binding)*)),)*]
    };
    (@binding (
        $binding:expr, $type:ident$([$count:expr])? $(, $($flags:ident)+)?)
    ) => {
        #[allow(path_statements)]
        vk::DescriptorSetLayoutBinding {
            binding: $binding,
            descriptor_type: vk::DescriptorType::$type,
            descriptor_count: { 1 $(; $count)? },
            stage_flags: {
                vk::ShaderStageFlags::ALL
                $(; vk::ShaderStageFlags::empty()
                    $(| vk::ShaderStageFlags::$flags)*)?
            },
            ..Default::default()
        }
    };
}

#[derive(Debug)]
crate struct BuiltinSetLayouts {
    crate example_globals: Arc<DescriptorSetLayout>,
    crate example_instances: Arc<DescriptorSetLayout>,
}

impl BuiltinSetLayouts {
    crate fn new(device: &Arc<Device>) -> Self {
        let bindings = bindings! {
            (0, STORAGE_BUFFER[1], ALL)
        };
        let example = unsafe {
            Arc::new(DescriptorSetLayout::from_bindings(
                Arc::clone(device),
                Default::default(),
                &bindings,
            ))
        };

        BuiltinSetLayouts {
            example_globals: Arc::clone(&example),
            example_instances: example,
        }
    }
}

#[cfg(test)]
mod tests {
    use enum_map::enum_map;
    use crate::*;
    use super::*;

    fn smoke_test(vars: testing::TestVars) {
        let device = Arc::clone(&vars.swapchain.device());
        let _layouts = BuiltinSetLayouts::new(&device);
    }

    unit::declare_tests![
        smoke_test,
    ];
}

unit::collect_tests![tests];
