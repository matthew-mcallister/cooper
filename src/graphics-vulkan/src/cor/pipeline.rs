use std::ptr;

use crate::*;

#[derive(Debug)]
crate struct PipelineLayout {
    inner: vk::PipelineLayout,
    set_layouts: Vec<String>,
}

impl PipelineLayout {
    crate fn inner(&self) -> vk::PipelineLayout {
        self.inner
    }

    crate fn set_layouts(&self) -> &[String] {
        &self.set_layouts
    }
}

crate unsafe fn create_pipeline_layout(
    core: &CoreData,
    set_layouts: Vec<String>,
) -> PipelineLayout {
    let dt = &*core.device().table;

    let vk_set_layouts: Vec<_> = set_layouts.iter()
        .map(|id| core.get_set_layout(id).inner())
        .collect();
    let create_info = vk::PipelineLayoutCreateInfo {
        set_layout_count: vk_set_layouts.len() as _,
        p_set_layouts: vk_set_layouts.as_ptr(),
        ..Default::default()
    };
    let mut inner = vk::null();
    dt.create_pipeline_layout(&create_info, ptr::null(), &mut inner)
        .check().unwrap();

    PipelineLayout {
        inner,
        set_layouts,
    }
}

#[derive(Debug)]
crate struct GraphicsPipeline {
    crate inner: vk::Pipeline,
    crate layout: String,
    crate pass: String,
    crate subpass: String,
}

impl GraphicsPipeline {
    crate fn inner(&self) -> vk::Pipeline {
        self.inner
    }

    crate fn layout(&self) -> &str {
        &self.layout
    }

    crate fn pass(&self) -> &str {
        &self.pass
    }

    crate fn subpass(&self) -> &str {
        &self.subpass
    }
}
