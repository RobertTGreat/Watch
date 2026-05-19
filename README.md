# Watch

Minimalist OLED dark mode media-player shell built with Rust and GPUI.

It presents a player-first surface with:

- A top-right menu for selecting a real file, folder, or multi-file queue.
- Drag/drop support for files and folders.
- Recursive folder scanning with natural episode sorting.
- Shuffle, repeat, play-next, and recent-folder queue controls.
- A library surface for recent media, continue watching, and unwatched episode candidates.
- Subtitle track, delay, size, color, position, language preference, and search-hook controls.
- Minimal settings for default volume, resume behavior, preferred languages, hardware decoding, and fullscreen startup.
- Controls that hide after a short period without mouse movement.

Playback is backed by `mpv` and the UI polls mpv IPC for real position, duration, pause, EOF, volume, speed, and track-list state.

- [`gpui`](https://www.gpui.rs/)
- [GPUI documentation](https://github.com/zed-industries/zed/tree/main/crates/gpui/docs)
- [GPUI examples](https://github.com/zed-industries/zed/tree/main/crates/gpui/examples)

## Usage

- Ensure Rust is installed - [Rustup](https://rustup.rs/)
- Run your app with `cargo run`
- Build the app and Windows installer with `cargo build --release`
- The release build writes the distributable installer to `dist\WatchSetup.exe` shortly after linking finishes. Packaging details are written to `dist\cargo-release-distribution.log`.
- You can still rebuild the installer directly with `powershell -NoProfile -ExecutionPolicy Bypass -File installer\build-installer.ps1`.
- The installer registers `Open with Watch` and makes Watch appear as a Windows Default Apps candidate for supported video files under the current Windows user. Windows still requires the user to choose Watch as the default in Settings.
- Optional portable dependencies can be placed under `tools\mpv` and `tools\ffmpeg`; the installer will bundle them when present. Without that, Watch looks for `mpv`, `ffprobe`, and `ffmpeg` on `PATH` and shows a setup message if required playback/probing tools are missing.

## Hotkeys

- `Space`: play or pause
- `K`: play or pause
- `J` / `L`: seek backward or forward
- `Left` / `Right`: seek backward or forward
- `Up` / `Down`: volume up or down
- `G` / `H`: decrease or increase subtitle delay
- `Shift+G`: reset subtitle delay
- `-` / `=`: slower or faster playback
- `S`: toggle shuffle
- `R`: cycle repeat mode
- Mouse wheel: volume
- Double-click: fullscreen
