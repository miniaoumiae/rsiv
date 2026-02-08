mod app;
mod cache;
mod config;
mod filtering;
mod frame_buffer;
mod image_item;
mod keybinds;
mod loader;
mod renderer;
mod script_handler;
mod status_bar;
mod utils;
mod view_mode;
mod watcher;

use app::{App, AppEvent};
use clap::Parser;
use winit::event_loop::EventLoop;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Recursive search for images in directories
    #[arg(short, long)]
    recursive: bool,

    /// Start in thumbnail mode
    #[arg(short = 't', long)]
    thumbnail: bool,

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

    let mut app = App::new(vec![], cli.thumbnail, proxy.clone());

    loader::spawn_discovery_worker(cli.paths.clone(), cli.recursive, proxy.clone());
    watcher::spawn_watcher(cli.paths, cli.recursive, proxy.clone());

    let _ = event_loop.run_app(&mut app);

    if cli.output_marked {
        for path in &app.marked_files {
            println!("{}", path);
        }
    }
}
