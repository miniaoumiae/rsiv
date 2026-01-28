# rsiv

**rsiv** is a lightweight image viewer written in Rust. It aims to be a recreation of `nsxiv` with native Wayland support and a focus on performance and simplicity.

> [!NOTE]
> This project is currently heavily work in progress.

## Features

- **Native Wayland Support**: Built using `winit` for modern window handling.
- **Image Rendering**: Fast rendering using `pixels` and `wgpu`.
- **View Modes**: Various viewing modes including Zoom, Fit to Window, Fit Width, and Fit Height.
- **Animations**: Support for animated image formats (e.g., GIF).
- **Directory Loading**: Automatically loads all images from a directory.
- **Status Bar**: Informative status bar showing file info and position.

## Installation

Ensure you have Rust and Cargo installed.

```bash
cargo build --release
```

## Usage

Run `rsiv` by providing one or more image paths or directories as arguments:

```bash
cargo run --release -- <path_to_image_or_directory> [more_paths...]
```

**Example:**

```bash
# Open a single image
cargo run --release -- image.png

# Open all images in a directory
cargo run --release -- ~/Pictures/Wa
```

## Controls

| Key           | Action                           |
| ------------- | -------------------------------- |
| `q`           | Quit application                 |
| `n`           | Next image                       |
| `p`           | Previous image                   |
| `h` / `Left`  | Shift image Left                 |
| `l` / `Right` | Shift image Right                |
| `j` / `Down`  | Shift image Down                 |
| `k` / `Up`    | Shift image Up                   |
| `+`           | Zoom In                          |
| `-`           | Zoom Out                         |
| `=`           | Reset Zoom (100%)                |
| `f`           | Fit to Window                    |
| `F`           | Best Fit (Fit but don't upscale) |
| `W`           | Fit Width                        |
| `H`           | Fit Height                       |
| `z`           | Reset Image Position (Center)    |
| `>`           | Rotate Clockwise                 |
| `<`           | Rotate Counter-Clockwise         |
| `b`           | Toggle Status Bar                |
| `Ctrl + a`    | Toggle Animation Playback        |

## Roadmap

- [x] Image rendering
- [x] Basic image view modes (zoom, adjust width, adjust height, fit best)
- [x] Basic status bar
- [ ] Keybinds personalisation
- [ ] UI personalisation
- [ ] Thumbnail mode
- [ ] Script handler support (C-x)

