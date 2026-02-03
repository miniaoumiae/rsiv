# rsiv: Relatively Simple Image Viewer

**rsiv** is a lightweight, high-performance image viewer for linux. It aims to be a modern replacement for `nsxiv` (and `sxiv`).

> [!WARNING]
> This project is currently a heavy work in progress.
>
> - No memory limiting mechanism is implemented yet. Opening directories with a large volume of images may exceed available RAM and cause a crash.
> - While `rsiv` mirrors most `nsxiv` keybindings, but some are no impemented of differant. You can see [docs/CONFIGURATION.md](./docs/CONFIGURATION.md) for details.

## Features

- **Thumbnail Mode**: A grid-based view to browse directories.
- **Format Support**: Static images, animated **GIFs**, and **SVGs**.
- **Script Handlers**: Execute external commands on images.
- **Configuration**: Fully customizable keybindings and UI in toml.
- **Fast Rendering**: Uses `pixels` (WebGPU) for a software-rendered frame buffer.

## Key differences from `nsxiv`

- Native **wayland** (and mac) support.
- Easy to configure toml file.
- Let you choose if you want to apply the handler to the current file or the marked ones (similar to `nnn`).

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
- [ ] Other view modes
- [x] UI personalisation
- [x] Thumbnail mode
- [ ] Other files options (`%f`) in the handlers ?
- [x] Script handler support (C-x)
- [ ] Mouse support ?
- [x] Configurable options
- [ ] Color filter (Gamma, Brightness, ...)
- [ ] Numeric prefix like `10n`
- [ ] Config hot reload ?
- [ ] Images reload on change
- [ ] Memory usage optimization (`[options]`)
- [ ] Search/Filter mode

## Credits

This is a full reimplementation of **[nsxiv](https://codeberg.org/nsxiv/nsxiv)**. I am grateful to the original maintainers for their work, which served as the foundation for this project.

- **[nnn](https://github.com/jarun/nnn)**, **[zathura](https://pwmt.org/projects/zathura)**: For features inspiration.
- **[image](https://github.com/image-rs/image)**, **[winit](https://github.com/rust-windowing/winit)**, **[pixels](https://github.com/parasyte/pixels)**: The great libraries that do a lot of the heavy lifting.
