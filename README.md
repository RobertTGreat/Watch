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
- `.m3u` / `.m3u8` playlist loading, CLI startup flags, and dynamic mpv audio output discovery.

Playback is backed by `mpv` and the UI polls mpv IPC in batches for real position, duration, pause, EOF, volume, speed, track-list, and chapter state.

Embedded playback is currently Windows-only.

- [`gpui`](https://www.gpui.rs/)
- [GPUI documentation](https://github.com/zed-industries/zed/tree/main/crates/gpui/docs)
- [GPUI examples](https://github.com/zed-industries/zed/tree/main/crates/gpui/examples)

## Usage

- Ensure Rust is installed - [Rustup](https://rustup.rs/)
- Run your app with `cargo run`
- Build the app quickly with `cargo build --release`.
- Build a distributable installer with `just dist`, or run `cargo build --profile dist` followed by `powershell -NoProfile -ExecutionPolicy Bypass -File installer\build-installer.ps1 -Configuration dist -SkipCargoBuild`.
- The installer registers `Open with Watch` and makes Watch appear as a Windows Default Apps candidate for supported video files under the current Windows user. Windows still requires the user to choose Watch as the default in Settings.
- Optional portable dependencies can be placed under `tools\mpv` and `tools\ffmpeg`; the installer will bundle them when present. Without that, Watch looks for `mpv`, `ffprobe`, and `ffmpeg` on `PATH` and shows a setup message if required playback/probing tools are missing.

CLI flags:

- `--fullscreen` / `--windowed`: override the saved startup fullscreen setting.
- `--resume-ask`, `--resume-always`, `--resume-never`: override resume behavior for this launch.
- `--folder <path>`: load a folder explicitly.

## Hotkeys

- `Space`: play or pause
- `K`: play or pause
- `M`: mute
- `F`: fullscreen
- `N` / `P`: next or previous queue item
- `.` / `,`: frame step forward or backward
- `Shift+A` / `Shift+B` / `Shift+C`: set A-B loop points or clear the loop
- `Shift+Left` / `Shift+Right`: previous or next chapter
- `0`-`9`: jump to 0%-90% of the current media
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
