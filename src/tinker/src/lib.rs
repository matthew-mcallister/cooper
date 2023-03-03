#![deny(unsafe_op_in_unsafe_fn)]

use std::path::Path;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;

use device::{AppInfo, Device};
use engine::Engine;

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Close,
}

/// Implements the main loop of an app.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Tinker {
    engine: Engine,
    receiver: Receiver<Event>,
    tick: u64,
    graphics_queue: Arc<device::Queue>,
    transfer_queue: Arc<device::Queue>,
    sw_index: u32,
    // Semaphore which presentation waits on
    backbuffer_semaphore: device::BinarySemaphore,
    // Single semaphore for keeping track of rendering completion
    frame_semaphore: device::TimelineSemaphore,
}

pub trait App: Send + 'static {
    fn app_info() -> AppInfo;

    fn init(tinker: &mut Tinker) -> Self;

    fn frame(&mut self, tinker: &mut Tinker) -> Vec<vk::CommandBuffer>;
}

impl Tinker {
    fn new(engine: Engine, receiver: Receiver<Event>) -> Self {
        let queue = Arc::clone(&engine.queues()[0][0]);
        Self {
            receiver,
            tick: 0,
            graphics_queue: Arc::clone(&queue),
            transfer_queue: queue,
            sw_index: 0,
            backbuffer_semaphore: device::BinarySemaphore::new(engine.device_ref()),
            frame_semaphore: device::TimelineSemaphore::new(engine.device_ref(), 0),
            engine,
        }
    }

    pub fn device(&self) -> &Arc<Device> {
        self.engine.device()
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn engine_mut(&mut self) -> &Engine {
        &mut self.engine
    }

    pub fn swapchain_image(&self) -> &Arc<device::SwapchainView> {
        &self.engine.swapchain().views()[self.sw_index as usize]
    }

    pub fn graphics_queue(&self) -> &Arc<device::Queue> {
        &self.graphics_queue
    }

    pub fn transfer_queue(&self) -> &Arc<device::Queue> {
        &&self.transfer_queue
    }

    pub fn poll(&self) -> Option<Event> {
        match self.receiver.try_recv() {
            Ok(e) => Some(e),
            Err(TryRecvError::Disconnected) => Some(Event::Close),
            Err(TryRecvError::Empty) => None,
        }
    }

    pub unsafe fn new_frame(&mut self) {
        self.frame_semaphore.wait(self.tick, 50_000_000).unwrap();
        self.tick += 1;
        self.engine.new_frame();
        unsafe { self.engine.reclaim_transient_resources() };
        self.sw_index = self.engine.acquire_next_image().unwrap();
    }

    pub unsafe fn present(&mut self) {
        unsafe {
            self.graphics_queue.present(
                &[&mut self.backbuffer_semaphore],
                self.engine.swapchain_mut(),
                self.sw_index,
            );
        }
    }

    pub fn submit_commands(&mut self, commands: &[vk::CommandBuffer]) {
        unsafe {
            self.graphics_queue.submit(&[device::SubmitInfo {
                wait_sems: &[device::WaitInfo {
                    semaphore: self.engine.acquire_semaphore_mut().inner_mut(),
                    value: 0,
                    stages: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT_BIT,
                }],
                sig_sems: &[
                    device::SignalInfo {
                        semaphore: self.backbuffer_semaphore.inner_mut(),
                        value: 0,
                    },
                    device::SignalInfo {
                        semaphore: self.frame_semaphore.inner_mut(),
                        value: self.tick,
                    },
                ],
                cmds: commands,
            }]);
        }
    }
}

pub fn run_app<A: App>(shader_dir: &Path) {
    env_logger::init();

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_inner_size(winit::dpi::LogicalSize {
            width: 1600,
            height: 900,
        })
        .with_resizable(false)
        .build(&event_loop)
        .unwrap();

    let mut engine = Engine::from_window(A::app_info(), &window).unwrap();
    engine.load_shaders_from_dir(shader_dir).unwrap();

    let (sender, receiver) = channel();
    let mut tinker = Tinker::new(engine, receiver);
    let mut app = A::init(&mut tinker);
    let mut j = Some(thread::spawn(move || loop {
        unsafe { tinker.new_frame() };
        let cmds = app.frame(&mut tinker);
        tinker.submit_commands(&cmds[..]);
        unsafe { tinker.present() };
        if let Some(Event::Close) = tinker.poll() {
            tinker.device().wait_idle();
            break;
        }
    }));

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_poll();
        match event {
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                ..
            } => {
                let _ = sender.send(Event::Close);
                let _ = j.take().unwrap().join();
                control_flow.set_exit();
            }
            _ => {}
        }
    });
}
