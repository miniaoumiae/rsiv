# rsiv: Relatively Simple Image Viewer

**rsiv** is a lightweight, high-performance image viewer for Linux. It aims to be a modern, stable, and easily configurable replacement for `nsxiv` (and `sxiv`).

> [!WARNING]
> This project is currently a heavy work in progress.
>
> While `rsiv` mirrors most `nsxiv` keybindings, some are no implemented or differ slightly. See [docs/CONFIGURATION.md](./docs/CONFIGURATION.md) for details.

## Features

- **Thumbnail Mode**: A fast, grid-based view to browse through directories.
- **Format Support**: Supports static images, animated **GIFs/WebPs**, and **SVGs**.
- **Instant Edits**: Image rotations and flips happen instantly without freezing the app.
- **Script Handlers**: Easily run external shell commands on your images.
- **Configuration**: Fully customizable keybindings and UI.
- **Fast Rendering**: Hardware-accelerated drawing for crisp performance.
- **Smart Memory Usage**: Automatically manages its memory based on a percentage of system's RAM.

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
| :---------------------- | :------------------------------------------------- |
| `-q`, `quiet`           | Quiet mode: Suppress warnings and non-fatal errors |
| `-r`, `--recursive`     | Recursively search directories for images.         |
| `-t`, `--thumbnail`     | Start the application in Thumbnail (Grid) mode.    |
| `-o`, `--output-marked` | Print paths of marked files to `stdout` upon exit. |

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
grid_pading = 20

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
- [ ] Config hot reload ?
- [ ] Mouse support ?
- [ ] Color filter (Gamma, Brightness, ...)
- [ ] Other sorting modes (date, size..) `'[', ']'` to switch

## Credits

This is a full reimplementation of **[nsxiv](https://codeberg.org/nsxiv/nsxiv)**. I am grateful to the original maintainers for their work, which served as the foundation for this project.

- **[nnn](https://github.com/jarun/nnn)**, **[zathura](https://pwmt.org/projects/zathura)**: For features inspiration.
- **[image](https://github.com/image-rs/image)**, **[winit](https://github.com/rust-windowing/winit)**, **[pixels](https://github.com/parasyte/pixels)**: The great libraries that do a lot of the heavy lifting.
