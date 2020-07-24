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

fn test_image(rloop: &mut RenderLoop, width: u32, height: u32) -> Arc<ImageDef>
{
    let extent = Extent3D::new(width, height, 1);
    rloop.define_image(
        Default::default(),
        ImageType::Dim2,
        Format::RGBA8,
        extent,
        1,
        1,
        Some(format!("{}x{}", width, height)),
    )
}

unsafe fn upload(mut rloop: Box<RenderLoop>) {
    let data = Arc::new(vec![0u8; 0x2_0000]);
    let images: Vec<_> = (0..7).map(|n| {
        let image = test_image(&mut rloop, 2 << n, 2 << n);
        rloop.upload_image(&image, Arc::clone(&data), 0);
        image
    }).collect();

    for image in images.iter() {
        assert_eq!(rloop.get_image_state(image), ResourceState::Unavailable);
    }

    loop {
        rloop = RenderWorld::new(rloop).render();
        if !rloop.uploader_busy() { break; }
    }

    for image in images.iter() {
        assert_eq!(rloop.get_image_state(image), ResourceState::Available);
    }
}

unit::declare_tests![upload];
