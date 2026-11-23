# oxishot

oxishot is an app for editing the screenshots you take. Grab a screenshot, open it in oxishot, and make it presentable in a few seconds: point things out with arrows, boxes, and text, blur anything sensitive, then save it or copy it to the clipboard.

The goal is to keep it fast and out of your way. There's no heavy image editor to learn, just the handful of tools you actually reach for when you're about to paste a screenshot into a chat, a doc, or a bug report. The project is in early development, so expect things to change.

## Features

- **Tools:** brush, rectangle, circle, arrow, text, blur, and an eyedropper
- **Editing:** select, move, rotate, and resize anything you've drawn; undo and redo
- **Styling:** stroke and fill colors (or none), stroke width, any installed font with its weights and size, blur strength
- **Output:** save as PNG or copy to the clipboard, then oxishot closes
- **Configurable:** keyboard shortcuts and a default save folder, from the settings dialog or a [plain config file](docs/configuration.md)

Planned: cropping, padding, backgrounds, and rounded corners.

## Install

oxishot is written in Rust. With a [Rust toolchain](https://rustup.rs) installed:

```sh
git clone <repository-url>
cd oxishot
cargo install --path .
```

This puts the `oxishot` binary in `~/.cargo/bin`.

On Linux, the save and folder dialogs go through [xdg-desktop-portal](https://flatpak.github.io/xdg-desktop-portal/), which most desktops already run.

## Usage

Open an image file:

```sh
oxishot screenshot.png
```

Or pipe one in, which is how it fits into a screenshot workflow:

```sh
# Wayland (grim + slurp): select a region and edit it
grim -g "$(slurp)" - | oxishot

# X11 (maim)
maim -s | oxishot
```

Bind one of these to a key in your window manager and you have a screenshot-and-annotate shortcut.

### Options

```
oxishot [OPTIONS] [FILE]

  [FILE]                 Image to open ("-" or omit to read from stdin)
  -i, --input <FILE>     Image to open, "-" for stdin
  -o, --output <FILE>    Where to save the result
  -h, --help             Print help
  -v, --version          Print version
```

`-o` sets the file name and folder the save dialog starts with. If you've turned on "Save to this folder without asking" in the settings, oxishot saves straight to that path instead.

### Shortcuts

| Action           | Default            |
| ---------------- | ------------------ |
| Select tool      | `V`                |
| Brush            | `B`                |
| Text             | `T`                |
| Rectangle        | `R`                |
| Circle           | `O`                |
| Arrow            | `A`                |
| Blur             | `U`                |
| Eyedropper       | `I`                |
| Undo / Redo      | `Ctrl+Z` / `Ctrl+Y` |
| Select all       | `Ctrl+A`           |
| Delete selection | `Delete`, `Backspace` |
| Save             | `Ctrl+S`           |
| Copy and close   | `Ctrl+C`           |

`Escape` deselects and closes popups. Shift-click adds to the selection and Ctrl-click removes from it. Drag a selection's corners to resize it, or just outside them to rotate it.

All of these except `Escape` can be changed. See [Configuration](docs/configuration.md).

## Configuration

Click the gear button in the top-right toolbar to open the settings, or edit `~/.config/oxishot/config.toml` directly. Both are described in [docs/configuration.md](docs/configuration.md).

## Contributing

Bug reports, ideas, and pull requests are welcome. To build and run from source:

```sh
cargo run -- screenshot.png
cargo test
```

The UI is written in [Slint](https://slint.dev) (`src/ui/`); drawing, hit-testing, and rasterizing are plain Rust (`src/editor.rs`, `src/raster.rs`, `src/shapes/`).

## License

oxishot is free software, licensed under the [GNU General Public License v3.0](LICENSE).

It's built with [Slint](https://slint.dev), which oxishot uses under the same license.
