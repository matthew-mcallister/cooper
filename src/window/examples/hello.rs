use std::thread;
use std::time::Duration;

use cooper_window::*;

fn main() {
    let (mut evt, proxy) = unsafe { init().unwrap() };
    evt.set_poll_interval(Duration::from_millis(5));

    let thd = thread::spawn(move || {
        let config = CreateInfo {
            title: "Hello, world!".to_owned(),
            dims: (320, 200).into(),
            hints: Default::default(),
        };
        let window = proxy.create_window(config).unwrap();
        while !window.should_close() {
            thread::sleep(Duration::from_millis(5));
        }
    });

    evt.pump();
    thd.join().unwrap();
}
