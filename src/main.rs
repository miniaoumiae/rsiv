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
use clap::Parser;
use winit::event_loop::EventLoop;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Recursive search for images in directories
    #[arg(short, long)]
    recursive: bool,

    /// Output marked files to stdout on exit
    #[arg(short, long)]
    output_marked: bool,

    /// Image paths or directories
    #[arg(required = true)]
    paths: Vec<String>,
}

fn main() {
    let cli = Cli::parse();

    let event_loop = EventLoop::<AppEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();

    // We pass cli.output_marked to App just in case it needs to know,
    // but primarily we check the app state after the loop finishes.
    let mut app = App::new(vec![]);

    loader::spawn_load_worker(cli.paths, cli.recursive, proxy);

    let _ = event_loop.run_app(&mut app);

    if cli.output_marked {
        for path in &app.marked_files {
            println!("{}", path);
        }
    }
}
