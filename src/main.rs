mod app;
mod config;
mod frame_buffer;
mod image_item;
mod loader;
mod renderer;
mod status_bar;
mod utils;
mod view_mode;

use app::{App, AppEvent};
use winit::event_loop::EventLoop;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: rsiv <path_to_image> [path_to_image...]");
        return;
    }

    let event_loop = EventLoop::<AppEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();
    let mut app = App::new(vec![]);

    let paths_to_load = args[1..].to_vec();
    loader::spawn_load_worker(paths_to_load, proxy);

    event_loop.run_app(&mut app).unwrap();
}
