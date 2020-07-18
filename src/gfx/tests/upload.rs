use std::sync::Arc;

use cooper_gfx::*;

mod common;

macro_rules! test_type {
    () => { crate::common::Test }
}

fn main() {
    env_logger::init();
    common::run_tests();
}

unsafe fn upload(mut rloop: Box<RenderLoop>) {
    let data = vec![0; 1024];

    let image = rloop.define_image(
        Default::default(),
        ImageType::Dim2,
        Format::RGBA8,
        (16, 16).into(),
        1,
        1,
    );
    rloop.upload_image(&image, Arc::new(data), 0);

    while rloop.get_image_state(&image) != ResourceState::Available {
        rloop = RenderWorld::new(rloop).render();
    }
}

unit::declare_tests![upload];
