#[derive(Debug)]
pub struct Config {
    pub width: u32,
    pub height: u32,
}

impl Config {
    pub fn extent(&self) -> vk::Extent2D {
        vk::Extent2D {
            width: self.width,
            height: self.height,
        }
    }

    pub fn viewport(&self) -> vk::Viewport {
        vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: self.width as _,
            height: self.height as _,
            min_depth: 1.0,
            max_depth: 0.0,
        }
    }

    pub fn view_rect(&self) -> vk::Rect2D {
        vk::Rect2D {
            offset: vk::Offset2D::new(0, 0),
            extent: self.extent(),
        }
    }
}
