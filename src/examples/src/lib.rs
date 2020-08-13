#![allow(dead_code)]

use std::thread;

pub unsafe fn with_event_loop<E, F>(f: F)
where
    E: std::fmt::Debug,
    F: FnOnce(window::EventLoopProxy) -> Result<(), E> + Send + 'static,
{
    env_logger::init();

    let (mut ev_loop, ev_proxy) = window::init().unwrap();

    let thread = thread::spawn(move || if let Err(e) = f(ev_proxy) {
        eprintln!("Error: {:?}", e);
    });

    ev_loop.pump();
    thread.join().unwrap();
}
