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
use std::io::{self, BufRead, IsTerminal};
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

    /// Quiet mode: Suppress warnings and non-fatal errors
    #[arg(short, long)]
    quiet: bool,

    /// Image paths or directories
    #[arg(required = false)]
    paths: Vec<String>,
}

fn main() {
    let cli = Cli::parse();

    crate::utils::set_quiet_mode(cli.quiet);

    let mut raw_paths = cli.paths.clone();

    if !io::stdin().is_terminal() {
        let stdin = io::stdin();
        let handle = stdin.lock();

        for line in handle.lines() {
            if let Ok(path_str) = line {
                let trimmed = path_str.trim();
                if !trimmed.is_empty() {
                    raw_paths.push(trimmed.to_string());
                }
            }
        }
    }

    let canonical_paths: Vec<String> = raw_paths
        .iter()
        .filter_map(|p| match std::fs::canonicalize(p) {
            Ok(path) => Some(path.to_string_lossy().into_owned()),
            Err(e) => {
                crate::rsiv_warn!("Skipping invalid path '{}': {}", p, e);
                None
            }
        })
        .collect();

    if canonical_paths.is_empty() {
        crate::rsiv_err!("No valid paths provided.");
        return;
    }

    let event_loop = EventLoop::<AppEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();

    let mut app = App::new(vec![], cli.thumbnail, proxy.clone());

    loader::spawn_discovery_worker(canonical_paths.clone(), cli.recursive, proxy.clone());
    watcher::spawn_watcher(canonical_paths, cli.recursive, proxy.clone());

    let _ = event_loop.run_app(&mut app);

    if cli.output_marked {
        for path in &app.marked_files {
            println!("{}", path);
        }
    }
}
