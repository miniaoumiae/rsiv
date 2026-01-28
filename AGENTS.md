# AGENTS.md

## Project Overview

**rsiv** is a high-performance, minimalist image viewer written in Rust. It draws inspiration from `nsxiv` but leverages modern Rust tooling.

- **Core Stack:** Rust, `winit` (Windowing), `pixels` (2D hardware-accelerated pixel buffer), `image` (Decoding), `embedded-graphics` (UI/Text).
- **Architecture:** The application uses a custom event loop to handle background image loading and window events. Rendering is performed via software rasterization (nearest-neighbor scaling) into a GPU-backed pixel buffer (`pixels` crate).

## Features

- **Format Support:** Supports common formats (PNG, JPG, GIF, BMP, WEBP, etc.) and animated GIFs.
- **Navigation:** Directory scanning, next/previous image navigation, and panning.
- **View Modes:** Smart scaling modes (Fit, Best Fit, Fit Width/Height) and Zoom.
- **UI:** Toggleable status bar with metadata.
- **Performance:** Threaded image loading to prevent UI blocking.

## Controls / Keybindings

| Key | Action |
| :--- | :--- |
| **Navigation** | |
| `n` | Next image |
| `p` | Previous image |
| `h`, `j`, `k`, `l` / Arrows | Pan image (Left, Down, Up, Right) |
| **View Modes** | |
| `f` | **Fit to Window**: Scale image to fit entirely within view |
| `F` | **Best Fit**: Fit to window, but do not upscale (max 100%) |
| `W` | **Fit Width**: Scale to match window width |
| `H` | **Fit Height**: Scale to match window height |
| `=` | **Absolute**: 1:1 scale (100%) |
| `+` | Zoom In (1.1x) |
| `-` | Zoom Out (1/1.1x) |
| `z` | **Reset View**: Center image (keeps current scale/mode) |
| **Playback** | |
| `Ctrl + a` | Toggle Animation Playback (Play/Pause) |
| **UI / System** | |
| `b` | Toggle Status Bar visibility |
| `q` / `Esc` | Quit |

## Technical Architecture

### 1. Rendering Pipeline
- **Buffer:** The `pixels` buffer is initialized to the **Window Dimensions**.
- **Scaling:** Unlike typical `pixels` usage where the buffer matches the image size, we intentionally match the window size. Scaling is performed in software using a custom **Nearest-Neighbor** implementation in `App::render`.
  - *Reasoning:* This allows precise control over pixel placement, crisp scaling for pixel art, and unified logic for panning/zooming without reallocating buffers constantly.
- **Animation:** Animated GIFs are decoded into a `Vec<FrameData>`. The render loop tracks `Instant::now` and updates the frame index based on the specific delay of the current frame.

### 2. Threading Model
- **Main Thread:** Handles Window events (`winit`) and Rendering.
- **Loader Thread:** A dedicated background thread scans directories and loads images.
  - Images are sent to the main loop via `AppEvent::ImageLoaded(ImageItem)`.
  - This ensures the UI remains responsive even when loading large folders or network drives.

### 3. UI System
- **Status Bar:** Implemented using `embedded-graphics` directly onto the frame buffer.
- **Layout:** The render logic calculates `available_h` (Window Height - Status Bar Height). Images are centered and scaled within this available area, ensuring the status bar never overlaps content.

## Setup & Build

```bash
# Install dependencies
cargo fetch

# Run with arguments (files or directories)
cargo run --release -- path/to/image.png path/to/folder/
```

## Code Structure

- `src/main.rs`: Entry point, argument parsing, thread spawning, and event loop initialization.
- `src/app.rs`: Core application logic, event handling, rendering loop, and input processing.
- `src/image_item.rs`: Image loading, format detection, and frame storage (`FrameData`).
- `src/view_mode.rs`: Enum defining viewing strategies (`FitToWindow`, `BestFit`, `Zoom`, etc.).
- `src/status_bar.rs`: Rendering logic for the bottom status bar.
- `src/frame_buffer.rs`: Wrapper to adapt the raw `[u8]` pixel buffer for `embedded-graphics`.
