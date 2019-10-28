use std::thread;

pub(crate) unsafe fn with_event_loop<F>(f: F)
where
    F: FnOnce(window::EventLoopProxy) + Send + 'static
{
    let (mut ev_loop, ev_proxy) = window::init().unwrap();

    let thread = thread::spawn(move || f(ev_proxy));

    ev_loop.pump();
    thread.join().unwrap();
}
