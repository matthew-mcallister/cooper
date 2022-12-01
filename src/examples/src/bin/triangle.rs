use winit::event::{Event, WindowEvent};

fn main() {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new().build(&event_loop);

    event_loop.run(move |event, _, control| {
        control.set_poll();
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                control.set_exit();
            },
            _ => {},
        }
    });
}