#![deny(unsafe_op_in_unsafe_fn)]

use std::path::Path;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;

use device::{AppInfo, Device};
use engine::Engine;
use winit::window::Window;

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Close,
}

/// Implements the main loop of an app.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Tinker {
    window: Window,
    engine: Engine,
    receiver: Receiver<Event>,
    start_time: std::time::Instant,
    tick: u64,
    graphics_queue: Arc<device::Queue>,
    transfer_queue: Arc<device::Queue>,
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
    fn new(window: Window, engine: Engine, receiver: Receiver<Event>) -> Self {
        let queue = Arc::clone(&engine.queues()[0][0]);
        Self {
            window,
            receiver,
            start_time: std::time::Instant::now(),
            tick: 0,
            graphics_queue: Arc::clone(&queue),
            transfer_queue: queue,
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

    pub fn graphics_queue(&self) -> &Arc<device::Queue> {
        &self.graphics_queue
    }

    pub fn transfer_queue(&self) -> &Arc<device::Queue> {
        &self.transfer_queue
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
        self.engine.acquire_next_image().unwrap();
    }

    pub fn present(&mut self) {
        self.engine.present(&[&mut self.backbuffer_semaphore]);
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

    pub fn start_time(&self) -> std::time::Instant {
        self.start_time
    }

    pub fn elapsed_time(&self) -> f32 {
        (std::time::Instant::now() - self.start_time).as_secs_f32()
    }

    pub fn aspect_ratio(&self) -> f32 {
        let winit::dpi::PhysicalSize { width, height } = self.window.inner_size();
        width as f32 / height as f32
    }

    pub fn perspective(&self, z_near: f32, z_far: f32, fov_y_deg: f32) -> math::Matrix4 {
        let tan_y = fov_y_deg.to_radians().tan();
        let tan_x = tan_y * self.aspect_ratio();
        math::Matrix4::perspective(z_near, z_far, tan_x, tan_y)
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

    let mut engine = Engine::from_window(A::app_info(), &window, Default::default()).unwrap();
    engine.load_shaders_from_dir(shader_dir).unwrap();

    let (sender, receiver) = channel();
    let mut tinker = Tinker::new(window, engine, receiver);
    let mut app = A::init(&mut tinker);
    let mut j = Some(thread::spawn(move || loop {
        unsafe { tinker.new_frame() };
        let cmds = app.frame(&mut tinker);
        tinker.submit_commands(&cmds[..]);
        tinker.present();
        if let Some(Event::Close) = tinker.poll() {
            tinker.device().wait_idle();
            std::mem::drop(app);
            break;
        }
    }));

    event_loop.run(move |event, _, control_flow| {
        // Basically only need to poll in case the other thread dies.
        control_flow.set_wait_timeout(std::time::Duration::from_millis(20));
        if j.as_ref().map_or(true, |j| j.is_finished()) {
            control_flow.set_exit();
            return;
        }
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
