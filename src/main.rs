mod app;
mod cache;
mod config;
mod frame_buffer;
mod image_item;
mod ipc;
mod keybinds;
mod loader;
mod renderer;
mod script_handler;
mod status_bar;
mod utils;
mod view_mode;
mod watcher;

use app::{App, AppEvent};
use clap::{Parser, Subcommand};
use std::io::{self, BufRead, IsTerminal};
use winit::event_loop::EventLoop;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

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

    /// Include hidden files and directories
    #[arg(short = 'H', long)]
    hidden: bool,

    /// Maximum recursion depth
    #[arg(short = 'd', long, requires = "recursive")]
    max_depth: Option<usize>,

    /// Disable filesystem watcher
    #[arg(long)]
    no_watch: bool,

    /// Disable IPC server
    #[arg(long)]
    no_ipc: bool,

    /// Image paths or directories
    #[arg(required = false)]
    paths: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Send a message to running instance(s)
    Msg {
        /// "add" to append a file, "cmd" to execute an Action, or "state" to get the current state
        #[arg(value_name = "TYPE")]
        msg_type: String,

        /// The file path or the Action name (not needed for "state")
        #[arg(value_name = "PAYLOAD", required_if_eq_any([("msg_type", "add"), ("msg_type", "cmd")]))]
        payload: Option<String>,

        /// Target a specific PID (if omitted, targets the most recently opened instance)
        #[arg(short, long)]
        target: Option<u32>,

        /// Broadcast the message to ALL running instances
        #[arg(short, long)]
        all: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    crate::utils::set_quiet_mode(cli.quiet);

    if let Some(Commands::Msg {
        msg_type,
        payload,
        target,
        all,
    }) = &cli.command
    {
        match ipc::send_message(msg_type, payload.as_deref().unwrap_or(""), *target, *all) {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                crate::rsiv_err!("{}", e);
                std::process::exit(1);
            }
        }
    }

    let mut raw_paths = cli.paths.clone();

    if !io::stdin().is_terminal() {
        let stdin = io::stdin();
        let handle = stdin.lock();

        for path_str in handle.lines().map_while(Result::ok) {
            let trimmed = path_str.trim();
            if !trimmed.is_empty() {
                raw_paths.push(trimmed.to_string());
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

    if !cli.no_ipc {
        ipc::spawn_ipc_server(proxy.clone());
    }

    let watcher = if !cli.no_watch {
        watcher::FileWatcher::new(canonical_paths.clone(), cli.recursive, proxy.clone())
    } else {
        None
    };

    let mut app = App::new(vec![], cli.thumbnail, proxy.clone(), watcher);

    loader::spawn_discovery_worker(
        canonical_paths.clone(),
        cli.recursive,
        cli.max_depth,
        cli.hidden,
        proxy.clone(),
    );
    let _ = event_loop.run_app(&mut app);

    if !cli.no_ipc {
        ipc::cleanup_sockets();
    }

    if cli.output_marked {
        for path in &app.gallery.marked_files {
            println!("{}", path);
        }
    }
}
