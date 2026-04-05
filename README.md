# rsiv: Relatively Simple Image Viewer

**rsiv** is a lightweight, high-performance image viewer for Linux. It aims to be a modern, stable, and easily configurable replacement for `nsxiv` (and `sxiv`).

> [!WARNING]
> While `rsiv` mirrors most `nsxiv` keybindings, some are no implemented or differ slightly. See [docs/CONFIGURATION.md](./docs/CONFIGURATION.md) for details.

## Features

- **Thumbnail Mode**: A fast, grid-based view to browse through directories.
- **Format Support**: Supports static images, animated **GIFs/WebPs**, and **SVGs**.
- **Script Handlers**: Easily run external shell commands on your images.
- **Configuration**: Fully customizable keybindings and UI.

## Key differences from `nsxiv`

- Native **Wayland** (and macOS) support.
- Easy to configure using a `.toml` file, no need to edit C headers and recompile.
- Built-in, real-time fuzzy matching for filtering and finding images quickly.
- Automatically updates the image list when files are added, renamed, or deleted by other programs.
- Choose whether you want to apply handlers to the current file or to marked files (similar to `nnn`).

## Installation

Ensure you have the Rust toolchain installed, then clone and build:

```sh
git clone "https://codeberg.org/miniaoumiae/rsiv"
cd rsiv
cargo install --path .
rsiv --help
```

Make sure to have `~/.cargo/bin` in your path

If you want the `.desktop` too a justfile is provided.

```sh
just install
```

## Usage

Run `rsiv` by providing image paths or directories.

```sh
rsiv [OPTIONS] <PATHS>...
```

### Common Examples

```bash
# Open a single image
rsiv image.png

# Open all images in a directory recursively
rsiv -r ~/Pictures/Wallpapers

# Open directory starting immediately in thumbnail mode
rsiv -t ~/Pictures/

# Pipe marked files to another program
rsiv -o ~/Pictures | xargs -I {} cp {} ~/Selected/
```

### CLI Arguments

| Flag                    | Description                                        |
| ----------------------- | -------------------------------------------------- |
| `-q`, `--quiet`         | Quiet mode: Suppress warnings and non-fatal errors |
| `-r`, `--recursive`     | Recursively search directories for images.         |
| `-H`, `--hidden`        | Include hidden files and directories.              |
| `-d`, `--max-depth <N>` | Maximum recursion depth (requires `-r`).           |
| `-t`, `--thumbnail`     | Start the application in Thumbnail (Grid) mode.    |
| `-o`, `--output-marked` | Print paths of marked files to `stdout` upon exit. |
| `--no-watch`            | Disable filesystem watcher.                        |
| `--no-ipc`              | Disable IPC server.                                |

## Configuration

`rsiv` looks for a config file at `~/.config/rsiv/config.toml` (or `$XDG_CONFIG_HOME`).

**Example `config.toml`:**

```toml
[ui]
bg_color = "#1a1b26"
status_bar_bg = "#24283b"
status_bar_fg = "#c0caf5"
font_family = "JetBrains Mono"
font_size = 12

[options]
default_view = "FitToWindow"
thumbnail_size = 180
grid_padding = 20

[handlers]
# Pressing 'Ctrl+x' then 'g' will open the current image in GIMP
"g" = ["gimp", "%f"]
# Pressing 'Ctrl+x' then 'w' will set the wallpaper using swww
"w" = ["swww", "img", "%f"]
```

> [!NOTE]
> `%f` in handlers is replaced by the absolute path of the image.

For a full explanation of all options, see **[docs/CONFIGURATION.md](./docs/CONFIGURATION.md)**.
You can find the default keybindings there as well.

## IPC (remote control)

`rsiv` exposes a simple IPC interface so you can control a running instance from scripts.
Use the `msg` subcommand to send messages:

```sh
rsiv msg <add|cmd|state> [PAYLOAD] [--target <PID> | --all]
```

See **[docs/IPC.md](./docs/IPC.md)** for details, supported actions, and socket behavior.

## Features Roadmap

- [x] Image rendering
- [x] Basic image view modes (zoom, adjust width, adjust height, fit best)
- [x] Basic status bar
- [x] SVG support
- [x] Keybinds personalisation
- [x] Command line arguments
- [x] Other view modes
- [x] UI personalisation
- [x] Thumbnail mode
- [x] Script handler support (C-x)
- [x] Configurable options
- [x] Numeric prefix like `10n`
- [x] Images reload on change
- [x] Memory usage optimization (`[options]`)
- [x] Search/Filter mode
- [x] Other files options (`%f`) in the handlers ?
- [x] Mouse support ?
- [ ] Config hot reload ?
- [ ] Color filter (Gamma, Brightness, ...)
- [ ] Other sorting modes (date, size..) `'[', ']'` to switch
