# rsiv

**rsiv** is a lightweight image viewer written in Rust. It aims to be a recreation of `nsxiv` with native Wayland support and a real config.

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

```sh
cargo build --release
```

## Usage

Run `rsiv` by providing one or more image paths or directories as arguments:

```sh
cargo run --release -- <path_to_image_or_directory> [more_paths...]
```

**Example:**

```bash
# Open a single image
cargo run --release -- image.png

# Open all images in a directory
cargo run --release -- ~/Pictures/Wa
```

## Features Roadmap

- [x] Image rendering
- [x] Basic image view modes (zoom, adjust width, adjust height, fit best)
- [x] Basic status bar
- [x] svg support
- [x] Keybinds personalisation
- [ ] command line arguments
- [ ] Other view modes
- [x] UI personalisation
- [x] Thumbnail mode
- [ ] Script handler support (C-x)
- [ ] Mouse support
- [ ] Option handling in config
- [ ] Adding color filter (Gamma and Brightness, ...)
- [ ] Config hot reload
- [ ] Images reload on change
- [ ] Memory usage optimization (options)
- [ ] Add search mode with n/N (n already is mapped tho ?)
