use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use device::{AttachmentImage, Framebuffer, RenderPass};
use more_asserts::assert_le;
use vk::traits::*;

const MAX_VIEWS: usize = 8;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Key {
    render_pass: vk::RenderPass,
    views: [vk::ImageView; MAX_VIEWS],
}

impl Key {
    fn new(render_pass: &RenderPass, attachments: &[AttachmentImage]) -> Self {
        assert_le!(attachments.len(), MAX_VIEWS);
        let mut views = [vk::ImageView::null(); MAX_VIEWS];
        for (attachment, dest) in attachments.iter().zip(views.iter_mut()) {
            *dest = attachment.inner();
        }
        Self {
            render_pass: render_pass.inner(),
            views,
        }
    }
}

// TODO: Use StagedCache instead?
#[derive(Debug, Default)]
pub struct FramebufferCache {
    framebuffers: RwLock<HashMap<Key, Arc<Framebuffer>>>,
}

impl FramebufferCache {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get_or_create(
        &self,
        render_pass: &Arc<RenderPass>,
        attachments: &[AttachmentImage],
    ) -> Arc<Framebuffer> {
        let key = Key::new(render_pass, attachments);
        if let Some(fb) = self.framebuffers.read().unwrap().get(&key) {
            return Arc::clone(fb);
        }
        let mut fbs = self.framebuffers.write().unwrap();
        if let Some(fb) = fbs.get(&key) {
            return Arc::clone(fb);
        } else {
            unsafe {
                let fb = Arc::new(Framebuffer::new(
                    Arc::clone(render_pass),
                    attachments.to_vec(),
                ));
                fbs.insert(key, Arc::clone(&fb));
                fb
            }
        }
    }

    pub fn clear_unused(&mut self) {
        // If there are no external references to the views attached
        // to a framebuffer, we can remove it from the cache since it
        // can no longer be looked up.
        self.framebuffers.get_mut().unwrap().retain(|_, fb| {
            fb.attachments().iter().all(|att| match att {
                AttachmentImage::Image(img) => Arc::strong_count(img) == 1,
                AttachmentImage::Swapchain(img) => Arc::strong_count(img) == 1,
            })
        });
    }
}
