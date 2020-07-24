use std::sync::Arc;

use cooper_gfx as gfx;

macro_rules! test_type {
    () => { gfx::testing::IntegrationTest }
}

fn main() {
    env_logger::init();
    gfx::testing::run_tests(crate::__collect_tests);
}

fn test_image(rloop: &mut gfx::RenderLoop, width: u32, height: u32) ->
    Arc<gfx::ImageDef>
{
    let extent = gfx::Extent3D::new(width, height, 1);
    rloop.define_image(
        Default::default(),
        gfx::ImageType::Dim2,
        gfx::Format::RGBA8,
        extent,
        1,
        1,
        Some(format!("{}x{}", width, height)),
    )
}

unsafe fn upload(mut rloop: Box<gfx::RenderLoop>) {
    use gfx::ResourceState::{Available, Unavailable};

    let data = Arc::new(vec![0u8; 0x2_0000]);
    let images: Vec<_> = (0..7).map(|n| {
        let image = test_image(&mut rloop, 2 << n, 2 << n);
        rloop.upload_image(&image, Arc::clone(&data), 0);
        image
    }).collect();

    for image in images.iter() {
        assert_eq!(rloop.get_image_state(image), Unavailable);
    }

    loop {
        rloop = gfx::RenderWorld::new(rloop).render();
        if !rloop.uploader_busy() { break; }
    }

    for image in images.iter() {
        assert_eq!(rloop.get_image_state(image), Available);
    }
}

unit::declare_tests![upload];
