# IPC (Inter-Process Communication)

`rsiv` exposes a lightweight IPC interface over Unix domain sockets. This lets you
control a running instance (e.g., add a file, trigger an action, or query state)
from scripts or other programs.

## Socket location and naming

- Directory: `$XDG_RUNTIME_DIR/rsiv` (falls back to `/tmp/rsiv`).
- Per-instance socket: `rsiv-<PID>.sock`
- Latest instance symlink: `rsiv-latest.sock`

The socket directory is created with `0700` permissions to keep access limited to
the current user.

## CLI interface

Use the `msg` subcommand:

```sh
rsiv msg <add|cmd|state> [PAYLOAD] [--target <PID> | --all]
```

### Message types

#### `add` — append a file

Adds a file to the current gallery (path is canonicalized). If the file is already
present, the request is acknowledged without changes.

```sh
rsiv msg add /path/to/image.png
```

#### `cmd` — run an action

Executes one of the built-in actions:

- `NextImage`
- `PrevImage`
- `ToggleGrid`
- `ToggleSlideshow`
- `ToggleStatusBar`
- `Quit`

```sh
rsiv msg cmd NextImage
```

#### `state` — query current state

Returns a short string describing the current view:

- Current image path when an image is loaded
- `No images` when the gallery is empty
- `Loading...` when metadata is still loading

```sh
rsiv msg state
```

## Target selection

- Default: sends to the most recently opened instance via `rsiv-latest.sock`.
- `--target <PID>`: sends to a specific instance.
- `--all`: broadcasts to all detected sockets in the runtime directory.

Examples:

```sh
# Target a specific instance
rsiv msg cmd ToggleGrid --target 12345

# Broadcast to all running instances
rsiv msg cmd ToggleStatusBar --all
```

## Protocol notes

- Requests and responses are JSON-encoded and framed with a 4-byte little-endian
  length prefix.
- Payloads are limited to 1 MiB.
- Responses are `Ack`, `State(<string>)`, or `Error(<string>)`.

## Cleanup behavior

On startup, the instance removes any stale socket for its PID. On exit, it removes
its socket and clears `rsiv-latest.sock` if it points to that instance. If a targeted
socket cannot be reached, the client removes it and reports an error.
