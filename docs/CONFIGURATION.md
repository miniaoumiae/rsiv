# rsiv Configuration

## NAME

**rsiv** - TOML configuration file format.

## SYNTAX

rsiv's configuration file uses the **TOML** format. The format's specification can be found at [https://toml.io/en/v1.0.0](https://toml.io/en/v1.0.0).

## LOCATION

rsiv looks for a configuration file in the following locations on UNIX/Linux systems:

1. `$XDG_CONFIG_HOME/rsiv/config.toml`
2. `$HOME/.config/rsiv/config.toml`

If no file is found, internal defaults are used.

## UI

This section documents the `[ui]` table of the configuration file.

**bg_color** = `string`

> The background color of the main canvas (behind the image).
>
> **Default:** `"#000000"`

**status_bar_bg** = `string`

> The background color of the bottom status bar.
>
> **Default:** `"#303030"`

**status_bar_fg** = `string`

> The text color of the status bar.
>
> **Default:** `"#FFFFFF"`

**font_family** = `string`

> The font family used for text in the status bar.
>
> **Default:** `"monospace"`

**font_size** = `integer`

> The font size in points.
>
> **Default:** `13`

**thumbnail_border_color** = `string`

> The color of the border surrounding the currently selected image in Grid/Thumbnail mode.
>
> **Default:** `"#FFFFFF"`

**mark_color** = `string`

> The color of the indicator tag for files that have been "marked" (selected).
>
> **Default:** `"#FF0000"`

**loading_color** = `string`

> The placeholder color used while an image is being decoded in the background.
>
> **Default:** `"#3c3c3c"`

**error_color** = `string`

> The placeholder color used if an image fails to load.
>
> **Default:** `"#FF0000"`

**status_format_left** = `string`

> The format string for the left side of the status bar. See **Status Bar Formatting** below.
>
> **Default:** `"%p"`

**status_format_right** = `string`

> The format string for the right side of the status bar. See **Status Bar Formatting** below.
>
> **Default:** `"%P %s %f %m %z %i"`

### Status Bar Formatting

The status bar strings accept the following tokens:

- **`%p`**: Current file absolute path.
- **`%P`**: The numeric prefix currently being typed (e.g., "10").
- **`%s`**: Slideshow status (e.g., "5s") if active.
- **`%f`**: Frame counter for animations (e.g., "[1/40]"). Hidden for static images.
- **`%z`**: Current zoom level (e.g., "100%").
- **`%i`**: Image index (e.g., "1/50").
- **`%m`**: Mark indicator ("\*") if the file is selected.
- **`%%`**: A literal "%" character.

## OPTIONS

This section documents the `[options]` table of the configuration file.

**default_view** = `"FitToWindow"` | `"BestFit"` | `"FitWidth"` | `"FitHeight"` | `"Absolute"` | `{ Zoom = float }`

> Defines the initial scale mode when opening an image.
>
> - `FitToWindow`: Scales the image to fill the available space (may upscale).
> - `BestFit`: Scales the image to fit, but will not upscale images smaller than the window.
> - `FitWidth`: Fits the image to the window width.
> - `FitHeight`: Fits the image to the window height.
> - `Absolute`: Displays the image at 100% scale (1:1 pixel mapping).
> - `{ Zoom = 2.0 }`: Displays the image at a specific magnification factor (e.g., 2.0 is 200%).
>
> **Default:** `"BestFit"`

**auto_center** = `true` | `false`

> When true, images are automatically centered on the screen when the scale mode is changed or reset.
>
> **Default:** `true`

**clamp_pan** = `true` | `false`

> When true, restricts panning so that the image cannot be moved off-screen.
>
> **Default:** `false`

**thumbnail_size** = `integer`

> The maximum width/height (in pixels) of thumbnails generated in Grid mode.
>
> **Default:** `160`

**grid_padding** = `integer`

> The padding (gap) in pixels between thumbnail cells in Grid mode.
>
> **Default:** `30`

**zoom_max** = `float`

> The maximum zoom level allowed (e.g., 8.0 is 800%).
>
> **Default:** `8.0`

**zoom_min** = `float`

> The minimum zoom level allowed (e.g., 0.1 is 10%).
>
> **Default:** `0.1`

**max_memory_percent** = `float`

> The maximum percentage of total system RAM that the image caches may use.
>
> **Default:** `15.0`

**min_free_memory_percent** = `float`

> The minimum percentage of total system RAM that should remain free before decoding images.
>
> **Default:** `5.0`

## HANDLERS

This section documents the `[handlers]` table. Handlers allow you to execute external commands using the current image path.

To trigger a handler, press the `handler_prefix` key (Default: `Ctrl+x`), followed by the key defined below.

> [!NOTE]: If you have files marked, the status bar will prompt you to choose whether to run the command on the (c)urrent file or all (m)arked files.
> After a handler executes on marked files, the marks are automatically cleared.

**"key"** = `["command", "arg", ...]`

> The key matches a single character input. The value is an array representing the command to run.
> Files are processed sequentially, similar to how `xargs` handles input, unless `%M` is used.
> The following special placeholders are available:
>
> - %f: Absolute path of the file
> - %d: Parent directory of the file
> - %F: File basename
> - %n: File basename without extension
> - %e: File extension
> - %M: Bulk file list. Expands to include all targeted files

> [!NOTE]
> Using %M makes all other placeholders invalid for that command.

**Example:**

```toml
[handlers]
# Open in GIMP
g = ["gimp", "%f"]
# Set wallpaper with aww
w = ["aww", "img", "%f"]
# Convert to PNG in the exact same folder
p = ["magick", "%f", "%d/%n.png"]

# Convert to greyscale
g=["magick", "%f", "-colorspace", "%d/%n_grayscale.%e"]

# Spawn an OS file picker with zenity to choose the zip destination!
Z = [
    "sh",
    "-c",
    "DEST=$(zenity --file-selection --save --title='Save Archive As' --filename='archive.zip'); if [ -n \"$DEST\" ]; then zip -j \"$DEST\" \"$@\"; fi",
    "--",
    "%M"
]
```

## KEYBINDINGS

This section documents the `[keybindings]` table.

Bindings can be a simple `string` or an `array of strings` to assign multiple keys to one action.

**Modifiers:**
Modifiers are specified by adding them before the key, separated by `+`.

- `Ctrl` / `Control`
- `Shift`
- `Alt`
- `Super` / `Meta`

**Example:** `"Ctrl+Shift+f"`

## Hardcoded Bindings

> `Escape`
>
> Cancel any active numeric prefix.
>
> Abort "Waiting for Handler" or Target modes.
>
> Exit Filter Mode and clear the active filter text (if actively typing a filter).

### Navigation and General

**quit** = `string` | `[string]`

> Quit the application.
>
> **Default:** `"q"`

**image_next** = `string` | `[string]`

> Go to the next image.
>
> **Default:** `"n"`

**image_previous** = `string` | `[string]`

> Go to the previous image.
>
> **Default:** `"p"`

**first_image** = `string` | `[string]`

> Jump to the first image.
>
> **Default:** `"g"`

**last_image** = `string` | `[string]`

> Jump to the last image.
>
> **Default:** `"G"`

**next_mark** = `string` | `[string]`

> Jump to the next marked image.
>
> **Default:** `"N"`

**prev_mark** = `string` | `[string]`

> Jump to the previous marked image.
>
> **Default:** `"P"`

**toggle_grid** = `string` | `[string]`

> Switch between Image View and Thumbnail/Grid View.
>
> **Default:** `"Enter"`

**handler_prefix** = `string` | `[string]`

> The prefix key to enter "Handler Mode".
>
> **Default:** `"Ctrl+x"`

**filter_mode** = `string | [string]`

> Enters Fuzzy Filter Mode. While in this mode, a / buffer appears in the status bar. As you type, the image list is filtered in real-time using fuzzy matching
>
> `"Enter"`: Exits Filter Mode but keeps the current filtered results active.
> `"Escape"`: Exits Filter Mode and clears the search, restoring the full image list.
> **Default:** `"/"`

### View Manipulation

**zoom_in** = `string` | `[string]`

> Zoom in by 10%.
>
> **Default:** `"+"`

**zoom_out** = `string` | `[string]`

> Zoom out by 10%.
>
> **Default:** `"-"`

**zoom_reset** = `string` | `[string]`

> Reset zoom to 100% (Absolute).
>
> **Default:** `"="`

**fit_best** = `string` | `[string]`

> Set mode to Fit To Window (scales up to fill).
>
> **Default:** `"f"`

**fit_cover** = `string` | `[string]`

> Set mode to Cover (fill up the available space).
>
> **Default:** `"C"`

**fit_best_no_upscale** = `string` | `[string]`

> Set mode to Best Fit (scales down to fit, never upscales).
>
> **Default:** `"F"`

**fit_width** = `string` | `[string]`

> Fit image to window width.
>
> **Default:** `"W"`

**fit_height** = `string` | `[string]`

> Fit image to window height.
>
> **Default:** `"H"`

**rotate_cw** = `string` | `[string]`

> Rotate image 90 degrees clockwise.
>
> **Default:** `">"`

**rotate_ccw** = `string` | `[string]`

> Rotate image 90 degrees counter-clockwise.
>
> **Default:** `"<"`

**image_flip_horizontal** = `string` | `[string]`

> Flip image horizontally.
>
> **Default:** `"_"`

**image_flip_vertical** = `string` | `[string]`

> Flip image vertically.
>
> **Default:** `"?"`

### Panning and Movement

**view_pan_left** = `string` | `[string]`

> Pan view left (or move cursor left in Grid).
>
> **Default:** `["h", "Left"]`

**view_pan_down** = `string` | `[string]`

> Pan view down (or move cursor down in Grid).
>
> **Default:** `["j", "Dowo"]`

**view_pan_up** = `string` | `[string]`

> Pan view up (or move cursor up in Grid).
>
> **Default:** `["k", "Up"]`

**view_pan_right** = `string` | `[string]`

> Pan view right (or move cursor right in Grid).
>
> **Default:** `["l", "Right"]`

### Toggles and Actions

**toggle_status_bar** = `string` | `[string]`

> Show/Hide the status bar.
>
> **Default:** `"b"`

**toggle_animation** = `string` | `[string]`

> Play/Pause GIF animations.
>
> **Default:** `"Ctrl+a"`

**toggle_slideshow** = `string` | `[string]`

> Start/Stop the slideshow.
>
> **Default:** `"s"`

**next_frame** = `string` | `[string]`

> Advance to the next frame in an animation. Automatically pauses playback.
>
> **Default:** `"."`

**prev_frame** = `string` | `[string]`

> Go back to the previous frame in an animation. Automatically pauses playback.
>
> **Default:** `","`

**toggle_alpha** = `string` | `[string]`

> Toggle visibility of the alpha-channel (transparency). When enabled, a checkerboard pattern is displayed behind transparent areas.
>
> Default: "A"

**mark_file** = `string` | `[string]`

> Toggle the "mark" on the current file.
>
> **Default:** `"m"`

**mark_all** = `string` | `[string]`

> Toggle marks on ALL files (invert selection).
>
> **Default:** `"M"`

**unmark_all** = `string` | `[string]`

> Remove marks from all files.
>
> **Default:** `"u"`

**remove_image** = `string` | `[string]`

> Remove the current image from the view.
>
> **Default:** `"D"`

## CREDITS AND INSPIRATION

The format and style of this configuration documentation is heavily inspired by the excellent documentation of [Alacritty](https://alacritty.org/config-alacritty.html).
