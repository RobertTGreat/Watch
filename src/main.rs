#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod allanime;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    ffi::OsStr,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    ops::Range,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use gpui::{
    actions, canvas, div, ease_in_out, fill, img, linear_color_stop, linear_gradient, point,
    prelude::*, px, relative, rgb, rgba, size, svg, Animation, AnimationExt, AnyElement, App,
    Bounds, ClipboardItem, Context, CursorStyle, Div, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, ExternalPaths, FocusHandle, Focusable, GlobalElementId, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, PaintQuad,
    PathPromptOptions, Pixels, Point, Render, ScrollDelta, ScrollHandle, ScrollWheelEvent,
    ShapedLine, SharedString, Style, TextRun, TitlebarOptions, Transformation, UTF16Selection,
    UnderlineStyle, Window, WindowBackgroundAppearance, WindowBounds, WindowOptions,
};
use gpui_platform::application;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW_FLAG: u32 = 0x08000000;
#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::HWND,
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_SHOWWINDOW,
        SW_HIDE, SW_SHOW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    },
};

const OLED_BLACK: u32 = 0x000000;
const APP_NAME: &str = "Watch";
const APP_ID: &str = "Watch";
const WATCH_PLAYER_KEY_CONTEXT: &str = "WatchPlayer";
const WATCH_DIALOG_KEY_CONTEXT: &str = "WatchDialog";
const PLAYER_BLACK: u32 = 0x030303;
const MENU_BLACK: u32 = 0x080808;
const SOFT_WHITE: u32 = 0xf5f5f5;
const MUTED_TEXT: u32 = 0x9a9a9a;
const FINE_BORDER: u32 = 0x101010;
const BRIGHT_BORDER: u32 = 0x202020;
const VLC_ORANGE: u32 = 0xff8a2a;
const BACKDROP_BLUR_BASE_BACKGROUND_ALPHA: f32 = 0.82;
const BACKDROP_BLUR_VIDEO_SURFACE_ALPHA: f32 = 0.88;
const BACKDROP_BLUR_LIBRARY_BACKGROUND_ALPHA: f32 = 0.78;
const BACKDROP_BLUR_MENU_BACKGROUND_ALPHA: f32 = 0.82;
const BACKDROP_BLUR_MODAL_BACKDROP_ALPHA: f32 = 0.54;
const BACKDROP_BLUR_MODAL_BACKGROUND_ALPHA: f32 = 0.70;
const BACKDROP_BLUR_FLOATING_CONTROL_ALPHA: f32 = 0.70;
const CONTINUE_REMOVE_HOVER_SCALE: f32 = 1.36;
const CONTINUE_REMOVE_SCALE_ANIMATION_MS: u64 = 140;
const LIBRARY_ICON_HOVER_SCALE: f32 = 1.22;
const LIBRARY_ICON_SCALE_ANIMATION_MS: u64 = 140;
const CONTROLS_HIDE_DELAY_MS: u64 = 500;
const CONTROLS_REVEAL_THROTTLE_MS: u128 = 250;
const PLAYBACK_STATE_POLL_MS: u64 = 250;
const OSD_HIDE_DELAY_MS: u64 = 1200;
const TIMELINE_THUMBNAIL_INTERVAL_SECONDS: u64 = 5;
const TIMELINE_HORIZONTAL_PADDING_PX: f32 = 16.0;
const TIMELINE_TIME_LABEL_WIDTH_PX: f32 = 64.0;
const TIMELINE_LABEL_GAP_PX: f32 = 16.0;
const VIDEO_DOUBLE_CLICK_TOP_GUARD_PX: f32 = 112.0;
const VIDEO_DOUBLE_CLICK_BOTTOM_GUARD_PX: f32 = 154.0;
const LIVE_CAPTURE_AUDIO_BUFFER_MS: u16 = 20;
const LIVE_CAPTURE_LAVF_BUFFER_SIZE_BYTES: usize = 4096;
const LIVE_CAPTURE_PROBE_SIZE_BYTES: usize = 32;
const LIVE_CAPTURE_RTBUF_SIZE: &str = "32M";
const LIVE_CAPTURE_STREAM_BUFFER_SIZE: &str = "4k";
const DIRECTSHOW_INTERNAL_AUDIO_PIN_NAME: &str = "Audio";
const AUDIO_OUTPUT_AUTO_DEVICE_ID: &str = "auto";
const LIVE_CAPTURE_AUDIO_SOURCE_AUTO: &str = "auto";
const LIVE_CAPTURE_AUDIO_SOURCE_NONE: &str = "none";
const LIBRARY_THUMBNAIL_POSITION_SECONDS: f64 = 45.0;
const LIBRARY_HORIZONTAL_MARGIN_PX: f32 = 96.0;
const LIBRARY_MAX_CONTENT_WIDTH_PX: f32 = 3200.0;
const LIBRARY_PREFERRED_CARD_WIDTH_PX: f32 = 230.0;
const LIBRARY_MIN_CARD_WIDTH_PX: f32 = 150.0;
const LIBRARY_MAX_CARD_WIDTH_PX: f32 = 280.0;
const LIBRARY_CARD_GAP_PX: f32 = 12.0;
const LIBRARY_MIN_VISIBLE_CARD_COUNT: f32 = 2.0;
const LIBRARY_MAX_VISIBLE_CARD_COUNT: f32 = 12.0;
const LIBRARY_BASE_VIEWPORT_WIDTH_PX: f32 = 1500.0;
const LIBRARY_BASE_VIEWPORT_HEIGHT_PX: f32 = 920.0;
const LIBRARY_MIN_UI_SCALE: f32 = 1.0;
const LIBRARY_MAX_UI_SCALE: f32 = 2.35;
const MAX_LIBRARY_THUMBNAILS_TO_GENERATE: usize = 12;
const REMOTE_THUMBNAIL_DOWNLOAD_TIMEOUT_SECONDS: u64 = 15;
const LIBRARY_TITLE_LINE_HEIGHT_PX: f32 = 18.0;
const LIBRARY_TITLE_AVERAGE_CHARACTER_WIDTH_PX: f32 = 7.2;
const LIBRARY_TITLE_SCROLL_END_PADDING_PX: f32 = 24.0;
const VOLUME_SLIDER_WIDTH: f32 = 142.0;
const MAIN_MENU_WIDTH: f32 = 320.0;
const CONTEXT_MENU_WIDTH: f32 = 300.0;
const LIBRARY_CONTEXT_MENU_WIDTH: f32 = 220.0;
const LIBRARY_CONTEXT_MENU_ESTIMATED_HEIGHT: f32 = 48.0;
const SOURCE_SEARCH_BAR_WIDTH: f32 = 760.0;
const SOURCE_SEARCH_RESULT_MAX_HEIGHT: f32 = 340.0;
const SOURCE_PROVIDER_SETTINGS_MAX_HEIGHT: f32 = 132.0;
const ALLANIME_PROVIDER_ID: &str = "builtin-allanime";
const ALLANIME_PROVIDER_NAME: &str = "AllAnime";
const ALLANIME_SOURCE_URL_PREFIX: &str = "allanime://";
const LIVE_CAPTURE_DROPDOWN_WIDTH: f32 = 360.0;
const LIBRARY_ACTION_GROUP_ESTIMATED_WIDTH: f32 = 448.0;
const LIBRARY_LIVE_CAPTURE_OVERLAY_TOP: f32 = 74.0;
const MENU_RIGHT_MARGIN: f32 = 16.0;
const MAIN_MENU_CLICKOFF_SAFE_COLUMN_WIDTH: f32 = MAIN_MENU_WIDTH + (MENU_RIGHT_MARGIN * 2.0);
const CONTEXT_MENU_OFFSET: f32 = 8.0;
const CONTEXT_MENU_ESTIMATED_HEIGHT: f32 = 390.0;
const QUEUE_LIST_MAX_HEIGHT: f32 = 260.0;
const CONTINUE_WATCHING_PROMPT_WIDTH: f32 = 340.0;
const MINIMUM_RESUME_POSITION_SECONDS: f64 = 1.0;
const COMPLETED_MEDIA_REMAINING_SECONDS: f64 = 90.0;
const COMPLETED_MEDIA_FRACTION: f64 = 0.92;
const DEFAULT_SEEK_STEP_SECONDS: f64 = 5.0;
const DEFAULT_VOLUME_STEP_PERCENT: i16 = 5;
const SPEED_STEP: f64 = 0.1;
const DEFAULT_VOLUME_PERCENT: u8 = 64;
const MAX_RECENT_MEDIA: usize = 24;
const MAX_RECENT_FOLDERS: usize = 12;
const MAX_LIBRARY_ITEMS_PER_SHELF: usize = 48;
const THUMBNAIL_PREVIEW_WIDTH: f32 = 180.0;
const THUMBNAIL_PREVIEW_HEIGHT: f32 = 102.0;
const WATCH_SESSION_DIRECTORY_NAME: &str = "Watch";
const WATCH_SESSION_FILE_NAME: &str = "watch-session.json";
const WATCH_SETTINGS_FILE_NAME: &str = "watch-settings.json";
const WATCH_LIBRARY_FILE_NAME: &str = "watch-library.json";
const THUMBNAIL_CACHE_DIRECTORY_NAME: &str = "timeline-thumbnails";
const LIBRARY_FLUSH_INTERVAL_MS: u128 = 5_000;
const THUMBNAIL_CACHE_MAX_BYTES: u64 = 512 * 1024 * 1024;
const THUMBNAIL_WORKER_COUNT: usize = 2;
const REMOTE_THUMBNAIL_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";
const ICON_PLAY: &str = "play.svg";
const ICON_PAUSE: &str = "pause.svg";
const ICON_NEXT: &str = "next.svg";
const ICON_PREVIOUS: &str = "previous.svg";
const ICON_STOP: &str = "stop.svg";
const ICON_MAXIMIZE: &str = "maximize.svg";
const ICON_MINIMIZE: &str = "minimize.svg";
const ICON_VOLUME: &str = "volume.svg";
const ICON_VOLUME_MUTED: &str = "volume-x.svg";
const ICON_SHUFFLE: &str = "shuffle.svg";
const ICON_REPEAT: &str = "repeat.svg";
const ICON_SETTINGS: &str = "settings.svg";
const ICON_SEARCH: &str = "search.svg";
const ICON_GLOBE: &str = "globe.svg";
const ICON_CHEVRON_LEFT: &str = "chevron-left.svg";
const ICON_CHEVRON_RIGHT: &str = "chevron-right.svg";
const ICON_CHEVRON_UP: &str = "chevron-up.svg";
const ICON_CHEVRON_DOWN: &str = "chevron-down.svg";
const ICON_EYE: &str = "eye.svg";
const ICON_X: &str = "x.svg";
const ICON_FILE: &str = "file.svg";
const ICON_FOLDER: &str = "folder.svg";

const VIDEO_EXTENSIONS: [&str; 20] = [
    "mkv", "mp4", "mov", "m4v", "3gp", "avi", "wmv", "asf", "ogm", "ogg", "flv", "webm", "mxf",
    "mpeg", "mpg", "m2ts", "ts", "vob", "divx", "dv",
];
const SUBTITLE_EXTENSIONS: [&str; 13] = [
    "srt", "ass", "ssa", "sub", "idx", "vtt", "smi", "sami", "txt", "usf", "mpl", "mpsub", "jss",
];
const PLAYLIST_EXTENSIONS: [&str; 2] = ["m3u", "m3u8"];
const MPV_POLL_PROPERTIES: &[&str] = &[
    "time-pos",
    "duration",
    "pause",
    "eof-reached",
    "volume",
    "mute",
    "speed",
    "aid",
    "sid",
    "track-list",
    "chapter-list",
    "chapter",
];

actions!(
    watch_player,
    [
        TogglePlayback,
        ToggleMute,
        ToggleFullscreen,
        PreviousQueueItem,
        NextQueueItem,
        FrameStepBackward,
        FrameStepForward,
        SetABLoopA,
        SetABLoopB,
        ClearABLoop,
        SeekPreviousChapter,
        SeekNextChapter,
        JumpToPercent0,
        JumpToPercent1,
        JumpToPercent2,
        JumpToPercent3,
        JumpToPercent4,
        JumpToPercent5,
        JumpToPercent6,
        JumpToPercent7,
        JumpToPercent8,
        JumpToPercent9,
        IncreaseSubtitleDelay,
        DecreaseSubtitleDelay,
        ResetSubtitleDelay,
        SeekBackward,
        SeekForward,
        VolumeUp,
        VolumeDown,
        IncreasePlaybackSpeed,
        DecreasePlaybackSpeed,
        ToggleShuffle,
        CycleRepeatMode
    ]
);

actions!(
    source_text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy
    ]
);

actions!(
    source_browser,
    [
        SubmitSourceSearch,
        SelectPreviousSourceResult,
        SelectNextSourceResult
    ]
);

#[derive(Clone)]
struct AudioOutputDeviceOption {
    label: String,
    device_id: String,
}

#[derive(Clone)]
struct LiveCaptureDevice {
    display_name: String,
    backend_name: String,
    audio_backend_name: Option<String>,
    audio_pin_name: Option<String>,
    latency_mode: LiveCaptureLatencyMode,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LiveCaptureLatencyMode {
    UltraLow,
    Stable,
}

#[derive(Clone)]
struct LiveCaptureAudioDevice {
    display_name: String,
    backend_name: String,
}

struct LiveCaptureDeviceScan {
    video_devices: Vec<LiveCaptureDevice>,
    audio_devices: Vec<LiveCaptureAudioDevice>,
}

#[derive(Clone, Serialize, Deserialize)]
struct SourceProvider {
    id: String,
    name: String,
    search_url_template: String,
    #[serde(default)]
    episodes_url_template: Option<String>,
    #[serde(default)]
    streams_url_template: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct InternetMediaHttpHeader {
    name: String,
    value: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct InternetMedia {
    provider_id: String,
    provider_name: String,
    title: String,
    #[serde(default)]
    series_title: Option<String>,
    #[serde(default)]
    episode_title: Option<String>,
    #[serde(default)]
    episode_number: Option<String>,
    subtitle: Option<String>,
    stream_url: String,
    thumbnail_url: Option<String>,
    #[serde(default)]
    http_headers: Vec<InternetMediaHttpHeader>,
}

#[derive(Clone)]
struct SourceSearchResult {
    provider: SourceProvider,
    item_id: Option<String>,
    title: String,
    subtitle: Option<String>,
    episodes_url: Option<String>,
    streams_url: Option<String>,
    direct_media: Option<InternetMedia>,
    thumbnail_url: Option<String>,
}

#[derive(Clone)]
struct SourceEpisodeResult {
    provider: SourceProvider,
    series_title: String,
    item_id: Option<String>,
    title: String,
    subtitle: Option<String>,
    streams_url: Option<String>,
    direct_media: Option<InternetMedia>,
}

#[derive(Clone)]
struct SourceStreamResult {
    media: InternetMedia,
    quality: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceBrowserView {
    SearchResults,
    Episodes,
    Streams,
}

#[derive(Clone)]
enum LoadedMediaSource {
    File(PathBuf),
    LiveCapture(LiveCaptureDevice),
    Internet(InternetMedia),
}

#[derive(Clone)]
struct LoadedMedia {
    source: LoadedMediaSource,
    duration_seconds: Option<f64>,
    audio_tracks: Vec<AudioTrack>,
    subtitle_paths: Vec<PathBuf>,
    embedded_subtitle_tracks: Vec<EmbeddedSubtitleTrack>,
}

impl LoadedMedia {
    fn file_path(&self) -> Option<&Path> {
        match &self.source {
            LoadedMediaSource::File(path) => Some(path),
            LoadedMediaSource::LiveCapture(_) => None,
            LoadedMediaSource::Internet(_) => None,
        }
    }

    fn display_title(&self) -> String {
        match &self.source {
            LoadedMediaSource::File(path) => display_name(path),
            LoadedMediaSource::LiveCapture(device) => {
                format!("Live Capture: {}", device.display_name)
            }
            LoadedMediaSource::Internet(media) => media.title.clone(),
        }
    }

    fn display_detail(&self) -> String {
        match &self.source {
            LoadedMediaSource::File(path) => {
                if let Some(parent_name) = path.parent().and_then(|p| p.file_name()).and_then(|os| os.to_str()) {
                    format!("📁 {}", parent_name)
                } else {
                    path.display().to_string()
                }
            }
            LoadedMediaSource::LiveCapture(device) => device.detail_label(),
            LoadedMediaSource::Internet(media) => media
                .subtitle
                .clone()
                .unwrap_or_else(|| media.provider_name.clone()),
        }
    }

    fn queue_title(&self) -> String {
        match &self.source {
            LoadedMediaSource::File(path) => queue_display_name(path),
            LoadedMediaSource::LiveCapture(device) => {
                format!("Live Capture: {}", device.display_name)
            }
            LoadedMediaSource::Internet(media) => media.title.clone(),
        }
    }

    fn append_mpv_input_args(&self, command: &mut Command, settings: &PlayerSettings) {
        match &self.source {
            LoadedMediaSource::File(path) => {
                command.arg(path.as_os_str());
            }
            LoadedMediaSource::LiveCapture(device) => {
                device.append_mpv_input_args(command, settings);
            }
            LoadedMediaSource::Internet(media) => {
                if !media.http_headers.is_empty() {
                    if let Some(user_agent) = media
                        .http_headers
                        .iter()
                        .find(|header| header.name.eq_ignore_ascii_case("user-agent"))
                        .map(|header| header.value.trim())
                        .filter(|value| !value.is_empty())
                    {
                        command.arg(format!("--user-agent={user_agent}"));
                    }
                    if let Some(referrer) = media
                        .http_headers
                        .iter()
                        .find(|header| header.name.eq_ignore_ascii_case("referer"))
                        .map(|header| header.value.trim())
                        .filter(|value| !value.is_empty())
                    {
                        command.arg(format!("--referrer={referrer}"));
                    }
                    let header_fields = media
                        .http_headers
                        .iter()
                        .filter(|header| !header.name.is_empty() && !header.value.is_empty())
                        .filter(|header| {
                            !header.name.eq_ignore_ascii_case("user-agent")
                                && !header.name.eq_ignore_ascii_case("referer")
                        })
                        .map(|header| format!("{}: {}", header.name, header.value))
                        .collect::<Vec<_>>()
                        .join(",");
                    if !header_fields.is_empty() {
                        command.arg(format!("--http-header-fields={header_fields}"));
                    }
                }
                command.arg(&media.stream_url);
            }
        }
    }

    fn is_seekable_file(&self) -> bool {
        self.file_path().is_some()
    }

    fn is_live_capture(&self) -> bool {
        matches!(self.source, LoadedMediaSource::LiveCapture(_))
    }

    fn internet_media(&self) -> Option<&InternetMedia> {
        match &self.source {
            LoadedMediaSource::Internet(media) => Some(media),
            LoadedMediaSource::File(_) | LoadedMediaSource::LiveCapture(_) => None,
        }
    }

    fn live_capture_device(&self) -> Option<&LiveCaptureDevice> {
        match &self.source {
            LoadedMediaSource::LiveCapture(device) => Some(device),
            LoadedMediaSource::File(_) | LoadedMediaSource::Internet(_) => None,
        }
    }
}

impl LiveCaptureDevice {
    fn detail_label(&self) -> String {
        #[cfg(target_os = "windows")]
        {
            if self.audio_pin_name.is_some() {
                "DirectShow video + internal audio capture device".to_string()
            } else if self.audio_backend_name.is_some() {
                "DirectShow video + audio capture device".to_string()
            } else {
                "DirectShow video capture device".to_string()
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            format!("Live capture device: {}", self.backend_name)
        }
    }

    fn append_mpv_input_args(&self, command: &mut Command, settings: &PlayerSettings) {
        #[cfg(target_os = "windows")]
        {
            match self.latency_mode {
                LiveCaptureLatencyMode::UltraLow => {
                    append_ultra_low_latency_live_capture_args(
                        command,
                        settings.is_lowest_latency_live_capture_enabled,
                        settings.is_live_capture_exclusive_audio_enabled,
                    );
                }
                LiveCaptureLatencyMode::Stable => {
                    append_stable_live_capture_args(
                        command,
                        settings.is_lowest_latency_live_capture_enabled,
                        settings.is_live_capture_exclusive_audio_enabled,
                    );
                }
            }
            if let Some(audio_pin_name) = self.audio_pin_name.as_deref() {
                command.arg(format!(
                    "--demuxer-lavf-o-add=audio_pin_name={audio_pin_name}"
                ));
            }
            command.arg(directshow_capture_url(
                &self.backend_name,
                self.audio_backend_name.as_deref(),
            ));
        }

        #[cfg(not(target_os = "windows"))]
        {
            command.arg(&self.backend_name);
        }
    }
}

impl LiveCaptureDevice {
    fn with_latency_mode(&self, latency_mode: LiveCaptureLatencyMode) -> Self {
        let mut device = self.clone();
        device.latency_mode = latency_mode;
        device
    }

    fn without_audio(&self) -> Self {
        let mut device = self.clone();
        device.audio_backend_name = None;
        device.audio_pin_name = None;
        device
    }
}

#[cfg(target_os = "windows")]
fn append_ultra_low_latency_live_capture_args(
    command: &mut Command,
    is_lowest_latency_live_capture_enabled: bool,
    is_live_capture_exclusive_audio_enabled: bool,
) {
    command
        .arg("--no-config")
        .arg("--hwdec=no")
        .arg("--cache=no")
        .arg("--profile=low-latency")
        .arg("--audio-buffer=0")
        .arg("--untimed")
        .arg("--video-sync=display-desync")
        .arg("--framedrop=decoder+vo")
        .arg("--demuxer-lavf-probe-info=no")
        .arg(format!(
            "--demuxer-lavf-probesize={LIVE_CAPTURE_PROBE_SIZE_BYTES}"
        ))
        .arg("--demuxer-lavf-analyzeduration=0")
        .arg(format!(
            "--demuxer-lavf-buffersize={LIVE_CAPTURE_LAVF_BUFFER_SIZE_BYTES}"
        ))
        .arg(format!(
            "--stream-buffer-size={LIVE_CAPTURE_STREAM_BUFFER_SIZE}"
        ))
        .arg("--opengl-swapinterval=0")
        .arg("--opengl-dwmflush=no")
        .arg("--demuxer-lavf-o-add=fflags=+nobuffer")
        .arg(format!(
            "--demuxer-lavf-o-add=rtbufsize={LIVE_CAPTURE_RTBUF_SIZE}"
        ))
        .arg(format!(
            "--demuxer-lavf-o-add=audio_buffer_size={LIVE_CAPTURE_AUDIO_BUFFER_MS}"
        ));

    if is_lowest_latency_live_capture_enabled {
        append_lowest_latency_live_capture_args(command, is_live_capture_exclusive_audio_enabled);
    }
}

#[cfg(target_os = "windows")]
fn append_stable_live_capture_args(
    command: &mut Command,
    is_lowest_latency_live_capture_enabled: bool,
    is_live_capture_exclusive_audio_enabled: bool,
) {
    command
        .arg("--no-config")
        .arg("--hwdec=no")
        .arg("--cache=no")
        .arg("--profile=low-latency")
        .arg("--audio-buffer=0")
        .arg("--framedrop=vo")
        .arg("--demuxer-lavf-o-add=fflags=+nobuffer")
        .arg("--demuxer-lavf-o-add=rtbufsize=64M")
        .arg("--demuxer-lavf-o-add=audio_buffer_size=50");

    if is_lowest_latency_live_capture_enabled {
        append_lowest_latency_live_capture_args(command, is_live_capture_exclusive_audio_enabled);
    }
}

#[cfg(target_os = "windows")]
fn append_lowest_latency_live_capture_args(
    command: &mut Command,
    is_live_capture_exclusive_audio_enabled: bool,
) {
    command
        .arg("--ao=wasapi")
        .arg("--audio-channels=stereo")
        .arg("--audio-samplerate=48000")
        .arg("--video-latency-hacks=yes")
        .arg("--swapchain-depth=1")
        .arg("--d3d11-sync-interval=0")
        .arg("--priority=high");

    if is_live_capture_exclusive_audio_enabled {
        command
            .arg("--audio-exclusive=yes")
            .arg("--wasapi-exclusive-buffer=min");
    }
}

#[derive(Clone)]
struct AudioTrack {
    track_id: i64,
    title: String,
    language: Option<String>,
    codec: Option<String>,
}

#[derive(Clone)]
struct EmbeddedSubtitleTrack {
    track_id: i64,
    title: String,
    language: Option<String>,
    codec: Option<String>,
    is_selected: bool,
}

struct TooltipText {
    text: SharedString,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlaybackRepeatMode {
    Off,
    One,
    All,
}

impl PlaybackRepeatMode {
    fn label(self) -> &'static str {
        match self {
            Self::Off => "Repeat off",
            Self::One => "Repeat one",
            Self::All => "Repeat all",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Off => Self::One,
            Self::One => Self::All,
            Self::All => Self::Off,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ResumeBehavior {
    Ask,
    Always,
    Never,
}

impl ResumeBehavior {
    fn label(self) -> &'static str {
        match self {
            Self::Ask => "Ask",
            Self::Always => "Always",
            Self::Never => "Never",
        }
    }
}

impl Default for ResumeBehavior {
    fn default() -> Self {
        Self::Ask
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct SavedWindowBounds {
    width: f32,
    height: f32,
    x: Option<f32>,
    y: Option<f32>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PlayerSettings {
    #[serde(default = "default_volume_percent")]
    default_volume_percent: u8,
    #[serde(default)]
    resume_behavior: ResumeBehavior,
    #[serde(default = "default_preferred_audio_language")]
    preferred_audio_language: String,
    #[serde(default = "default_preferred_subtitle_language")]
    preferred_subtitle_language: String,
    #[serde(default = "default_prefer_embedded_subtitles")]
    prefer_embedded_subtitles: bool,
    #[serde(default = "default_audio_output_device")]
    audio_output_device: String,
    #[serde(default = "default_live_capture_audio_source")]
    live_capture_audio_source: String,
    #[serde(default = "default_lowest_latency_live_capture_enabled")]
    is_lowest_latency_live_capture_enabled: bool,
    #[serde(default)]
    is_live_capture_exclusive_audio_enabled: bool,
    #[serde(default = "default_hardware_decoding_mode")]
    hardware_decoding_mode: String,
    #[serde(default)]
    start_fullscreen: bool,
    #[serde(default)]
    is_backdrop_blur_enabled: bool,
    #[serde(default = "default_subtitle_font_size")]
    subtitle_font_size: u8,
    #[serde(default = "default_subtitle_color")]
    subtitle_color: String,
    #[serde(default = "default_subtitle_position_percent")]
    subtitle_position_percent: u8,
    #[serde(default = "default_seek_step_seconds")]
    seek_step_seconds: f64,
    #[serde(default = "default_volume_step_percent")]
    volume_step_percent: i16,
    #[serde(default)]
    source_providers: Vec<SourceProvider>,
    #[serde(default)]
    window_bounds: Option<SavedWindowBounds>,
    #[serde(default)]
    schema_version: u32,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            default_volume_percent: DEFAULT_VOLUME_PERCENT,
            resume_behavior: ResumeBehavior::Ask,
            preferred_audio_language: "eng".to_string(),
            preferred_subtitle_language: "eng".to_string(),
            prefer_embedded_subtitles: true,
            audio_output_device: AUDIO_OUTPUT_AUTO_DEVICE_ID.to_string(),
            live_capture_audio_source: LIVE_CAPTURE_AUDIO_SOURCE_AUTO.to_string(),
            is_lowest_latency_live_capture_enabled: true,
            is_live_capture_exclusive_audio_enabled: false,
            hardware_decoding_mode: "auto-safe".to_string(),
            start_fullscreen: false,
            is_backdrop_blur_enabled: false,
            subtitle_font_size: 48,
            subtitle_color: "#FFFFFF".to_string(),
            subtitle_position_percent: 95,
            seek_step_seconds: DEFAULT_SEEK_STEP_SECONDS,
            volume_step_percent: DEFAULT_VOLUME_STEP_PERCENT,
            source_providers: Vec::new(),
            window_bounds: None,
            schema_version: 1,
        }
    }
}

impl PlayerSettings {
    fn window_background_appearance(&self) -> WindowBackgroundAppearance {
        if self.is_backdrop_blur_enabled {
            WindowBackgroundAppearance::Blurred
        } else {
            WindowBackgroundAppearance::Transparent
        }
    }

    fn surface_background_color(&self, solid_color: u32, backdrop_blur_alpha: f32) -> gpui::Rgba {
        if self.is_backdrop_blur_enabled {
            rgb_alpha(solid_color, backdrop_blur_alpha)
        } else {
            rgb(solid_color)
        }
    }
}

fn default_volume_percent() -> u8 {
    DEFAULT_VOLUME_PERCENT
}

fn default_preferred_audio_language() -> String {
    "eng".to_string()
}

fn default_preferred_subtitle_language() -> String {
    "eng".to_string()
}

fn default_prefer_embedded_subtitles() -> bool {
    true
}

fn default_audio_output_device() -> String {
    AUDIO_OUTPUT_AUTO_DEVICE_ID.to_string()
}

fn default_live_capture_audio_source() -> String {
    LIVE_CAPTURE_AUDIO_SOURCE_AUTO.to_string()
}

fn default_lowest_latency_live_capture_enabled() -> bool {
    true
}

fn default_hardware_decoding_mode() -> String {
    "auto-safe".to_string()
}

fn default_subtitle_font_size() -> u8 {
    48
}

fn default_subtitle_color() -> String {
    "#FFFFFF".to_string()
}

fn default_subtitle_position_percent() -> u8 {
    95
}

fn default_seek_step_seconds() -> f64 {
    DEFAULT_SEEK_STEP_SECONDS
}

fn default_volume_step_percent() -> i16 {
    DEFAULT_VOLUME_STEP_PERCENT
}

#[derive(Clone)]
struct MediaHistoryEntry {
    path: PathBuf,
    playback_position_seconds: f64,
    duration_seconds: Option<f64>,
    is_completed: bool,
    updated_at_millis: u128,
    selected_audio_track_id: Option<i64>,
    selected_embedded_subtitle_track_id: Option<i64>,
    selected_subtitle_path: Option<PathBuf>,
}

#[derive(Clone)]
struct InternetMediaHistoryEntry {
    media: InternetMedia,
    playback_position_seconds: f64,
    duration_seconds: Option<f64>,
    is_completed: bool,
    updated_at_millis: u128,
}

#[derive(Clone, Default)]
struct PlayerLibrary {
    recent_media_paths: Vec<PathBuf>,
    recent_folder_paths: Vec<PathBuf>,
    pinned_folder_paths: Vec<PathBuf>,
    media_history: Vec<MediaHistoryEntry>,
    recent_internet_media: Vec<InternetMedia>,
    internet_media_history: Vec<InternetMediaHistoryEntry>,
}

#[derive(Clone)]
struct LibraryGridItem {
    path: PathBuf,
    title: String,
    subtitle: Option<String>,
    episode_badge: Option<String>,
    thumbnail_media_path: Option<PathBuf>,
    thumbnail_url: Option<String>,
    internet_media: Option<InternetMedia>,
    resume_history_entry: Option<MediaHistoryEntry>,
    is_watched: bool,
    is_internet_media: bool,
    can_remove_from_continue_watching: bool,
}

#[derive(Clone)]
struct LibraryShelf {
    key: String,
    title: String,
    subtitle: Option<String>,
    empty_message: &'static str,
    items: Vec<LibraryGridItem>,
}

#[derive(Clone, Default)]
struct LibraryViewModel {
    shelves: Vec<LibraryShelf>,
    generation: u64,
}

#[allow(dead_code)]
#[derive(Clone)]
struct Chapter {
    title: String,
    time_seconds: f64,
}

#[allow(dead_code)]
#[derive(Clone)]
struct FolderListingCacheEntry {
    folder_path: PathBuf,
    modified_millis: u128,
    media_paths: Vec<PathBuf>,
}

#[derive(Clone)]
struct TimelineHoverPreview {
    media_path: PathBuf,
    timeline_fraction: f64,
    position_seconds: f64,
    thumbnail_second: u64,
    timeline_width_px: f32,
    thumbnail_path: Option<PathBuf>,
}

#[derive(Clone, PartialEq, Eq)]
struct TimelineThumbnailKey {
    media_path: PathBuf,
    thumbnail_second: u64,
}

#[derive(Clone)]
struct DependencyStatus {
    mpv_path: Option<PathBuf>,
    ffprobe_path: Option<PathBuf>,
    ffmpeg_path: Option<PathBuf>,
}

impl DependencyStatus {
    fn missing_dependency_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();

        if self.mpv_path.is_none() {
            names.push("mpv");
        }
        if self.ffprobe_path.is_none() {
            names.push("ffprobe");
        }

        names
    }

    fn setup_message(&self) -> Option<String> {
        let missing_dependency_names = self.missing_dependency_names();

        if missing_dependency_names.is_empty() {
            None
        } else {
            Some(format!(
                "Missing {}. Install mpv and FFmpeg, add them to PATH, or place portable binaries under tools\\mpv and tools\\ffmpeg before building the installer.",
                missing_dependency_names.join(" and ")
            ))
        }
    }
}

#[derive(Clone, Default)]
struct MpvPlaybackSnapshot {
    time_pos_seconds: Option<f64>,
    duration_seconds: Option<f64>,
    is_paused: Option<bool>,
    is_eof_reached: Option<bool>,
    volume_percent: Option<u8>,
    is_muted: Option<bool>,
    playback_speed: Option<f64>,
    audio_track_id: Option<i64>,
    subtitle_track_id: Option<i64>,
    chapters: Vec<Chapter>,
    current_chapter_index: Option<usize>,
    audio_tracks: Vec<AudioTrack>,
    embedded_subtitle_tracks: Vec<EmbeddedSubtitleTrack>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextMenuSection {
    Audio,
    Subtitles,
    OpenMedia,
    Queue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum SettingsTab {
    General,
    Audio,
    Subtitles,
    Providers,
}

impl SettingsTab {
    fn label(&self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Audio => "Audio & Capture",
            Self::Subtitles => "Subtitles",
            Self::Providers => "Source Providers",
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            Self::General => ICON_SETTINGS,
            Self::Audio => ICON_VOLUME,
            Self::Subtitles => ICON_EYE,
            Self::Providers => ICON_GLOBE,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsSelectorKind {
    AudioOutput,
    ResumeBehavior,
    SeekStep,
    VolumeStep,
    PreferredAudioLanguage,
    PreferredSubtitleLanguage,
    HardwareDecoding,
    StartFullscreen,
    LiveLowestLatency,
    LiveCaptureExclusiveAudio,
    BackdropBlur,
}

impl SettingsSelectorKind {
    fn title(self) -> &'static str {
        match self {
            Self::AudioOutput => "Audio Output",
            Self::ResumeBehavior => "Resume",
            Self::SeekStep => "Seek Step",
            Self::VolumeStep => "Volume Step",
            Self::PreferredAudioLanguage => "Audio Language",
            Self::PreferredSubtitleLanguage => "Subtitle Language",
            Self::HardwareDecoding => "Hardware Decode",
            Self::StartFullscreen => "Start Fullscreen",
            Self::LiveLowestLatency => "Live Lowest Latency",
            Self::LiveCaptureExclusiveAudio => "Exclusive Audio",
            Self::BackdropBlur => "Backdrop Blur",
        }
    }
}

#[derive(Clone)]
enum SettingsSelectorChoice {
    AudioOutputDevice(String),
    ResumeBehavior(ResumeBehavior),
    SeekStepSeconds(f64),
    VolumeStepPercent(i16),
    PreferredAudioLanguage(String),
    PreferredSubtitleLanguage(String),
    HardwareDecodingMode(String),
    StartFullscreen(bool),
    LiveLowestLatency(bool),
    LiveCaptureExclusiveAudio(bool),
    BackdropBlur(bool),
}

#[derive(Clone)]
struct SettingsSelectorOption {
    label: String,
    detail: Option<String>,
    is_selected: bool,
    choice: SettingsSelectorChoice,
}

#[derive(Clone)]
struct SavedWatchSession {
    media_paths: Vec<PathBuf>,
    current_media_path: PathBuf,
    current_queue_index: usize,
    playback_position_seconds: f64,
    volume_percent: u8,
    is_muted: bool,
}

#[derive(Default)]
struct StartupOptions {
    media_paths: Vec<PathBuf>,
    start_fullscreen: Option<bool>,
    resume_behavior: Option<ResumeBehavior>,
}

#[cfg(target_os = "windows")]
struct EmbeddedVideoHost {
    window_id: isize,
    parent_window_id: isize,
}

#[cfg(not(target_os = "windows"))]
struct EmbeddedVideoHost;

struct WatchPlayer {
    focus_handle: FocusHandle,
    playback_queue: Vec<LoadedMedia>,
    current_queue_index: Option<usize>,
    selected_audio_track_id: Option<i64>,
    selected_subtitle_path: Option<PathBuf>,
    selected_embedded_subtitle_track_id: Option<i64>,
    chapters: Vec<Chapter>,
    current_chapter_index: Option<usize>,
    is_playing: bool,
    is_eof_reached: bool,
    playback_position_seconds: f64,
    playback_progress_generation: u64,
    playback_speed: f64,
    volume_percent: u8,
    is_muted: bool,
    subtitle_delay_ms: i32,
    is_shuffle_enabled: bool,
    shuffle_bag: Vec<usize>,
    repeat_mode: PlaybackRepeatMode,
    are_controls_visible: bool,
    is_pointer_over_player_overlay: bool,
    controls_visibility_generation: u64,
    last_controls_reveal_millis: u128,
    osd_message: Option<SharedString>,
    osd_generation: u64,
    is_main_menu_open: bool,
    is_subtitle_menu_open: bool,
    is_settings_modal_open: bool,
    is_library_open: bool,
    is_live_capture_menu_open: bool,
    is_source_search_open: bool,
    is_source_search_pending: bool,
    is_live_capture_scan_pending: bool,
    is_audio_output_scan_pending: bool,
    show_unwatched_only: bool,
    open_settings_selector: Option<SettingsSelectorKind>,
    active_settings_tab: SettingsTab,
    subtitle_menu_anchor: Option<Point<Pixels>>,
    library_context_menu_anchor: Option<Point<Pixels>>,
    library_context_menu_media_path: Option<PathBuf>,
    library_context_menu_internet_media: Option<InternetMedia>,
    hovered_continue_remove_media_path: Option<PathBuf>,
    exiting_continue_remove_media_path: Option<PathBuf>,
    continue_remove_scale_animation_generation: u64,
    hovered_library_icon_key: Option<String>,
    exiting_library_icon_key: Option<String>,
    library_icon_scale_animation_generation: u64,
    open_context_menu_section: Option<ContextMenuSection>,
    pending_watch_session: Option<SavedWatchSession>,
    settings: PlayerSettings,
    library: PlayerLibrary,
    is_library_dirty: bool,
    last_library_flush_millis: u128,
    last_recorded_progress_seconds: f64,
    last_window_bounds_flush_millis: u128,
    library_generation: u64,
    library_view_model: LibraryViewModel,
    folder_listing_cache: HashMap<String, FolderListingCacheEntry>,
    audio_output_devices: Vec<AudioOutputDeviceOption>,
    live_capture_devices: Vec<LiveCaptureDevice>,
    live_capture_audio_devices: Vec<LiveCaptureAudioDevice>,
    source_search_input: Entity<InlineTextInput>,
    source_provider_input: Entity<InlineTextInput>,
    source_search_results: Vec<SourceSearchResult>,
    source_episode_results: Vec<SourceEpisodeResult>,
    source_stream_results: Vec<SourceStreamResult>,
    source_browser_view: SourceBrowserView,
    selected_source_result_index: usize,
    source_search_scroll_handle: ScrollHandle,
    source_episode_scroll_handle: ScrollHandle,
    source_stream_scroll_handle: ScrollHandle,
    selected_source_series_title: Option<String>,
    selected_source_episode_title: Option<String>,
    selected_source_thumbnail_url: Option<String>,
    last_source_search_query: String,
    source_search_status: Option<SharedString>,
    library_shelf_offsets: HashMap<String, usize>,
    collapsed_library_shelf_keys: HashSet<String>,
    dependency_status: DependencyStatus,
    timeline_hover_preview: Option<TimelineHoverPreview>,
    pending_timeline_thumbnail_key: Option<TimelineThumbnailKey>,
    thumbnail_generation: u64,
    is_timeline_scrubbing: bool,
    last_timeline_seek_position_seconds: Option<f64>,
    status_message: Option<SharedString>,
    video_host_window: Option<EmbeddedVideoHost>,
    playback_process: Option<Child>,
    playback_ipc_path: Option<String>,
    playback_log_path: Option<PathBuf>,
}

impl WatchPlayer {
    fn new(initial_media_paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = load_player_settings();
        window.set_background_appearance(settings.window_background_appearance());
        let dependency_status = detect_dependency_status();
        let pending_watch_session = match settings.resume_behavior {
            ResumeBehavior::Ask | ResumeBehavior::Always => load_saved_watch_session(),
            ResumeBehavior::Never => {
                clear_saved_watch_session();
                None
            }
        };
        let status_message = platform_support_message()
            .map(ToString::to_string)
            .or_else(|| dependency_status.setup_message())
            .unwrap_or_else(|| {
                "Open a real media file or folder to populate the player.".to_string()
            });
        let source_search_input = cx.new(|cx| InlineTextInput::new("Search providers", cx));
        cx.observe(&source_search_input, |_player, _input, cx| {
            cx.notify();
        })
        .detach();
        let source_provider_input = cx.new(|cx| {
            InlineTextInput::new(
                "Name | search URL {query} | episodes URL {id} | streams URL {episode_id}",
                cx,
            )
        });
        let mut player = Self {
            focus_handle: cx.focus_handle(),
            playback_queue: Vec::new(),
            current_queue_index: None,
            selected_audio_track_id: None,
            selected_subtitle_path: None,
            selected_embedded_subtitle_track_id: None,
            chapters: Vec::new(),
            current_chapter_index: None,
            is_playing: false,
            is_eof_reached: false,
            playback_position_seconds: 0.0,
            playback_progress_generation: 0,
            playback_speed: 1.0,
            volume_percent: settings.default_volume_percent.min(100),
            is_muted: false,
            subtitle_delay_ms: 0,
            is_shuffle_enabled: false,
            shuffle_bag: Vec::new(),
            repeat_mode: PlaybackRepeatMode::Off,
            are_controls_visible: true,
            is_pointer_over_player_overlay: false,
            controls_visibility_generation: 0,
            last_controls_reveal_millis: 0,
            osd_message: None,
            osd_generation: 0,
            is_main_menu_open: false,
            is_subtitle_menu_open: false,
            is_settings_modal_open: false,
            is_library_open: initial_media_paths.is_empty(),
            is_live_capture_menu_open: false,
            is_source_search_open: false,
            is_source_search_pending: false,
            is_live_capture_scan_pending: false,
            is_audio_output_scan_pending: false,
            show_unwatched_only: false,
            open_settings_selector: None,
            active_settings_tab: SettingsTab::General,
            subtitle_menu_anchor: None,
            library_context_menu_anchor: None,
            library_context_menu_media_path: None,
            library_context_menu_internet_media: None,
            hovered_continue_remove_media_path: None,
            exiting_continue_remove_media_path: None,
            continue_remove_scale_animation_generation: 0,
            hovered_library_icon_key: None,
            exiting_library_icon_key: None,
            library_icon_scale_animation_generation: 0,
            open_context_menu_section: None,
            pending_watch_session,
            settings,
            library: load_player_library(),
            is_library_dirty: false,
            last_library_flush_millis: current_time_millis(),
            last_recorded_progress_seconds: 0.0,
            last_window_bounds_flush_millis: current_time_millis(),
            library_generation: 1,
            library_view_model: LibraryViewModel::default(),
            folder_listing_cache: HashMap::new(),
            audio_output_devices: default_audio_output_options(),
            live_capture_devices: Vec::new(),
            live_capture_audio_devices: Vec::new(),
            source_search_input,
            source_provider_input,
            source_search_results: Vec::new(),
            source_episode_results: Vec::new(),
            source_stream_results: Vec::new(),
            source_browser_view: SourceBrowserView::SearchResults,
            selected_source_result_index: 0,
            source_search_scroll_handle: ScrollHandle::new(),
            source_episode_scroll_handle: ScrollHandle::new(),
            source_stream_scroll_handle: ScrollHandle::new(),
            selected_source_series_title: None,
            selected_source_episode_title: None,
            selected_source_thumbnail_url: None,
            last_source_search_query: String::new(),
            source_search_status: None,
            library_shelf_offsets: HashMap::new(),
            collapsed_library_shelf_keys: HashSet::new(),
            dependency_status,
            timeline_hover_preview: None,
            pending_timeline_thumbnail_key: None,
            thumbnail_generation: 0,
            is_timeline_scrubbing: false,
            last_timeline_seek_position_seconds: None,
            status_message: Some(status_message.into()),
            video_host_window: None,
            playback_process: None,
            playback_ipc_path: None,
            playback_log_path: None,
        };
        if !initial_media_paths.is_empty() {
            player.load_media_paths(initial_media_paths, window, cx);
        } else if player.settings.resume_behavior == ResumeBehavior::Always {
            if let Some(watch_session) = player.pending_watch_session.take() {
                player.load_watch_session(watch_session, window, cx);
            }
        }
        if player.settings.start_fullscreen && !window.is_fullscreen() {
            window.toggle_fullscreen();
        }
        if player.is_library_open {
            player.schedule_library_thumbnail_generation(window, cx);
        }
        player.refresh_audio_output_devices(window, cx);
        prune_thumbnail_cache();
        player.schedule_controls_hide(window, cx);
        player
    }

    fn current_media(&self) -> Option<&LoadedMedia> {
        self.current_queue_index
            .and_then(|queue_index| self.playback_queue.get(queue_index))
    }

    fn current_media_title(&self) -> String {
        self.current_media()
            .map(|media| {
                media
                    .file_path()
                    .map(playback_display_title)
                    .unwrap_or_else(|| media.display_title())
            })
            .unwrap_or_else(|| "No media loaded".to_string())
    }

    fn current_media_detail(&self) -> String {
        self.current_media()
            .map(LoadedMedia::display_detail)
            .unwrap_or_else(|| {
                "Use Menu > Play file, Play folder, Play queue, or Live Capture.".to_string()
            })
    }

    fn current_media_path(&self) -> Option<PathBuf> {
        self.current_media()
            .and_then(LoadedMedia::file_path)
            .map(Path::to_path_buf)
    }

    fn current_media_duration_seconds(&self) -> Option<f64> {
        self.current_media()
            .filter(|media| !media.is_live_capture())
            .and_then(|media| media.duration_seconds)
            .filter(|duration_seconds| duration_seconds.is_finite() && *duration_seconds > 0.0)
    }

    fn current_watch_session(&self) -> Option<SavedWatchSession> {
        let current_media_path = self.current_media_path()?;
        let playback_position_seconds = self.playback_position_seconds.max(0.0);

        if !playback_position_seconds.is_finite()
            || playback_position_seconds < MINIMUM_RESUME_POSITION_SECONDS
        {
            return None;
        }

        let media_paths = self
            .playback_queue
            .iter()
            .filter_map(|media| media.file_path().map(Path::to_path_buf))
            .collect::<Vec<_>>();

        if media_paths.is_empty() {
            return None;
        }

        let current_queue_index = media_paths
            .iter()
            .position(|media_path| media_path == &current_media_path)
            .unwrap_or(0);

        Some(SavedWatchSession {
            media_paths,
            current_media_path,
            current_queue_index,
            playback_position_seconds,
            volume_percent: self.volume_percent,
            is_muted: self.is_muted,
        })
    }

    fn save_current_watch_session(&self) {
        if let Some(watch_session) = self.current_watch_session() {
            save_watch_session(&watch_session);
        }
    }

    fn mark_library_dirty(&mut self) {
        self.library_generation = self.library_generation.wrapping_add(1);
    }

    fn record_current_media_progress(&mut self) {
        if let Some(internet_media) = self
            .current_media()
            .and_then(LoadedMedia::internet_media)
            .cloned()
        {
            self.record_current_internet_media_progress(internet_media);
            return;
        }

        let Some(current_media_path) = self.current_media_path() else {
            return;
        };
        let duration_seconds = self.current_media_duration_seconds();
        let playback_position_seconds = self.playback_position_seconds.max(0.0);
        let is_completed = duration_seconds
            .map(|duration_seconds| {
                playback_position_seconds >= duration_seconds * COMPLETED_MEDIA_FRACTION
                    || duration_seconds - playback_position_seconds
                        <= COMPLETED_MEDIA_REMAINING_SECONDS
            })
            .unwrap_or(false);
        let updated_at_millis = current_time_millis();
        let current_media_path_key = library_media_path_key(&current_media_path);

        self.library
            .media_history
            .retain(|entry| library_media_path_key(&entry.path) != current_media_path_key);
        self.library.media_history.insert(
            0,
            MediaHistoryEntry {
                path: current_media_path.clone(),
                playback_position_seconds,
                duration_seconds,
                is_completed,
                updated_at_millis,
                selected_audio_track_id: self.selected_audio_track_id,
                selected_embedded_subtitle_track_id: self.selected_embedded_subtitle_track_id,
                selected_subtitle_path: self.selected_subtitle_path.clone(),
            },
        );
        self.library.media_history.truncate(MAX_RECENT_MEDIA * 2);
        promote_recent_path(
            &mut self.library.recent_media_paths,
            current_media_path,
            MAX_RECENT_MEDIA,
        );
        self.is_library_dirty = true;
        self.last_recorded_progress_seconds = playback_position_seconds;
        self.mark_library_dirty();
    }

    fn record_current_internet_media_progress(&mut self, internet_media: InternetMedia) {
        let duration_seconds = self.current_media_duration_seconds();
        let playback_position_seconds = self.playback_position_seconds.max(0.0);
        let is_completed = duration_seconds
            .map(|duration_seconds| {
                playback_position_seconds >= duration_seconds * COMPLETED_MEDIA_FRACTION
                    || duration_seconds - playback_position_seconds
                        <= COMPLETED_MEDIA_REMAINING_SECONDS
            })
            .unwrap_or(false);
        let current_internet_media_key = internet_media_key(&internet_media);

        self.library
            .internet_media_history
            .retain(|entry| internet_media_key(&entry.media) != current_internet_media_key);
        self.library.internet_media_history.insert(
            0,
            InternetMediaHistoryEntry {
                media: internet_media.clone(),
                playback_position_seconds,
                duration_seconds,
                is_completed,
                updated_at_millis: current_time_millis(),
            },
        );
        self.library
            .internet_media_history
            .truncate(MAX_RECENT_MEDIA * 2);
        promote_recent_internet_media(
            &mut self.library.recent_internet_media,
            internet_media,
            MAX_RECENT_MEDIA,
        );
        self.is_library_dirty = true;
        self.last_recorded_progress_seconds = playback_position_seconds;
        self.mark_library_dirty();
    }

    fn flush_player_library_if_due(&mut self, force: bool) {
        if !self.is_library_dirty {
            return;
        }

        let now = current_time_millis();
        let is_due =
            now.saturating_sub(self.last_library_flush_millis) >= LIBRARY_FLUSH_INTERVAL_MS;

        if force || is_due {
            save_player_library_atomic(&self.library);
            self.last_library_flush_millis = now;
            self.is_library_dirty = false;
        }
    }

    fn remember_current_queue_in_library(&mut self) {
        let internet_media = self
            .playback_queue
            .iter()
            .filter_map(|media| media.internet_media().cloned())
            .collect::<Vec<_>>();

        for media in internet_media {
            promote_recent_internet_media(
                &mut self.library.recent_internet_media,
                media,
                MAX_RECENT_MEDIA,
            );
        }

        let media_paths = self
            .playback_queue
            .iter()
            .filter_map(|media| media.file_path().map(Path::to_path_buf))
            .collect::<Vec<_>>();

        for media_path in media_paths {
            promote_recent_path(
                &mut self.library.recent_media_paths,
                media_path.clone(),
                MAX_RECENT_MEDIA,
            );
            if let Some(parent_directory) = media_path.parent() {
                promote_recent_path(
                    &mut self.library.recent_folder_paths,
                    parent_directory.to_path_buf(),
                    MAX_RECENT_FOLDERS,
                );
            }
        }

        self.mark_library_dirty();
        save_player_library_atomic(&self.library);
    }

    fn selected_audio_track_label(&self) -> String {
        let Some(current_media) = self.current_media() else {
            return "No media loaded".to_string();
        };

        self.selected_audio_track_id
            .and_then(|selected_track_id| {
                current_media
                    .audio_tracks
                    .iter()
                    .find(|track| track.track_id == selected_track_id)
                    .map(|track| track.title.clone())
            })
            .unwrap_or_else(|| {
                if current_media.audio_tracks.is_empty() {
                    "No audio tracks found".to_string()
                } else {
                    "Default audio".to_string()
                }
            })
    }

    fn selected_subtitle_label(&self) -> String {
        let Some(current_media) = self.current_media() else {
            return "No media loaded".to_string();
        };

        if let Some(selected_track_id) = self.selected_embedded_subtitle_track_id {
            if let Some(subtitle_track) = current_media
                .embedded_subtitle_tracks
                .iter()
                .find(|track| track.track_id == selected_track_id)
            {
                return subtitle_track.title.clone();
            }
        }

        if let Some(selected_subtitle_path) = self.selected_subtitle_path.as_ref() {
            return display_name(selected_subtitle_path);
        }

        "Off".to_string()
    }

    fn queue_summary_label(&self) -> String {
        match (self.playback_queue.len(), self.current_queue_index) {
            (0, _) => "No queued media".to_string(),
            (queue_len, Some(current_index)) => {
                format!("{} of {}", current_index + 1, queue_len)
            }
            (queue_len, None) => format!("{queue_len} items"),
        }
    }

    fn is_video_surface_active(&self) -> bool {
        self.current_media().is_some() && self.playback_process.is_some()
    }

    fn is_current_media_live_capture(&self) -> bool {
        self.current_media()
            .is_some_and(LoadedMedia::is_live_capture)
    }

    fn is_library_surface_visible(&self) -> bool {
        self.is_library_open || self.current_media().is_none()
    }

    fn set_playback_position_seconds(&mut self, position_seconds: f64) {
        let position_seconds = position_seconds.max(0.0);
        self.playback_position_seconds =
            if let Some(duration_seconds) = self.current_media_duration_seconds() {
                position_seconds.clamp(0.0, duration_seconds)
            } else {
                position_seconds
            };
    }

    fn schedule_playback_state_poll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ipc_path) = self.playback_ipc_path.clone() else {
            return;
        };
        let generation_to_update = self.playback_progress_generation;

        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(PLAYBACK_STATE_POLL_MS))
                .await;
            let playback_snapshot = read_mpv_playback_snapshot(&ipc_path);

            let _ = this.update_in(cx, |player, window, cx| {
                if player.playback_progress_generation != generation_to_update
                    || player.playback_ipc_path.as_ref() != Some(&ipc_path)
                {
                    return;
                }

                if let Some(playback_snapshot) = playback_snapshot {
                    if player.apply_mpv_playback_snapshot(playback_snapshot, window, cx) {
                        cx.notify();
                        return;
                    }
                } else if let Some(exit_status) = player.playback_process_exit_status() {
                    if player.retry_live_capture_after_startup_failure(window, cx) {
                        cx.notify();
                        return;
                    }

                    let exit_message = player.playback_backend_exit_message(exit_status);
                    player.is_playing = false;
                    player.playback_progress_generation += 1;
                    player.playback_process = None;
                    player.playback_ipc_path = None;
                    player.status_message = Some(exit_message.into());
                    cx.notify();
                    return;
                }

                player.schedule_playback_state_poll(window, cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn apply_mpv_playback_snapshot(
        &mut self,
        playback_snapshot: MpvPlaybackSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(position_seconds) = playback_snapshot.time_pos_seconds {
            self.set_playback_position_seconds(position_seconds);
        }

        if let Some(duration_seconds) = playback_snapshot.duration_seconds {
            if let Some(current_queue_index) = self.current_queue_index {
                if let Some(current_media) = self.playback_queue.get_mut(current_queue_index) {
                    current_media.duration_seconds = Some(duration_seconds);
                }
            }
        }

        if let Some(is_paused) = playback_snapshot.is_paused {
            self.is_playing = !is_paused;
        }
        if let Some(volume_percent) = playback_snapshot.volume_percent {
            self.volume_percent = volume_percent.min(100);
        }
        if let Some(is_muted) = playback_snapshot.is_muted {
            self.is_muted = is_muted;
        }
        if let Some(playback_speed) = playback_snapshot.playback_speed {
            self.playback_speed = playback_speed.clamp(0.25, 4.0);
        }
        if let Some(audio_track_id) = playback_snapshot.audio_track_id {
            self.selected_audio_track_id = Some(audio_track_id);
        }
        if let Some(subtitle_track_id) = playback_snapshot.subtitle_track_id {
            if subtitle_track_id > 0 {
                self.selected_embedded_subtitle_track_id = Some(subtitle_track_id);
            }
        }

        if let Some(current_queue_index) = self.current_queue_index {
            if let Some(current_media) = self.playback_queue.get_mut(current_queue_index) {
                if !playback_snapshot.audio_tracks.is_empty() {
                    current_media.audio_tracks = playback_snapshot.audio_tracks;
                }
                if !playback_snapshot.embedded_subtitle_tracks.is_empty() {
                    current_media.embedded_subtitle_tracks =
                        playback_snapshot.embedded_subtitle_tracks;
                }
            }
        }
        self.chapters = playback_snapshot.chapters;
        self.current_chapter_index = playback_snapshot.current_chapter_index;

        let is_eof_reached = playback_snapshot.is_eof_reached.unwrap_or(false);
        if is_eof_reached && !self.is_eof_reached {
            self.is_eof_reached = true;
            self.handle_end_of_current_media(window, cx);
            return true;
        }
        if !is_eof_reached {
            self.is_eof_reached = false;
        }

        self.record_current_media_progress();
        self.flush_player_library_if_due(false);
        false
    }

    fn playback_process_exit_status(&mut self) -> Option<std::process::ExitStatus> {
        self.playback_process
            .as_mut()
            .and_then(|process| process.try_wait().ok())
            .flatten()
    }

    fn retry_live_capture_after_startup_failure(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(current_queue_index) = self.current_queue_index else {
            return false;
        };
        let Some(current_device) = self
            .playback_queue
            .get(current_queue_index)
            .and_then(LoadedMedia::live_capture_device)
            .cloned()
        else {
            return false;
        };

        let retry_device = match current_device.latency_mode {
            LiveCaptureLatencyMode::UltraLow => {
                Some(current_device.with_latency_mode(LiveCaptureLatencyMode::Stable))
            }
            LiveCaptureLatencyMode::Stable if current_device.audio_backend_name.is_some() => {
                Some(current_device.without_audio())
            }
            LiveCaptureLatencyMode::Stable => None,
        };
        let Some(retry_device) = retry_device else {
            return false;
        };
        let retry_status_message = if retry_device.audio_backend_name.is_some() {
            "Retrying live capture with stable low-latency mode."
        } else {
            "Audio capture failed; retrying live capture without audio."
        };

        if let Some(current_media) = self.playback_queue.get_mut(current_queue_index) {
            current_media.source = LoadedMediaSource::LiveCapture(retry_device);
        }

        self.status_message = Some(retry_status_message.into());
        self.start_current_media_playback(window, cx);
        true
    }

    fn playback_backend_exit_message(&self, exit_status: std::process::ExitStatus) -> String {
        let exit_label = exit_status
            .code()
            .map(|exit_code| format!("exit code {exit_code}"))
            .unwrap_or_else(|| "terminated".to_string());

        if let Some(log_message) = self
            .playback_log_path
            .as_ref()
            .and_then(|log_path| concise_mpv_log_message(log_path))
        {
            format!("Playback backend exited ({exit_label}): {log_message}")
        } else {
            format!("Playback backend exited ({exit_label}).")
        }
    }

    fn handle_end_of_current_media(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_playing = false;
        self.record_current_media_progress();
        self.flush_player_library_if_due(true);

        if self.repeat_mode == PlaybackRepeatMode::One {
            self.show_osd("Repeat one".to_string(), window, cx);
            self.start_current_media_playback(window, cx);
            return;
        }

        if let Some(next_queue_index) = self.next_queue_index_for_playback(true) {
            self.play_queue_item(next_queue_index, window, cx);
            return;
        }

        self.playback_progress_generation += 1;
        self.stop_playback_process();
        self.status_message = Some("Finished.".into());
        clear_saved_watch_session();
        self.reveal_controls(window, cx);
    }

    fn reveal_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current_time_millis = current_time_millis();
        let should_reschedule_hide = !self.are_controls_visible
            || current_time_millis.saturating_sub(self.last_controls_reveal_millis)
                >= CONTROLS_REVEAL_THROTTLE_MS;

        self.are_controls_visible = true;
        if should_reschedule_hide {
            self.last_controls_reveal_millis = current_time_millis;
            self.controls_visibility_generation += 1;
            self.schedule_controls_hide(window, cx);
        }
        cx.notify();
    }

    fn schedule_controls_hide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let generation_to_hide = self.controls_visibility_generation;

        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(CONTROLS_HIDE_DELAY_MS))
                .await;

            let _ = this.update_in(cx, |player, _window, cx| {
                if player.controls_visibility_generation == generation_to_hide
                    && !player.is_main_menu_open
                    && !player.is_subtitle_menu_open
                    && !player.is_settings_modal_open
                    && !player.is_source_search_open
                    && !player.is_pointer_over_player_overlay
                {
                    player.are_controls_visible = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn set_player_overlay_hover(
        &mut self,
        is_hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_pointer_over_player_overlay == is_hovered {
            return;
        }

        self.is_pointer_over_player_overlay = is_hovered;
        if is_hovered {
            self.reveal_controls(window, cx);
        } else {
            self.controls_visibility_generation += 1;
            self.schedule_controls_hide(window, cx);
            cx.notify();
        }
    }

    fn show_osd(&mut self, message: String, window: &mut Window, cx: &mut Context<Self>) {
        self.osd_message = Some(message.into());
        self.osd_generation += 1;
        let generation_to_hide = self.osd_generation;

        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(OSD_HIDE_DELAY_MS))
                .await;

            let _ = this.update_in(cx, |player, _window, cx| {
                if player.osd_generation == generation_to_hide {
                    player.osd_message = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn toggle_playback(&mut self, _: &TogglePlayback, window: &mut Window, cx: &mut Context<Self>) {
        if self.current_media().is_some() {
            if self.playback_process.is_none() {
                self.start_current_media_playback(window, cx);
                self.reveal_controls(window, cx);
                return;
            }

            self.is_playing = !self.is_playing;
            self.status_message = None;
            self.send_mpv_command(json!(["set_property", "pause", !self.is_playing]));
            self.playback_progress_generation += 1;
            if self.is_playing {
                self.schedule_playback_state_poll(window, cx);
            }
            self.show_osd(
                if self.is_playing { "Play" } else { "Pause" }.to_string(),
                window,
                cx,
            );
        } else {
            self.is_playing = false;
            self.status_message = Some("Load media or choose a live capture device first.".into());
        }
        self.reveal_controls(window, cx);
    }

    fn increase_subtitle_delay(
        &mut self,
        _: &IncreaseSubtitleDelay,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.subtitle_delay_ms += 50;
        self.send_mpv_command(json!([
            "set_property",
            "sub-delay",
            self.subtitle_delay_ms as f64 / 1000.0
        ]));
        self.show_osd(
            format!("Subtitle delay {} ms", self.subtitle_delay_ms),
            window,
            cx,
        );
        self.reveal_controls(window, cx);
    }

    fn decrease_subtitle_delay(
        &mut self,
        _: &DecreaseSubtitleDelay,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.subtitle_delay_ms -= 50;
        self.send_mpv_command(json!([
            "set_property",
            "sub-delay",
            self.subtitle_delay_ms as f64 / 1000.0
        ]));
        self.show_osd(
            format!("Subtitle delay {} ms", self.subtitle_delay_ms),
            window,
            cx,
        );
        self.reveal_controls(window, cx);
    }

    fn reset_subtitle_delay(
        &mut self,
        _: &ResetSubtitleDelay,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.subtitle_delay_ms = 0;
        self.send_mpv_command(json!(["set_property", "sub-delay", 0]));
        self.show_osd("Subtitle delay reset".to_string(), window, cx);
        self.reveal_controls(window, cx);
    }

    fn scrub_to_timeline_fraction(
        &mut self,
        timeline_fraction: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_timeline_seek(timeline_fraction, false, false, window, cx);
    }

    fn finish_timeline_scrub(
        &mut self,
        timeline_fraction: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_timeline_scrubbing = false;
        self.apply_timeline_seek(timeline_fraction, true, true, window, cx);
        self.last_timeline_seek_position_seconds = None;
    }

    fn apply_timeline_seek(
        &mut self,
        timeline_fraction: f64,
        should_force_backend_seek: bool,
        should_show_osd: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(duration_seconds) = self.current_media_duration_seconds() else {
            self.reveal_controls(window, cx);
            return;
        };

        let clamped_fraction = timeline_fraction.clamp(0.0, 1.0);
        let position_seconds = duration_seconds * clamped_fraction;
        self.set_playback_position_seconds(position_seconds);
        self.is_eof_reached = false;

        if should_force_backend_seek {
            self.send_mpv_command(json!(["seek", position_seconds, "absolute+exact"]));
            self.last_timeline_seek_position_seconds = Some(position_seconds);
        }

        if should_show_osd {
            self.show_osd(
                format!("Seek {}", format_timestamp(position_seconds)),
                window,
                cx,
            );
        } else {
            cx.notify();
        }

        self.reveal_controls(window, cx);
    }

    fn seek_relative_seconds(
        &mut self,
        seconds_delta: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.current_media_duration_seconds().is_none() {
            self.reveal_controls(window, cx);
            return;
        }

        let target_position_seconds = self.playback_position_seconds + seconds_delta;
        self.last_timeline_seek_position_seconds = None;
        self.set_playback_position_seconds(target_position_seconds);
        self.is_eof_reached = false;
        self.send_mpv_command(json!(["seek", seconds_delta, "relative+exact"]));
        self.show_osd(
            format!("Seek {}", format_timestamp(self.playback_position_seconds)),
            window,
            cx,
        );
        self.reveal_controls(window, cx);
    }

    fn seek_backward(&mut self, _: &SeekBackward, window: &mut Window, cx: &mut Context<Self>) {
        self.seek_relative_seconds(-self.settings.seek_step_seconds, window, cx);
    }

    fn seek_forward(&mut self, _: &SeekForward, window: &mut Window, cx: &mut Context<Self>) {
        self.seek_relative_seconds(self.settings.seek_step_seconds, window, cx);
    }

    fn update_timeline_hover_preview(
        &mut self,
        timeline_fraction: f64,
        timeline_width_px: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(duration_seconds) = self.current_media_duration_seconds() else {
            return;
        };
        let Some(media_path) = self.current_media_path() else {
            return;
        };

        let clamped_fraction = timeline_fraction.clamp(0.0, 1.0);
        let position_seconds = duration_seconds * clamped_fraction;
        let thumbnail_second = thumbnail_second_for_position(position_seconds);
        let thumbnail_key = TimelineThumbnailKey {
            media_path: media_path.clone(),
            thumbnail_second,
        };
        let previous_thumbnail_path = self
            .timeline_hover_preview
            .as_ref()
            .filter(|preview| {
                preview.media_path == media_path && preview.thumbnail_second == thumbnail_second
            })
            .and_then(|preview| preview.thumbnail_path.clone());
        let cached_thumbnail_path = timeline_thumbnail_path(&media_path, position_seconds);
        let thumbnail_path = previous_thumbnail_path.or_else(|| {
            cached_thumbnail_path
                .is_file()
                .then_some(cached_thumbnail_path)
        });

        self.timeline_hover_preview = Some(TimelineHoverPreview {
            media_path: media_path.clone(),
            timeline_fraction: clamped_fraction,
            position_seconds,
            thumbnail_second,
            timeline_width_px: timeline_width_px.max(1.0),
            thumbnail_path: thumbnail_path.clone(),
        });
        cx.notify();

        if thumbnail_path.is_some()
            || self.dependency_status.ffmpeg_path.is_none()
            || self.pending_timeline_thumbnail_key.as_ref() == Some(&thumbnail_key)
        {
            self.reveal_controls(window, cx);
            return;
        }

        self.pending_timeline_thumbnail_key = Some(thumbnail_key.clone());
        self.thumbnail_generation += 1;
        let thumbnail_generation = self.thumbnail_generation;
        let ffmpeg_path = self.dependency_status.ffmpeg_path.clone();

        cx.spawn_in(window, async move |this, cx| {
            let thumbnail_path = ffmpeg_path.and_then(|ffmpeg_path| {
                generate_timeline_thumbnail(&ffmpeg_path, &media_path, position_seconds)
            });

            let _ = this.update_in(cx, |player, _window, cx| {
                if player.pending_timeline_thumbnail_key.as_ref() == Some(&thumbnail_key) {
                    player.pending_timeline_thumbnail_key = None;
                }

                if player.thumbnail_generation == thumbnail_generation {
                    if let Some(preview) = player.timeline_hover_preview.as_mut() {
                        if preview.media_path == thumbnail_key.media_path
                            && preview.thumbnail_second == thumbnail_key.thumbnail_second
                        {
                            preview.thumbnail_path = thumbnail_path;
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
        self.reveal_controls(window, cx);
    }

    fn clear_timeline_hover_preview(&mut self, cx: &mut Context<Self>) {
        if self.timeline_hover_preview.is_some() {
            self.timeline_hover_preview = None;
            self.thumbnail_generation += 1;
            cx.notify();
        }
    }

    fn stop_current_playback(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.playback_progress_generation += 1;
        self.record_current_media_progress();
        self.flush_player_library_if_due(true);
        self.stop_playback_process();
        self.is_playing = false;
        self.playback_position_seconds = 0.0;
        self.timeline_hover_preview = None;
        self.pending_timeline_thumbnail_key = None;
        self.is_timeline_scrubbing = false;
        self.last_timeline_seek_position_seconds = None;
        self.status_message = Some("Stopped.".into());
        clear_saved_watch_session();
        self.reveal_controls(window, cx);
    }

    fn toggle_window_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !window.is_fullscreen() {
            self.save_current_window_bounds(window);
        }
        window.toggle_fullscreen();
        self.reveal_controls(window, cx);
    }

    fn toggle_fullscreen(
        &mut self,
        _: &ToggleFullscreen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_window_fullscreen(window, cx);
    }

    fn should_toggle_fullscreen_from_video_double_click(
        &self,
        window_position: Point<Pixels>,
        window: &Window,
    ) -> bool {
        if !self.is_video_surface_active()
            || self.is_library_open
            || self.is_main_menu_open
            || self.is_subtitle_menu_open
            || self.is_settings_modal_open
            || self.open_context_menu_section.is_some()
            || self.pending_watch_session.is_some()
        {
            return false;
        }

        is_position_inside_video_double_click_region(window.viewport_size().height, window_position)
    }

    fn toggle_mute(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_muted = !self.is_muted;
        self.send_mpv_command(json!(["set_property", "mute", self.is_muted]));
        self.show_osd(
            if self.is_muted { "Muted" } else { "Unmuted" }.to_string(),
            window,
            cx,
        );
        self.reveal_controls(window, cx);
    }

    fn toggle_mute_action(&mut self, _: &ToggleMute, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_mute(window, cx);
    }

    fn change_volume_by(&mut self, volume_delta: i16, window: &mut Window, cx: &mut Context<Self>) {
        let current_volume = i16::from(self.volume_percent);
        let next_volume = (current_volume + volume_delta).clamp(0, 100) as u8;
        self.set_volume_percent(next_volume, window, cx);
    }

    fn increase_volume(&mut self, _: &VolumeUp, window: &mut Window, cx: &mut Context<Self>) {
        self.change_volume_by(self.settings.volume_step_percent, window, cx);
    }

    fn decrease_volume(&mut self, _: &VolumeDown, window: &mut Window, cx: &mut Context<Self>) {
        self.change_volume_by(-self.settings.volume_step_percent, window, cx);
    }

    fn previous_queue_item(
        &mut self,
        _: &PreviousQueueItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.play_previous_queue_item(window, cx);
    }

    fn next_queue_item(&mut self, _: &NextQueueItem, window: &mut Window, cx: &mut Context<Self>) {
        self.play_next_queue_item(window, cx);
    }

    fn frame_step_forward(
        &mut self,
        _: &FrameStepForward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.send_mpv_command(json!(["frame-step"]));
        self.show_osd("Frame forward".to_string(), window, cx);
        self.reveal_controls(window, cx);
    }

    fn frame_step_backward(
        &mut self,
        _: &FrameStepBackward,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.send_mpv_command(json!(["frame-back-step"]));
        self.show_osd("Frame back".to_string(), window, cx);
        self.reveal_controls(window, cx);
    }

    fn set_ab_loop_a(&mut self, _: &SetABLoopA, window: &mut Window, cx: &mut Context<Self>) {
        self.send_mpv_command(json!([
            "set_property",
            "ab-loop-a",
            self.playback_position_seconds
        ]));
        self.show_osd("A-B loop: A set".to_string(), window, cx);
        self.reveal_controls(window, cx);
    }

    fn set_ab_loop_b(&mut self, _: &SetABLoopB, window: &mut Window, cx: &mut Context<Self>) {
        self.send_mpv_command(json!([
            "set_property",
            "ab-loop-b",
            self.playback_position_seconds
        ]));
        self.show_osd("A-B loop: B set".to_string(), window, cx);
        self.reveal_controls(window, cx);
    }

    fn clear_ab_loop(&mut self, _: &ClearABLoop, window: &mut Window, cx: &mut Context<Self>) {
        self.send_mpv_command(json!(["set_property", "ab-loop-a", "no"]));
        self.send_mpv_command(json!(["set_property", "ab-loop-b", "no"]));
        self.show_osd("A-B loop cleared".to_string(), window, cx);
        self.reveal_controls(window, cx);
    }

    fn seek_to_next_chapter(
        &mut self,
        _: &SeekNextChapter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.send_mpv_command(json!(["add", "chapter", 1]));
        self.show_osd("Next chapter".to_string(), window, cx);
        self.reveal_controls(window, cx);
    }

    fn seek_to_previous_chapter(
        &mut self,
        _: &SeekPreviousChapter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.send_mpv_command(json!(["add", "chapter", -1]));
        self.show_osd("Previous chapter".to_string(), window, cx);
        self.reveal_controls(window, cx);
    }

    fn jump_to_percent(&mut self, percent: f64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(duration_seconds) = self.current_media_duration_seconds() else {
            self.reveal_controls(window, cx);
            return;
        };

        let position_seconds = duration_seconds * percent.clamp(0.0, 1.0);
        self.set_playback_position_seconds(position_seconds);
        self.send_mpv_command(json!(["seek", position_seconds, "absolute+exact"]));
        self.show_osd(
            format!("Seek {}", format_timestamp(position_seconds)),
            window,
            cx,
        );
        self.reveal_controls(window, cx);
    }

    fn jump_to_percent_0(
        &mut self,
        _: &JumpToPercent0,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_to_percent(0.0, window, cx);
    }
    fn jump_to_percent_1(
        &mut self,
        _: &JumpToPercent1,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_to_percent(0.1, window, cx);
    }
    fn jump_to_percent_2(
        &mut self,
        _: &JumpToPercent2,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_to_percent(0.2, window, cx);
    }
    fn jump_to_percent_3(
        &mut self,
        _: &JumpToPercent3,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_to_percent(0.3, window, cx);
    }
    fn jump_to_percent_4(
        &mut self,
        _: &JumpToPercent4,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_to_percent(0.4, window, cx);
    }
    fn jump_to_percent_5(
        &mut self,
        _: &JumpToPercent5,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_to_percent(0.5, window, cx);
    }
    fn jump_to_percent_6(
        &mut self,
        _: &JumpToPercent6,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_to_percent(0.6, window, cx);
    }
    fn jump_to_percent_7(
        &mut self,
        _: &JumpToPercent7,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_to_percent(0.7, window, cx);
    }
    fn jump_to_percent_8(
        &mut self,
        _: &JumpToPercent8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_to_percent(0.8, window, cx);
    }
    fn jump_to_percent_9(
        &mut self,
        _: &JumpToPercent9,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.jump_to_percent(0.9, window, cx);
    }

    fn change_playback_speed(
        &mut self,
        speed_delta: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.playback_speed = (self.playback_speed + speed_delta).clamp(0.25, 4.0);
        self.send_mpv_command(json!(["set_property", "speed", self.playback_speed]));
        self.show_osd(format!("Speed {:.1}x", self.playback_speed), window, cx);
        self.reveal_controls(window, cx);
    }

    fn increase_playback_speed(
        &mut self,
        _: &IncreasePlaybackSpeed,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_playback_speed(SPEED_STEP, window, cx);
    }

    fn decrease_playback_speed(
        &mut self,
        _: &DecreasePlaybackSpeed,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_playback_speed(-SPEED_STEP, window, cx);
    }

    fn toggle_shuffle(&mut self, _: &ToggleShuffle, window: &mut Window, cx: &mut Context<Self>) {
        self.is_shuffle_enabled = !self.is_shuffle_enabled;
        self.rebuild_shuffle_bag();
        self.show_osd(
            if self.is_shuffle_enabled {
                "Shuffle on"
            } else {
                "Shuffle off"
            }
            .to_string(),
            window,
            cx,
        );
        self.reveal_controls(window, cx);
    }

    fn cycle_repeat_mode(
        &mut self,
        _: &CycleRepeatMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.repeat_mode = self.repeat_mode.next();
        self.show_osd(self.repeat_mode.label().to_string(), window, cx);
        self.reveal_controls(window, cx);
    }

    fn toggle_main_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_main_menu_open = !self.is_main_menu_open;
        self.is_subtitle_menu_open = false;
        self.is_live_capture_menu_open = false;
        self.is_settings_modal_open = false;
        self.open_settings_selector = None;
        self.subtitle_menu_anchor = None;
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.library_context_menu_internet_media = None;
        self.open_context_menu_section = None;
        self.reveal_controls(window, cx);
    }

    fn show_subtitle_context_menu(
        &mut self,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_subtitle_menu_open = true;
        self.is_main_menu_open = false;
        self.is_live_capture_menu_open = false;
        self.is_settings_modal_open = false;
        self.is_source_search_open = false;
        self.open_settings_selector = None;
        self.subtitle_menu_anchor = Some(anchor);
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.library_context_menu_internet_media = None;
        self.open_context_menu_section = None;
        self.reveal_controls(window, cx);
    }

    fn close_open_menus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.is_live_capture_menu_open = false;
        self.is_settings_modal_open = false;
        self.is_source_search_open = false;
        self.open_settings_selector = None;
        self.subtitle_menu_anchor = None;
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.library_context_menu_internet_media = None;
        self.open_context_menu_section = None;
        self.reveal_controls(window, cx);
    }

    fn open_settings_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_settings_modal_open = true;
        self.open_settings_selector = None;
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.is_live_capture_menu_open = false;
        self.is_source_search_open = false;
        self.subtitle_menu_anchor = None;
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.library_context_menu_internet_media = None;
        self.open_context_menu_section = None;
        self.reveal_controls(window, cx);
    }

    fn close_settings_modal(&mut self, cx: &mut Context<Self>) {
        self.is_settings_modal_open = false;
        self.open_settings_selector = None;
        cx.notify();
    }

    fn open_source_search_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_source_search_open = true;
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.is_live_capture_menu_open = false;
        self.is_settings_modal_open = false;
        self.open_settings_selector = None;
        self.subtitle_menu_anchor = None;
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.library_context_menu_internet_media = None;
        self.open_context_menu_section = None;
        self.source_search_status = Some("Search AllAnime and saved providers.".into());
        window.focus(&self.source_search_input.focus_handle(cx), cx);
        self.reveal_controls(window, cx);
    }

    fn close_source_search_overlay(&mut self, cx: &mut Context<Self>) {
        self.is_source_search_open = false;
        self.is_source_search_pending = false;
        self.source_search_results.clear();
        self.source_episode_results.clear();
        self.source_stream_results.clear();
        self.source_browser_view = SourceBrowserView::SearchResults;
        self.selected_source_result_index = 0;
        self.selected_source_series_title = None;
        self.selected_source_episode_title = None;
        self.selected_source_thumbnail_url = None;
        self.source_search_status = None;
        cx.notify();
    }

    fn add_source_provider_from_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let provider_input = self.source_provider_input.read(cx).text();
        let Some(provider) = source_provider_from_input(&provider_input) else {
            self.source_search_status =
                Some("Enter a provider as Name | search URL | optional episodes URL | optional streams URL.".into());
            cx.notify();
            return;
        };

        upsert_source_provider(&mut self.settings.source_providers, provider.clone());
        save_player_settings(&self.settings);
        self.source_provider_input
            .update(cx, |input, cx| input.clear(cx));
        self.status_message = Some(format!("Source provider added: {}", provider.name).into());
        self.source_search_status = Some("Provider added. Search for a title when ready.".into());
        let next_focus_handle = if self.is_source_search_open {
            self.source_search_input.focus_handle(cx)
        } else {
            self.source_provider_input.focus_handle(cx)
        };
        window.focus(&next_focus_handle, cx);
        cx.notify();
    }

    fn remove_source_provider(&mut self, provider_id: String, cx: &mut Context<Self>) {
        self.settings
            .source_providers
            .retain(|provider| provider.id != provider_id);
        save_player_settings(&self.settings);
        self.source_search_results.clear();
        self.source_episode_results.clear();
        self.source_stream_results.clear();
        self.source_browser_view = SourceBrowserView::SearchResults;
        self.selected_source_result_index = 0;
        self.selected_source_series_title = None;
        self.selected_source_episode_title = None;
        self.selected_source_thumbnail_url = None;
        self.source_search_status = Some("Source provider removed.".into());
        cx.notify();
    }

    fn submit_source_search_or_selection(
        &mut self,
        _: &SubmitSourceSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_source_search_open || self.is_source_search_pending {
            return;
        }

        if available_source_provider_count(&self.settings) == 0 {
            self.add_source_provider_from_input(window, cx);
            return;
        }

        let search_query = self.source_search_input.read(cx).text().trim().to_string();
        let should_search = self.source_browser_view == SourceBrowserView::SearchResults
            && (self.source_search_results.is_empty()
                || search_query != self.last_source_search_query);

        if should_search {
            self.search_source_providers(window, cx);
        } else {
            self.confirm_selected_source_result(window, cx);
        }
    }

    fn select_previous_source_result(
        &mut self,
        _: &SelectPreviousSourceResult,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.shift_selected_source_result(-1, cx);
    }

    fn select_next_source_result(
        &mut self,
        _: &SelectNextSourceResult,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.shift_selected_source_result(1, cx);
    }

    fn shift_selected_source_result(&mut self, direction: i32, cx: &mut Context<Self>) {
        if !self.is_source_search_open || self.is_source_search_pending {
            return;
        }

        let result_count = self.active_source_result_count();
        if result_count == 0 {
            self.selected_source_result_index = 0;
            cx.notify();
            return;
        }

        let max_index = result_count.saturating_sub(1);
        self.selected_source_result_index = if direction < 0 {
            self.selected_source_result_index.saturating_sub(1)
        } else {
            (self.selected_source_result_index + 1).min(max_index)
        };
        self.scroll_selected_source_result_into_view();
        cx.notify();
    }

    fn active_source_result_count(&self) -> usize {
        match self.source_browser_view {
            SourceBrowserView::SearchResults => self.source_search_results.len(),
            SourceBrowserView::Episodes => self.source_episode_results.len(),
            SourceBrowserView::Streams => self.source_stream_results.len(),
        }
    }

    fn active_source_scroll_handle(&self) -> &ScrollHandle {
        match self.source_browser_view {
            SourceBrowserView::SearchResults => &self.source_search_scroll_handle,
            SourceBrowserView::Episodes => &self.source_episode_scroll_handle,
            SourceBrowserView::Streams => &self.source_stream_scroll_handle,
        }
    }

    fn scroll_selected_source_result_into_view(&self) {
        if self.active_source_result_count() > 0 {
            self.active_source_scroll_handle()
                .scroll_to_item(self.selected_source_result_index);
        }
    }

    fn clamp_selected_source_result_index(&mut self) {
        let result_count = self.active_source_result_count();
        if result_count == 0 {
            self.selected_source_result_index = 0;
        } else {
            self.selected_source_result_index =
                self.selected_source_result_index.min(result_count - 1);
        }
    }

    fn confirm_selected_source_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clamp_selected_source_result_index();
        match self.source_browser_view {
            SourceBrowserView::SearchResults => {
                if let Some(search_result) = self
                    .source_search_results
                    .get(self.selected_source_result_index)
                    .cloned()
                {
                    self.load_source_search_result(search_result, window, cx);
                }
            }
            SourceBrowserView::Episodes => {
                if let Some(episode_result) = self
                    .source_episode_results
                    .get(self.selected_source_result_index)
                    .cloned()
                {
                    self.load_source_episode_result(episode_result, window, cx);
                }
            }
            SourceBrowserView::Streams => {
                if let Some(stream_result) = self
                    .source_stream_results
                    .get(self.selected_source_result_index)
                    .cloned()
                {
                    self.load_source_stream_result(stream_result, window, cx);
                }
            }
        }
    }

    fn select_suggestion(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.source_search_input.update(cx, |input, cx| {
            input.set_text(text, cx);
        });
        self.search_source_providers(window, cx);
    }

    fn render_type_ahead_suggestions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_input = self.source_search_input.read(cx).text().trim().to_string();
        let current_input_lower = current_input.to_lowercase();

        const POPULAR_TITLES: &[&str] = &[
            "One Piece",
            "Naruto Shippuden",
            "Attack on Titan",
            "Demon Slayer: Kimetsu no Yaiba",
            "Jujutsu Kaisen",
            "My Hero Academia",
            "Death Note",
            "Fullmetal Alchemist: Brotherhood",
            "Bleach: Thousand-Year Blood War",
            "Hunter x Hunter",
            "Steins;Gate",
            "Chainsaw Man",
            "Spy x Family",
            "Solo Leveling",
            "Frieren: Beyond Journey's End",
            "Kaguya-sama: Love is War",
            "Your Name",
            "Spirited Away",
            "Cowboy Bebop",
            "Neon Genesis Evangelion",
            "Cyberpunk: Edgerunners",
            "Monster",
            "Code Geass: Lelouch of the Rebellion",
            "Mob Psycho 100",
            "Vinland Saga",
        ];

        // Filter titles
        let matched_titles: Vec<&str> = if current_input.is_empty() {
            POPULAR_TITLES.iter().take(8).cloned().collect()
        } else {
            POPULAR_TITLES
                .iter()
                .filter(|&&title| {
                    let title_lower = title.to_lowercase();
                    if title_lower.contains(&current_input_lower) {
                        return true;
                    }
                    let words: Vec<&str> = current_input_lower.split_whitespace().collect();
                    if !words.is_empty() && words.iter().all(|&word| title_lower.contains(word)) {
                        return true;
                    }
                    false
                })
                .take(10)
                .cloned()
                .collect()
        };

        div()
            .id("type-ahead-suggestions")
            .flex()
            .flex_col()
            .gap_4()
            .p_1()
            .child(
                // Titles Section
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED_TEXT))
                            .child("Popular Suggestions")
                    )
                    .child(
                        if matched_titles.is_empty() {
                            div().text_xs().text_color(rgb(MUTED_TEXT)).child("No matching popular titles.")
                        } else {
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .children(matched_titles.into_iter().map(|title| {
                                    div()
                                        .id(format!("suggest-{title}"))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .px_2()
                                        .py_2()
                                        .rounded_sm()
                                        .hover(|style| {
                                            style.bg(rgb(0x121212))
                                        })
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |player, _, window, cx| {
                                            player.select_suggestion(title, window, cx);
                                            cx.stop_propagation();
                                        }))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    svg()
                                                        .external_path(crate::icon_path(ICON_SEARCH))
                                                        .w(px(14.0))
                                                        .h(px(14.0))
                                                        .text_color(rgb(MUTED_TEXT)),
                                                )
                                                .child(title.to_string()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(MUTED_TEXT))
                                                .child("Suggest")
                                        )
                                }))
                        }
                    )
            )
    }

    fn search_source_providers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let source_providers = available_source_providers(&self.settings.source_providers);
        if source_providers.is_empty() {
            self.source_search_status =
                Some("No source provider set. Add one here to enable search.".into());
            window.focus(&self.source_provider_input.focus_handle(cx), cx);
            cx.notify();
            return;
        }

        let search_query = self.source_search_input.read(cx).text().trim().to_string();
        if search_query.is_empty() {
            self.source_search_status = Some("Enter a title to search.".into());
            window.focus(&self.source_search_input.focus_handle(cx), cx);
            cx.notify();
            return;
        }

        self.is_source_search_pending = true;
        self.source_search_results.clear();
        self.source_episode_results.clear();
        self.source_stream_results.clear();
        self.source_browser_view = SourceBrowserView::SearchResults;
        self.selected_source_result_index = 0;
        self.selected_source_series_title = None;
        self.selected_source_episode_title = None;
        self.selected_source_thumbnail_url = None;
        self.last_source_search_query = search_query.clone();
        self.source_search_status = Some("Searching source providers...".into());
        cx.notify();

        let search_task = cx.background_spawn(async move {
            search_configured_source_providers(source_providers, search_query)
        });

        cx.spawn_in(window, async move |this, cx| {
            let search_results = search_task.await;

            let _ = this.update_in(cx, |player, _window, cx| {
                player.is_source_search_pending = false;
                match search_results {
                    Ok(results) => {
                        let result_count = results.len();
                        player.source_search_results = results;
                        player.selected_source_result_index = 0;
                        player.scroll_selected_source_result_into_view();
                        player.source_search_status = Some(
                            if result_count == 0 {
                                "No source results found.".to_string()
                            } else {
                                format!("{result_count} source result(s).")
                            }
                            .into(),
                        );
                    }
                    Err(error_message) => {
                        player.source_search_results.clear();
                        player.source_search_status = Some(error_message.into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_source_search_result(
        &mut self,
        search_result: SourceSearchResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_source_thumbnail_url = search_result.thumbnail_url.clone();

        if let Some(mut direct_media) = search_result.direct_media.clone() {
            apply_source_media_metadata(
                &mut direct_media,
                &search_result.title,
                None,
                search_result.thumbnail_url.clone(),
            );
            self.load_internet_media(direct_media, window, cx);
            return;
        }

        if source_search_result_has_episode_step(&search_result) {
            self.fetch_source_episodes(search_result, window, cx);
            return;
        }

        if source_search_result_has_stream_step(&search_result) {
            self.fetch_source_streams_for_series(search_result, window, cx);
            return;
        }

        self.source_search_status =
            Some("This result has no playable stream or episode endpoint.".into());
        cx.notify();
    }

    fn load_source_episode_result(
        &mut self,
        episode_result: SourceEpisodeResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(mut direct_media) = episode_result.direct_media.clone() {
            apply_source_media_metadata(
                &mut direct_media,
                &episode_result.series_title,
                Some(&episode_result.title),
                self.selected_source_thumbnail_url.clone(),
            );
            self.load_internet_media(direct_media, window, cx);
            return;
        }

        if source_episode_result_has_stream_step(&episode_result) {
            self.fetch_source_streams_for_episode(episode_result, window, cx);
            return;
        }

        self.source_search_status = Some("This episode has no playable stream endpoint.".into());
        cx.notify();
    }

    fn load_source_stream_result(
        &mut self,
        stream_result: SourceStreamResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut internet_media = stream_result.media;
        if internet_media.thumbnail_url.is_none() {
            internet_media.thumbnail_url = self.selected_source_thumbnail_url.clone();
        }
        self.load_internet_media(internet_media, window, cx);
    }

    fn fetch_source_episodes(
        &mut self,
        search_result: SourceSearchResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(episodes_url) = source_search_result_episodes_url(&search_result) else {
            self.source_search_status = Some("This provider did not return an episode URL.".into());
            cx.notify();
            return;
        };
        let provider = search_result.provider.clone();
        let series_title = search_result.title.clone();

        self.is_source_search_pending = true;
        self.source_episode_results.clear();
        self.source_stream_results.clear();
        self.source_browser_view = SourceBrowserView::Episodes;
        self.selected_source_result_index = 0;
        self.selected_source_series_title = Some(series_title.clone());
        self.selected_source_episode_title = None;
        self.selected_source_thumbnail_url = search_result.thumbnail_url.clone();
        self.source_search_status = Some(format!("Loading episodes for {series_title}...").into());
        cx.notify();

        let episode_task = cx.background_spawn(async move {
            fetch_source_episode_results(&provider, &series_title, &episodes_url)
        });

        cx.spawn_in(window, async move |this, cx| {
            let episode_results = episode_task.await;

            let _ = this.update_in(cx, |player, _window, cx| {
                player.is_source_search_pending = false;
                match episode_results {
                    Ok(results) => {
                        let episode_count = results.len();
                        player.source_episode_results = results;
                        player.selected_source_result_index = 0;
                        player.scroll_selected_source_result_into_view();
                        player.source_search_status = Some(
                            if episode_count == 0 {
                                "No episodes found.".to_string()
                            } else {
                                format!("{episode_count} episode(s).")
                            }
                            .into(),
                        );
                    }
                    Err(error_message) => {
                        player.source_episode_results.clear();
                        player.source_search_status = Some(error_message.into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn fetch_source_streams_for_series(
        &mut self,
        search_result: SourceSearchResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(streams_url) = source_search_result_streams_url(&search_result) else {
            self.source_search_status = Some("This provider did not return a stream URL.".into());
            cx.notify();
            return;
        };
        let provider = search_result.provider.clone();
        let series_title = search_result.title.clone();

        self.fetch_source_streams(provider, series_title, None, streams_url, window, cx);
    }

    fn fetch_source_streams_for_episode(
        &mut self,
        episode_result: SourceEpisodeResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(streams_url) = source_episode_result_streams_url(&episode_result) else {
            self.source_search_status = Some("This provider did not return a stream URL.".into());
            cx.notify();
            return;
        };
        let provider = episode_result.provider.clone();
        let series_title = episode_result.series_title.clone();
        let episode_title = episode_result.title.clone();

        self.fetch_source_streams(
            provider,
            series_title,
            Some(episode_title),
            streams_url,
            window,
            cx,
        );
    }

    fn fetch_source_streams(
        &mut self,
        provider: SourceProvider,
        series_title: String,
        episode_title: Option<String>,
        streams_url: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let status_title = episode_title
            .clone()
            .unwrap_or_else(|| series_title.clone());
        self.is_source_search_pending = true;
        self.source_stream_results.clear();
        self.source_browser_view = SourceBrowserView::Streams;
        self.selected_source_result_index = 0;
        self.selected_source_series_title = Some(series_title.clone());
        self.selected_source_episode_title = episode_title.clone();
        self.source_search_status = Some(format!("Loading streams for {status_title}...").into());
        cx.notify();

        let stream_task = cx.background_spawn(async move {
            fetch_source_stream_results(
                &provider,
                &series_title,
                episode_title.as_deref(),
                &streams_url,
            )
        });

        cx.spawn_in(window, async move |this, cx| {
            let stream_results = stream_task.await;

            let _ = this.update_in(cx, |player, _window, cx| {
                player.is_source_search_pending = false;
                match stream_results {
                    Ok(mut results) => {
                        let thumbnail_url = player.selected_source_thumbnail_url.clone();
                        for stream_result in &mut results {
                            if stream_result.media.thumbnail_url.is_none() {
                                stream_result.media.thumbnail_url = thumbnail_url.clone();
                            }
                        }
                        let stream_count = results.len();
                        player.source_stream_results = results;
                        player.selected_source_result_index = 0;
                        player.scroll_selected_source_result_into_view();
                        player.source_search_status = Some(
                            if stream_count == 0 {
                                "No playable streams found.".to_string()
                            } else {
                                format!("{stream_count} stream source(s).")
                            }
                            .into(),
                        );
                    }
                    Err(error_message) => {
                        player.source_stream_results.clear();
                        player.source_search_status = Some(error_message.into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn show_source_search_results(&mut self, cx: &mut Context<Self>) {
        self.source_browser_view = SourceBrowserView::SearchResults;
        self.source_stream_results.clear();
        self.source_episode_results.clear();
        self.selected_source_result_index = 0;
        self.selected_source_series_title = None;
        self.selected_source_episode_title = None;
        self.selected_source_thumbnail_url = None;
        self.source_search_status = Some("Search source providers.".into());
        self.scroll_selected_source_result_into_view();
        cx.notify();
    }

    fn show_source_episode_results(&mut self, cx: &mut Context<Self>) {
        self.source_browser_view = SourceBrowserView::Episodes;
        self.source_stream_results.clear();
        self.selected_source_result_index = 0;
        self.selected_source_episode_title = None;
        self.source_search_status = Some("Select an episode.".into());
        self.scroll_selected_source_result_into_view();
        cx.notify();
    }

    fn load_internet_media(
        &mut self,
        internet_media: InternetMedia,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pending_watch_session = None;
        clear_saved_watch_session();
        self.record_current_media_progress();
        self.flush_player_library_if_due(true);
        self.stop_playback_process();
        self.playback_queue = vec![build_internet_media(internet_media.clone())];
        self.current_queue_index = Some(0);
        self.selected_audio_track_id = None;
        self.selected_subtitle_path = None;
        self.selected_embedded_subtitle_track_id = None;
        self.playback_position_seconds = 0.0;
        self.is_eof_reached = false;
        self.is_library_open = false;
        self.is_live_capture_menu_open = false;
        self.is_source_search_open = false;
        promote_recent_internet_media(
            &mut self.library.recent_internet_media,
            internet_media,
            MAX_RECENT_MEDIA,
        );
        self.mark_library_dirty();
        save_player_library_atomic(&self.library);
        self.schedule_internet_library_thumbnail_cache(window, cx);
        self.start_current_media_playback(window, cx);
        self.reveal_controls(window, cx);
    }

    fn show_library_context_menu(
        &mut self,
        media_path: PathBuf,
        anchor: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.library_context_menu_anchor = Some(anchor);
        self.library_context_menu_media_path = Some(media_path);
        self.library_context_menu_internet_media = None;
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.is_live_capture_menu_open = false;
        self.is_settings_modal_open = false;
        self.is_source_search_open = false;
        self.open_settings_selector = None;
        self.subtitle_menu_anchor = None;
        self.open_context_menu_section = None;
        cx.notify();
    }

    fn show_internet_library_context_menu(
        &mut self,
        internet_media: InternetMedia,
        anchor: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.library_context_menu_anchor = Some(anchor);
        self.library_context_menu_media_path = Some(internet_media_library_path(&internet_media));
        self.library_context_menu_internet_media = Some(internet_media);
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.is_live_capture_menu_open = false;
        self.is_settings_modal_open = false;
        self.is_source_search_open = false;
        self.open_settings_selector = None;
        self.subtitle_menu_anchor = None;
        self.open_context_menu_section = None;
        cx.notify();
    }

    fn close_library_context_menu(&mut self, cx: &mut Context<Self>) {
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.library_context_menu_internet_media = None;
        cx.notify();
    }

    fn toggle_context_menu_section(
        &mut self,
        section: ContextMenuSection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_context_menu_section = if self.open_context_menu_section == Some(section) {
            None
        } else {
            Some(section)
        };
        self.reveal_controls(window, cx);
    }

    fn continue_saved_watch_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(watch_session) = self.pending_watch_session.take() else {
            self.reveal_controls(window, cx);
            return;
        };

        self.load_watch_session(watch_session, window, cx);
        self.reveal_controls(window, cx);
    }

    fn decline_saved_watch_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending_watch_session = None;
        clear_saved_watch_session();
        self.is_library_open = true;
        self.are_controls_visible = false;
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.library_context_menu_internet_media = None;
        self.timeline_hover_preview = None;
        self.schedule_library_thumbnail_generation(window, cx);
        cx.notify();
    }

    fn reveal_library_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.playback_progress_generation += 1;
        self.record_current_media_progress();
        self.flush_player_library_if_due(true);
        self.save_current_watch_session();
        self.stop_playback_process();
        self.is_playing = false;
        self.is_eof_reached = false;
        self.is_library_open = true;
        self.are_controls_visible = false;
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.is_live_capture_menu_open = false;
        self.is_settings_modal_open = false;
        self.is_source_search_open = false;
        self.open_settings_selector = None;
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.library_context_menu_internet_media = None;
        self.open_context_menu_section = None;
        self.timeline_hover_preview = None;
        self.pending_timeline_thumbnail_key = None;
        self.is_timeline_scrubbing = false;
        self.last_timeline_seek_position_seconds = None;
        self.osd_message = None;
        self.status_message = Some("Library".into());
        self.schedule_library_thumbnail_generation(window, cx);
        cx.notify();
    }

    fn schedule_library_thumbnail_generation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.schedule_local_library_thumbnail_generation(window, cx);
        self.schedule_internet_library_thumbnail_cache(window, cx);
    }

    fn schedule_local_library_thumbnail_generation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ffmpeg_path) = self.dependency_status.ffmpeg_path.clone() else {
            return;
        };
        let media_paths = library_thumbnail_media_paths(&self.library)
            .into_iter()
            .filter(|media_path| !library_thumbnail_path(media_path).is_file())
            .take(MAX_LIBRARY_THUMBNAILS_TO_GENERATE)
            .collect::<Vec<_>>();

        if media_paths.is_empty() {
            return;
        }

        let thumbnail_task = cx.background_spawn(async move {
            generate_library_thumbnails_bounded(ffmpeg_path, media_paths);
            prune_thumbnail_cache();
        });

        cx.spawn_in(window, async move |this, cx| {
            thumbnail_task.await;
            let _ = this.update_in(cx, |_player, _window, cx| {
                cx.notify();
            });
        })
        .detach();
    }

    fn schedule_internet_library_thumbnail_cache(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let thumbnail_urls = internet_library_thumbnail_urls(&self.library)
            .into_iter()
            .filter(|thumbnail_url| existing_remote_thumbnail_path(thumbnail_url).is_none())
            .take(MAX_LIBRARY_THUMBNAILS_TO_GENERATE)
            .collect::<Vec<_>>();

        if thumbnail_urls.is_empty() {
            return;
        }

        let thumbnail_task = cx.background_spawn(async move {
            for thumbnail_url in thumbnail_urls {
                let _ = cache_remote_thumbnail(&thumbnail_url);
            }
            prune_thumbnail_cache();
        });

        cx.spawn_in(window, async move |this, cx| {
            thumbnail_task.await;
            let _ = this.update_in(cx, |_player, _window, cx| {
                cx.notify();
            });
        })
        .detach();
    }

    fn open_file_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.is_live_capture_menu_open = false;
        self.open_context_menu_section = None;
        self.reveal_controls(window, cx);
        let Some(media_path) = video_file_dialog("Play file").pick_file() else {
            return;
        };

        if is_video_path(&media_path) || is_playlist_path(&media_path) {
            self.load_media_paths(media_paths_from_path(media_path), window, cx);
        }
        self.reveal_controls(window, cx);
    }

    fn open_folder_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.is_live_capture_menu_open = false;
        self.open_context_menu_section = None;
        self.reveal_controls(window, cx);
        let path_selection = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Play folder".into()),
        });

        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = path_selection.await else {
                return;
            };

            let media_paths = paths
                .into_iter()
                .flat_map(|path| media_paths_in_folder(&path))
                .collect::<Vec<_>>();

            let _ = this.update_in(cx, |player, window, cx| {
                player.load_media_paths(media_paths, window, cx);
                player.reveal_controls(window, cx);
            });
        })
        .detach();
    }

    fn open_queue_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.is_live_capture_menu_open = false;
        self.open_context_menu_section = None;
        self.reveal_controls(window, cx);
        let path_selection = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: true,
            prompt: Some("Build play queue".into()),
        });

        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = path_selection.await else {
                return;
            };

            let media_paths = paths
                .into_iter()
                .flat_map(media_paths_from_path)
                .collect::<Vec<_>>();

            let _ = this.update_in(cx, |player, window, cx| {
                player.load_media_paths(media_paths, window, cx);
                player.reveal_controls(window, cx);
            });
        })
        .detach();
    }

    fn toggle_live_capture_device_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_live_capture_menu_open = !self.is_live_capture_menu_open;
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.is_settings_modal_open = false;
        self.open_settings_selector = None;
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.library_context_menu_internet_media = None;
        self.open_context_menu_section = None;

        if self.is_live_capture_menu_open && self.live_capture_devices.is_empty() {
            self.refresh_live_capture_devices(window, cx);
        }

        cx.notify();
    }

    fn refresh_live_capture_devices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_live_capture_scan_pending {
            return;
        }

        let Some(ffmpeg_path) = self.dependency_status.ffmpeg_path.clone() else {
            self.status_message = Some("FFmpeg is required to list live capture devices.".into());
            self.is_live_capture_menu_open = true;
            cx.notify();
            return;
        };

        self.is_live_capture_scan_pending = true;
        self.status_message = Some("Scanning live capture devices...".into());
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let device_scan_result = list_live_capture_devices(&ffmpeg_path);

            let _ = this.update_in(cx, |player, _window, cx| {
                player.is_live_capture_scan_pending = false;
                match device_scan_result {
                    Ok(device_scan) => {
                        let device_count = device_scan.video_devices.len();
                        let audio_device_count = device_scan.audio_devices.len();
                        player.live_capture_devices = device_scan.video_devices;
                        player.live_capture_audio_devices = device_scan.audio_devices;
                        player.status_message = Some(
                            if device_count == 0 {
                                "No live capture devices were found.".to_string()
                            } else {
                                format!(
                                    "Found {device_count} live capture device(s) and {audio_device_count} audio source(s)."
                                )
                            }
                            .into(),
                        );
                    }
                    Err(error_message) => {
                        player.live_capture_devices.clear();
                        player.live_capture_audio_devices.clear();
                        player.status_message = Some(error_message.into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_live_capture_device(
        &mut self,
        device: LiveCaptureDevice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let device = self.live_capture_device_with_selected_audio_source(device);
        self.pending_watch_session = None;
        clear_saved_watch_session();
        self.record_current_media_progress();
        self.flush_player_library_if_due(true);
        self.stop_playback_process();
        self.playback_queue = vec![build_live_capture_media(device)];
        self.current_queue_index = Some(0);
        self.selected_audio_track_id = None;
        self.selected_subtitle_path = None;
        self.selected_embedded_subtitle_track_id = None;
        self.playback_position_seconds = 0.0;
        self.is_eof_reached = false;
        self.is_library_open = false;
        self.is_live_capture_menu_open = false;
        self.start_current_media_playback(window, cx);
        self.reveal_controls(window, cx);
    }

    fn live_capture_device_with_selected_audio_source(
        &self,
        mut device: LiveCaptureDevice,
    ) -> LiveCaptureDevice {
        match self.settings.live_capture_audio_source.as_str() {
            LIVE_CAPTURE_AUDIO_SOURCE_NONE => {
                device.audio_backend_name = None;
                device.audio_pin_name = None;
            }
            LIVE_CAPTURE_AUDIO_SOURCE_AUTO => {}
            selected_audio_backend_name => {
                device.audio_backend_name = Some(selected_audio_backend_name.to_string());
                device.audio_pin_name = None;
            }
        }

        device
    }

    fn set_live_capture_audio_source(
        &mut self,
        audio_source: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.live_capture_audio_source = audio_source;
        save_player_settings(&self.settings);
        self.status_message = Some(
            format!(
                "Live capture audio source: {}",
                self.live_capture_audio_source_label()
            )
            .into(),
        );

        if self.is_current_media_live_capture() {
            self.restart_current_live_capture_with_current_settings(window, cx);
        } else {
            cx.notify();
        }
    }

    fn live_capture_audio_source_label(&self) -> String {
        live_capture_audio_source_label(
            &self.settings.live_capture_audio_source,
            &self.live_capture_audio_devices,
        )
    }

    fn refresh_audio_output_devices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_audio_output_scan_pending {
            return;
        }

        let Some(mpv_path) = self.dependency_status.mpv_path.clone() else {
            return;
        };

        self.is_audio_output_scan_pending = true;
        cx.spawn_in(window, async move |this, cx| {
            let audio_output_devices = list_audio_output_devices(&mpv_path);

            let _ = this.update_in(cx, |player, _window, cx| {
                player.audio_output_devices = audio_output_devices;
                player.is_audio_output_scan_pending = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn audio_output_device_label(&self, audio_output_device: &str) -> String {
        self.audio_output_devices
            .iter()
            .find(|device_option| device_option.device_id == audio_output_device)
            .map(|device_option| device_option.label.clone())
            .unwrap_or_else(|| "Custom output".to_string())
    }

    fn restart_current_live_capture_with_current_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(current_queue_index) = self.current_queue_index else {
            return;
        };
        let Some(current_device) = self
            .playback_queue
            .get(current_queue_index)
            .and_then(LoadedMedia::live_capture_device)
            .cloned()
        else {
            return;
        };
        let updated_device = self.live_capture_device_with_selected_audio_source(
            current_device.with_latency_mode(LiveCaptureLatencyMode::UltraLow),
        );

        if let Some(current_media) = self.playback_queue.get_mut(current_queue_index) {
            current_media.source = LoadedMediaSource::LiveCapture(updated_device);
        }
        self.start_current_media_playback(window, cx);
    }

    fn load_media_paths(
        &mut self,
        media_paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pending_watch_session = None;
        clear_saved_watch_session();
        let media_paths = normalize_media_paths(media_paths);

        if media_paths.is_empty() {
            self.stop_playback_process();
            self.playback_queue.clear();
            self.current_queue_index = None;
            self.selected_audio_track_id = None;
            self.selected_subtitle_path = None;
            self.selected_embedded_subtitle_track_id = None;
            self.is_playing = false;
            self.is_library_open = true;
            self.timeline_hover_preview = None;
            self.pending_timeline_thumbnail_key = None;
            self.status_message = Some("No supported video files were selected.".into());
            return;
        }

        self.record_current_media_progress();
        self.flush_player_library_if_due(true);
        self.playback_queue = media_paths
            .into_iter()
            .map(|path| build_loaded_media(&path, &self.dependency_status))
            .collect();
        self.current_queue_index = Some(0);
        self.select_initial_tracks_for_queue_index(0);
        self.restore_track_preferences_for_current_media();
        self.is_library_open = false;
        self.is_live_capture_menu_open = false;
        self.remember_current_queue_in_library();
        self.start_current_media_playback(window, cx);
    }

    fn load_watch_session(
        &mut self,
        watch_session: SavedWatchSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let media_paths = watch_session
            .media_paths
            .into_iter()
            .filter(|path| path.is_file() && is_video_path(path))
            .collect::<Vec<_>>();
        let mut media_paths = normalize_media_paths(media_paths);

        if watch_session.current_media_path.is_file()
            && is_video_path(&watch_session.current_media_path)
            && !media_paths
                .iter()
                .any(|media_path| media_path == &watch_session.current_media_path)
        {
            media_paths.push(watch_session.current_media_path.clone());
        }

        if media_paths.is_empty() {
            self.stop_playback_process();
            self.playback_queue.clear();
            self.current_queue_index = None;
            self.is_playing = false;
            self.is_library_open = true;
            self.timeline_hover_preview = None;
            self.pending_timeline_thumbnail_key = None;
            self.status_message = Some("The saved media file could not be found.".into());
            clear_saved_watch_session();
            return;
        }

        self.record_current_media_progress();
        self.flush_player_library_if_due(true);
        self.playback_queue = media_paths
            .into_iter()
            .map(|path| build_loaded_media(&path, &self.dependency_status))
            .collect();

        let fallback_queue_index = watch_session
            .current_queue_index
            .min(self.playback_queue.len().saturating_sub(1));
        let resume_queue_index = self
            .playback_queue
            .iter()
            .position(|media| {
                media
                    .file_path()
                    .is_some_and(|path| path == watch_session.current_media_path)
            })
            .unwrap_or(fallback_queue_index);

        self.current_queue_index = Some(resume_queue_index);
        self.volume_percent = watch_session.volume_percent.min(100);
        self.is_muted = watch_session.is_muted;
        self.select_initial_tracks_for_queue_index(resume_queue_index);
        self.restore_track_preferences_for_current_media();
        self.is_library_open = false;
        self.is_live_capture_menu_open = false;
        self.remember_current_queue_in_library();
        self.start_current_media_playback_at_position(
            watch_session.playback_position_seconds,
            window,
            cx,
        );
    }

    fn select_initial_tracks_for_queue_index(&mut self, queue_index: usize) {
        let Some(media) = self.playback_queue.get(queue_index) else {
            self.selected_audio_track_id = None;
            self.selected_subtitle_path = None;
            self.selected_embedded_subtitle_track_id = None;
            return;
        };

        self.selected_audio_track_id =
            preferred_audio_track_id(&media.audio_tracks, &self.settings.preferred_audio_language)
                .or_else(|| media.audio_tracks.first().map(|track| track.track_id));

        if self.settings.prefer_embedded_subtitles {
            self.selected_embedded_subtitle_track_id = preferred_embedded_subtitle_track_id(
                &media.embedded_subtitle_tracks,
                &self.settings.preferred_subtitle_language,
            );
            self.selected_subtitle_path = None;

            if self.selected_embedded_subtitle_track_id.is_some() {
                return;
            }
        }

        self.selected_subtitle_path = preferred_sidecar_subtitle_path(
            &media.subtitle_paths,
            &self.settings.preferred_subtitle_language,
        )
        .or_else(|| media.subtitle_paths.first().cloned());
        self.selected_embedded_subtitle_track_id = None;
    }

    fn restore_track_preferences_for_current_media(&mut self) {
        let Some(current_media_path) = self.current_media_path() else {
            return;
        };
        let current_media_path_key = library_media_path_key(&current_media_path);
        let Some(history_entry) = self
            .library
            .media_history
            .iter()
            .find(|entry| library_media_path_key(&entry.path) == current_media_path_key)
            .cloned()
        else {
            return;
        };

        self.selected_audio_track_id = history_entry.selected_audio_track_id;
        self.selected_embedded_subtitle_track_id =
            history_entry.selected_embedded_subtitle_track_id;
        self.selected_subtitle_path = history_entry.selected_subtitle_path;
    }

    fn select_embedded_subtitle_track(
        &mut self,
        track_id: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_subtitle_path = None;
        self.selected_embedded_subtitle_track_id = Some(track_id);
        self.send_mpv_command(json!(["set_property", "sid", track_id]));
        self.is_subtitle_menu_open = false;
        self.show_osd(
            format!("Subtitles: {}", self.selected_subtitle_label()),
            window,
            cx,
        );
        self.reveal_controls(window, cx);
    }

    fn select_audio_track(&mut self, track_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_audio_track_id = Some(track_id);
        self.send_mpv_command(json!(["set_property", "aid", track_id]));
        self.is_subtitle_menu_open = false;
        self.open_context_menu_section = None;
        self.show_osd(
            format!("Audio: {}", self.selected_audio_track_label()),
            window,
            cx,
        );
        self.reveal_controls(window, cx);
    }

    fn disable_subtitles(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_subtitle_path = None;
        self.selected_embedded_subtitle_track_id = None;
        self.send_mpv_command(json!(["set_property", "sid", "no"]));
        self.is_subtitle_menu_open = false;
        self.show_osd("Subtitles off".to_string(), window, cx);
        self.reveal_controls(window, cx);
    }

    fn set_volume_percent(
        &mut self,
        volume_percent: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.volume_percent = volume_percent.min(100);
        self.send_mpv_command(json!(["set_property", "volume", self.volume_percent]));
        self.show_osd(format!("Volume {}%", self.volume_percent), window, cx);
        self.reveal_controls(window, cx);
    }

    fn set_volume_percent_if_changed(
        &mut self,
        volume_percent: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let volume_percent = volume_percent.min(100);
        if self.volume_percent == volume_percent {
            return;
        }

        self.set_volume_percent(volume_percent, window, cx);
    }

    fn move_queue_item_up(
        &mut self,
        queue_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if queue_index == 0 || queue_index >= self.playback_queue.len() {
            self.reveal_controls(window, cx);
            return;
        }

        self.playback_queue.swap(queue_index, queue_index - 1);
        self.current_queue_index = self.current_queue_index.map(|current_index| {
            if current_index == queue_index {
                queue_index - 1
            } else if current_index == queue_index - 1 {
                queue_index
            } else {
                current_index
            }
        });
        self.save_current_watch_session();
        self.reveal_controls(window, cx);
    }

    fn move_queue_item_down(
        &mut self,
        queue_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next_queue_index = queue_index + 1;
        if next_queue_index >= self.playback_queue.len() {
            self.reveal_controls(window, cx);
            return;
        }

        self.playback_queue.swap(queue_index, next_queue_index);
        self.current_queue_index = self.current_queue_index.map(|current_index| {
            if current_index == queue_index {
                next_queue_index
            } else if current_index == next_queue_index {
                queue_index
            } else {
                current_index
            }
        });
        self.save_current_watch_session();
        self.reveal_controls(window, cx);
    }

    fn play_previous_queue_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(current_index) = self.current_queue_index {
            self.play_queue_item(current_index.saturating_sub(1), window, cx);
        } else {
            self.reveal_controls(window, cx);
        }
    }

    fn play_next_queue_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(next_queue_index) = self.next_queue_index_for_playback(false) {
            self.play_queue_item(next_queue_index, window, cx);
            return;
        }

        self.show_osd("End of queue".to_string(), window, cx);
        self.reveal_controls(window, cx);
    }

    fn rebuild_shuffle_bag(&mut self) {
        use rand::seq::SliceRandom;

        let Some(current_index) = self.current_queue_index else {
            self.shuffle_bag.clear();
            return;
        };

        self.shuffle_bag = (0..self.playback_queue.len())
            .filter(|index| *index != current_index)
            .collect();
        self.shuffle_bag.shuffle(&mut rand::thread_rng());
    }

    fn next_queue_index_for_playback(&mut self, is_from_eof: bool) -> Option<usize> {
        let current_queue_index = self.current_queue_index?;
        let queue_len = self.playback_queue.len();

        if queue_len == 0 {
            return None;
        }
        if queue_len == 1 {
            return (self.repeat_mode == PlaybackRepeatMode::All || !is_from_eof)
                .then_some(current_queue_index);
        }
        if self.is_shuffle_enabled {
            if self.shuffle_bag.is_empty() {
                self.rebuild_shuffle_bag();
            }
            return self.shuffle_bag.pop();
        }

        let next_queue_index = current_queue_index + 1;
        if next_queue_index < queue_len {
            Some(next_queue_index)
        } else if self.repeat_mode == PlaybackRepeatMode::All || !is_from_eof {
            Some(0)
        } else {
            None
        }
    }

    fn play_queue_item_next(
        &mut self,
        queue_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(current_queue_index) = self.current_queue_index else {
            self.play_queue_item(queue_index, window, cx);
            return;
        };

        if queue_index >= self.playback_queue.len() || queue_index == current_queue_index {
            self.reveal_controls(window, cx);
            return;
        }

        let media = self.playback_queue.remove(queue_index);
        let insertion_index = if queue_index < current_queue_index {
            current_queue_index
        } else {
            current_queue_index + 1
        }
        .min(self.playback_queue.len());
        self.playback_queue.insert(insertion_index, media);
        self.current_queue_index = Some(if queue_index < current_queue_index {
            current_queue_index - 1
        } else {
            current_queue_index
        });
        self.save_current_watch_session();
        self.show_osd("Queued to play next".to_string(), window, cx);
        self.reveal_controls(window, cx);
    }

    fn play_queue_item(&mut self, queue_index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if queue_index < self.playback_queue.len() {
            self.record_current_media_progress();
            self.flush_player_library_if_due(true);
            self.current_queue_index = Some(queue_index);
            self.select_initial_tracks_for_queue_index(queue_index);
            self.restore_track_preferences_for_current_media();
            self.is_library_open = false;
            self.start_current_media_playback(window, cx);
        }
        self.reveal_controls(window, cx);
    }

    fn start_current_media_playback(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_current_media_playback_at_position(0.0, window, cx);
    }

    fn start_current_media_playback_at_position(
        &mut self,
        start_position_seconds: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_playback_process();
        self.playback_progress_generation += 1;

        let Some(current_media) = self.current_media().cloned() else {
            self.playback_position_seconds = 0.0;
            self.is_playing = false;
            self.status_message = Some("No media is selected.".into());
            return;
        };
        let Some(mpv_path) = self.dependency_status.mpv_path.clone() else {
            self.playback_position_seconds = 0.0;
            self.is_playing = false;
            self.status_message = self
                .dependency_status
                .setup_message()
                .map(Into::into)
                .or_else(|| Some("mpv was not found.".into()));
            return;
        };

        let clamped_start_position_seconds = if current_media.is_seekable_file() {
            self.current_media_duration_seconds()
                .map(|duration_seconds| start_position_seconds.clamp(0.0, duration_seconds))
                .unwrap_or_else(|| start_position_seconds.max(0.0))
        } else {
            0.0
        };
        self.playback_position_seconds = clamped_start_position_seconds;
        self.is_eof_reached = false;

        let Some(video_host_window_id) = self.ensure_video_host_window(window) else {
            self.is_playing = false;
            self.status_message =
                Some("Could not create the embedded video surface for this window.".into());
            return;
        };

        let ipc_path = create_playback_ipc_path();
        let log_path = create_playback_log_path();
        self.position_video_host_for_current_window(window, true);

        let mut command = Command::new(mpv_path);
        command
            .arg(format!("--wid={video_host_window_id}"))
            .arg(format!("--input-ipc-server={ipc_path}"))
            .arg(format!("--log-file={}", log_path.display()))
            .arg("--force-window=yes")
            .arg("--no-border")
            .arg("--no-osc")
            .arg("--keep-open=yes")
            .arg("--show-in-taskbar=no")
            .arg("--taskbar-progress=no")
            .arg("--title=Watch Playback Backend")
            .arg("--no-terminal")
            .arg("--really-quiet")
            .arg(format!("--hwdec={}", self.settings.hardware_decoding_mode))
            .arg(format!(
                "--audio-device={}",
                self.settings.audio_output_device
            ))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        hide_child_process_window(&mut command);

        if current_media.is_seekable_file() && clamped_start_position_seconds > 0.0 {
            command.arg(format!("--start={clamped_start_position_seconds:.3}"));
        }
        current_media.append_mpv_input_args(&mut command, &self.settings);

        match command.spawn() {
            Ok(child) => {
                self.playback_process = Some(child);
                self.playback_ipc_path = Some(ipc_path);
                self.playback_log_path = Some(log_path);
                self.is_playing = true;
                self.status_message = None;
                self.configure_playback_backend_after_start(clamped_start_position_seconds);
                self.schedule_playback_state_poll(window, cx);
            }
            Err(error) => {
                self.playback_process = None;
                self.playback_ipc_path = None;
                self.playback_log_path = None;
                self.is_playing = false;
                self.status_message =
                    Some(format!("Could not start mpv playback backend: {error}").into());
            }
        }
    }

    fn ensure_video_host_window(&mut self, window: &Window) -> Option<isize> {
        if self.video_host_window.is_none() {
            let parent_window_id = native_window_id(window)?;
            self.video_host_window = create_embedded_video_host(parent_window_id);
        }

        self.video_host_window
            .as_ref()
            .and_then(EmbeddedVideoHost::window_id)
    }

    fn position_video_host_for_current_window(&self, window: &Window, is_visible: bool) {
        let Some((video_host_window_id, parent_window_id)) = self
            .video_host_window
            .as_ref()
            .and_then(EmbeddedVideoHost::window_ids)
        else {
            return;
        };

        let viewport_size = window.viewport_size();
        let video_bounds = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: viewport_size,
        };

        position_embedded_video_host(
            video_host_window_id,
            parent_window_id,
            video_bounds,
            window.scale_factor(),
            is_visible,
        );
    }

    fn stop_playback_process(&mut self) {
        if let Some(mut process) = self.playback_process.take() {
            let _ = self.send_mpv_command(json!(["quit"]));
            let _ = process.kill();
            let _ = process.wait();
        }
        if let Some(video_host_window_id) = self
            .video_host_window
            .as_ref()
            .and_then(EmbeddedVideoHost::window_id)
        {
            set_embedded_video_host_visible(video_host_window_id, false);
        }
        self.playback_ipc_path = None;
        self.playback_log_path = None;
    }

    fn send_mpv_command(&self, command: Value) -> bool {
        let Some(ipc_path) = self.playback_ipc_path.as_ref() else {
            return false;
        };

        send_mpv_ipc_command(ipc_path, command)
    }

    fn load_selected_subtitle_in_backend(&mut self) {
        if let Some(track_id) = self.selected_embedded_subtitle_track_id {
            self.send_mpv_command(json!(["set_property", "sid", track_id]));
            return;
        }

        let Some(subtitle_path) = self.selected_subtitle_path.clone() else {
            return;
        };

        self.send_mpv_command(json!([
            "sub-add",
            subtitle_path.display().to_string(),
            "select"
        ]));
    }

    fn configure_playback_backend_after_start(&self, start_position_seconds: f64) {
        let Some(ipc_path) = self.playback_ipc_path.clone() else {
            return;
        };
        let volume_percent = self.volume_percent;
        let is_muted = self.is_muted;
        let selected_audio_track_id = self.selected_audio_track_id;
        let selected_embedded_subtitle_track_id = self.selected_embedded_subtitle_track_id;
        let selected_subtitle_path = self.selected_subtitle_path.clone();
        let subtitle_font_size = self.settings.subtitle_font_size;
        let subtitle_color = self.settings.subtitle_color.clone();
        let subtitle_position_percent = self.settings.subtitle_position_percent;
        let subtitle_delay_seconds = self.subtitle_delay_ms as f64 / 1000.0;
        let playback_speed = self.playback_speed;

        let _ = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(80));
            send_mpv_ipc_command(&ipc_path, json!(["set_property", "volume", volume_percent]));
            send_mpv_ipc_command(&ipc_path, json!(["set_property", "mute", is_muted]));
            if let Some(audio_track_id) = selected_audio_track_id {
                send_mpv_ipc_command(&ipc_path, json!(["set_property", "aid", audio_track_id]));
            }
            if let Some(track_id) = selected_embedded_subtitle_track_id {
                send_mpv_ipc_command(&ipc_path, json!(["set_property", "sid", track_id]));
            } else if let Some(subtitle_path) = selected_subtitle_path {
                send_mpv_ipc_command(
                    &ipc_path,
                    json!(["sub-add", subtitle_path.display().to_string(), "select"]),
                );
            }
            send_mpv_ipc_command(
                &ipc_path,
                json!(["set_property", "sub-font-size", subtitle_font_size]),
            );
            send_mpv_ipc_command(
                &ipc_path,
                json!(["set_property", "sub-color", subtitle_color]),
            );
            send_mpv_ipc_command(
                &ipc_path,
                json!(["set_property", "sub-pos", subtitle_position_percent]),
            );
            send_mpv_ipc_command(
                &ipc_path,
                json!(["set_property", "sub-delay", subtitle_delay_seconds]),
            );
            send_mpv_ipc_command(&ipc_path, json!(["set_property", "speed", playback_speed]));
            if start_position_seconds > 0.0 {
                send_mpv_ipc_command(
                    &ipc_path,
                    json!(["seek", start_position_seconds, "absolute+exact"]),
                );
            }
        });
    }

    fn apply_subtitle_style_in_backend(&self) {
        self.send_mpv_command(json!([
            "set_property",
            "sub-font-size",
            self.settings.subtitle_font_size
        ]));
        self.send_mpv_command(json!([
            "set_property",
            "sub-color",
            self.settings.subtitle_color
        ]));
        self.send_mpv_command(json!([
            "set_property",
            "sub-pos",
            self.settings.subtitle_position_percent
        ]));
    }

    fn cycle_subtitle_size(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.subtitle_font_size = match self.settings.subtitle_font_size {
            0..=42 => 48,
            43..=54 => 60,
            55..=66 => 72,
            _ => 42,
        };
        save_player_settings(&self.settings);
        self.apply_subtitle_style_in_backend();
        self.show_osd(
            format!("Subtitle size {}", self.settings.subtitle_font_size),
            window,
            cx,
        );
        self.reveal_controls(window, cx);
    }

    fn cycle_subtitle_color(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.subtitle_color = match self.settings.subtitle_color.as_str() {
            "#FFFFFF" => "#FFD27A".to_string(),
            "#FFD27A" => "#80D8FF".to_string(),
            "#80D8FF" => "#B6FFB0".to_string(),
            _ => "#FFFFFF".to_string(),
        };
        save_player_settings(&self.settings);
        self.apply_subtitle_style_in_backend();
        self.show_osd(
            format!("Subtitle color {}", self.settings.subtitle_color),
            window,
            cx,
        );
        self.reveal_controls(window, cx);
    }

    fn cycle_subtitle_position(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.subtitle_position_percent = match self.settings.subtitle_position_percent {
            0..=84 => 90,
            85..=92 => 95,
            93..=97 => 100,
            _ => 85,
        };
        save_player_settings(&self.settings);
        self.apply_subtitle_style_in_backend();
        self.show_osd(
            format!(
                "Subtitle position {}%",
                self.settings.subtitle_position_percent
            ),
            window,
            cx,
        );
        self.reveal_controls(window, cx);
    }

    fn open_subtitle_search_hook(&mut self, cx: &mut Context<Self>) {
        let Some(media_path) = self.current_media_path() else {
            self.status_message = Some("Load media before searching subtitles.".into());
            cx.notify();
            return;
        };
        let query = url_encode(&searchable_media_title(&media_path));
        let search_url =
            format!("https://www.opensubtitles.org/en/search2/sublanguageid-all/moviename-{query}");
        cx.open_url(&search_url);
        self.status_message = Some("Opened subtitle search in your browser.".into());
        cx.notify();
    }

    fn open_subtitle_file_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(subtitle_path) = rfd::FileDialog::new()
            .set_title("Load subtitle")
            .add_filter("Subtitle files", &SUBTITLE_EXTENSIONS)
            .pick_file()
        else {
            return;
        };

        if !is_subtitle_path(&subtitle_path) {
            self.status_message = Some("Unsupported subtitle file.".into());
            cx.notify();
            return;
        }

        self.selected_subtitle_path = Some(subtitle_path.clone());
        self.selected_embedded_subtitle_track_id = None;

        if let Some(current_queue_index) = self.current_queue_index {
            if let Some(current_media) = self.playback_queue.get_mut(current_queue_index) {
                if !current_media.subtitle_paths.contains(&subtitle_path) {
                    current_media.subtitle_paths.push(subtitle_path);
                }
            }
        }

        self.load_selected_subtitle_in_backend();
        self.show_osd("Subtitle loaded".to_string(), window, cx);
        self.reveal_controls(window, cx);
    }

    fn load_library_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let media_paths = if path.is_dir() {
            media_paths_in_folder_cached(&path, &mut self.folder_listing_cache)
        } else {
            media_paths_from_path(path)
        };
        self.load_media_paths(media_paths, window, cx);
        self.reveal_controls(window, cx);
    }

    fn continue_library_entry(
        &mut self,
        history_entry: MediaHistoryEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let media_paths = history_entry
            .path
            .parent()
            .map(|folder_path| {
                media_paths_in_folder_cached(folder_path, &mut self.folder_listing_cache)
            })
            .filter(|paths| !paths.is_empty())
            .unwrap_or_else(|| vec![history_entry.path.clone()]);
        self.load_watch_session(
            SavedWatchSession {
                media_paths,
                current_media_path: history_entry.path,
                current_queue_index: 0,
                playback_position_seconds: history_entry.playback_position_seconds,
                volume_percent: self.volume_percent,
                is_muted: self.is_muted,
            },
            window,
            cx,
        );
        self.reveal_controls(window, cx);
    }

    fn save_settings_and_show(
        &mut self,
        message: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        save_player_settings(&self.settings);
        self.show_osd(message, window, cx);
        self.reveal_controls(window, cx);
    }

    fn save_current_window_bounds(&mut self, window: &Window) {
        let viewport_size = window.viewport_size();
        self.settings.window_bounds = Some(SavedWindowBounds {
            width: viewport_size.width.as_f32(),
            height: viewport_size.height.as_f32(),
            x: None,
            y: None,
        });
        save_player_settings(&self.settings);
    }

    fn save_current_window_bounds_if_due(&mut self, window: &Window) {
        if window.is_fullscreen() {
            return;
        }

        let now = current_time_millis();
        if now.saturating_sub(self.last_window_bounds_flush_millis) < LIBRARY_FLUSH_INTERVAL_MS {
            return;
        }

        self.last_window_bounds_flush_millis = now;
        self.save_current_window_bounds(window);
    }

    fn set_default_volume_percent(&mut self, default_volume_percent: u8, cx: &mut Context<Self>) {
        self.settings.default_volume_percent = default_volume_percent.min(100);
        save_player_settings(&self.settings);
        self.status_message =
            Some(format!("Default volume {}%", self.settings.default_volume_percent).into());
        cx.notify();
    }

    #[allow(dead_code)]
    fn open_settings_selector(
        &mut self,
        selector_kind: SettingsSelectorKind,
        cx: &mut Context<Self>,
    ) {
        self.open_settings_selector = Some(selector_kind);
        cx.notify();
    }

    fn close_settings_selector(&mut self, cx: &mut Context<Self>) {
        self.open_settings_selector = None;
        cx.notify();
    }

    fn apply_settings_selector_choice(
        &mut self,
        selector_choice: SettingsSelectorChoice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings_selector = None;

        match selector_choice {
            SettingsSelectorChoice::AudioOutputDevice(audio_output_device) => {
                self.set_audio_output_device(audio_output_device, window, cx);
            }
            SettingsSelectorChoice::ResumeBehavior(resume_behavior) => {
                self.set_resume_behavior_setting(resume_behavior, window, cx);
            }
            SettingsSelectorChoice::SeekStepSeconds(seek_step_seconds) => {
                self.settings.seek_step_seconds = seek_step_seconds;
                self.save_settings_and_show(
                    format!("Seek step {} seconds", seek_step_seconds),
                    window,
                    cx,
                );
            }
            SettingsSelectorChoice::VolumeStepPercent(volume_step_percent) => {
                self.settings.volume_step_percent = volume_step_percent;
                self.save_settings_and_show(
                    format!("Volume step {}%", volume_step_percent),
                    window,
                    cx,
                );
            }
            SettingsSelectorChoice::PreferredAudioLanguage(preferred_audio_language) => {
                self.set_preferred_audio_language_setting(preferred_audio_language, window, cx);
            }
            SettingsSelectorChoice::PreferredSubtitleLanguage(preferred_subtitle_language) => {
                self.set_preferred_subtitle_language_setting(
                    preferred_subtitle_language,
                    window,
                    cx,
                );
            }
            SettingsSelectorChoice::HardwareDecodingMode(hardware_decoding_mode) => {
                self.set_hardware_decoding_setting(hardware_decoding_mode, window, cx);
            }
            SettingsSelectorChoice::StartFullscreen(is_enabled) => {
                self.set_start_fullscreen_setting(is_enabled, window, cx);
            }
            SettingsSelectorChoice::LiveLowestLatency(is_enabled) => {
                self.set_lowest_latency_live_capture_setting(is_enabled, window, cx);
            }
            SettingsSelectorChoice::LiveCaptureExclusiveAudio(is_enabled) => {
                self.set_live_capture_exclusive_audio_setting(is_enabled, window, cx);
            }
            SettingsSelectorChoice::BackdropBlur(is_enabled) => {
                self.set_backdrop_blur_setting(is_enabled, window, cx);
            }
        }
    }

    fn set_audio_output_device(
        &mut self,
        audio_output_device: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.audio_output_device = audio_output_device.clone();
        save_player_settings(&self.settings);
        self.send_mpv_command(json!(["set_property", "audio-device", audio_output_device]));
        self.open_settings_selector = None;
        self.save_settings_and_show(
            format!(
                "Audio output {}",
                self.audio_output_device_label(&self.settings.audio_output_device)
            ),
            window,
            cx,
        );
    }

    fn set_resume_behavior_setting(
        &mut self,
        resume_behavior: ResumeBehavior,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.resume_behavior = resume_behavior;
        if self.settings.resume_behavior == ResumeBehavior::Never {
            self.pending_watch_session = None;
            clear_saved_watch_session();
        }
        self.save_settings_and_show(
            format!("Resume {}", self.settings.resume_behavior.label()),
            window,
            cx,
        );
    }

    fn set_preferred_audio_language_setting(
        &mut self,
        preferred_audio_language: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.preferred_audio_language = preferred_audio_language;
        self.save_settings_and_show(
            format!("Audio language {}", self.settings.preferred_audio_language),
            window,
            cx,
        );
    }

    fn set_preferred_subtitle_language_setting(
        &mut self,
        preferred_subtitle_language: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.preferred_subtitle_language = preferred_subtitle_language;
        self.save_settings_and_show(
            format!(
                "Subtitle language {}",
                self.settings.preferred_subtitle_language
            ),
            window,
            cx,
        );
    }

    fn set_hardware_decoding_setting(
        &mut self,
        hardware_decoding_mode: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.hardware_decoding_mode = hardware_decoding_mode;
        self.save_settings_and_show(
            format!("Hardware decode {}", self.settings.hardware_decoding_mode),
            window,
            cx,
        );
    }

    fn set_start_fullscreen_setting(
        &mut self,
        is_enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.start_fullscreen = is_enabled;
        self.save_settings_and_show(
            format!("Start fullscreen {}", on_off_message_word(is_enabled)),
            window,
            cx,
        );
    }

    fn set_lowest_latency_live_capture_setting(
        &mut self,
        is_enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.is_lowest_latency_live_capture_enabled = is_enabled;

        if self.is_current_media_live_capture() {
            self.restart_current_live_capture_with_current_settings(window, cx);
        }

        self.save_settings_and_show(
            format!(
                "Live capture lowest latency {}",
                on_off_message_word(is_enabled)
            ),
            window,
            cx,
        );
    }

    fn set_live_capture_exclusive_audio_setting(
        &mut self,
        is_enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.is_live_capture_exclusive_audio_enabled = is_enabled;

        if self.is_current_media_live_capture() {
            self.restart_current_live_capture_with_current_settings(window, cx);
        }

        self.save_settings_and_show(
            format!(
                "Live capture exclusive audio {}",
                on_off_message_word(is_enabled)
            ),
            window,
            cx,
        );
    }

    fn set_backdrop_blur_setting(
        &mut self,
        is_enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.is_backdrop_blur_enabled = is_enabled;
        window.set_background_appearance(self.settings.window_background_appearance());
        self.save_settings_and_show(
            format!("Backdrop blur {}", on_off_message_word(is_enabled)),
            window,
            cx,
        );
    }

    fn render_video_surface(
        &mut self,
        is_window_fullscreen: bool,
        viewport_width: f32,
        viewport_height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_video_surface_active = self.is_video_surface_active();
        let should_show_blurred_library_backdrop =
            self.settings.is_backdrop_blur_enabled && self.is_library_surface_visible();
        let should_show_player_overlay = !self.is_library_open;

        div()
            .id("player-surface")
            .relative()
            .overflow_hidden()
            .size_full()
            .bg(
                if is_video_surface_active || should_show_blurred_library_backdrop {
                    rgb_alpha(PLAYER_BLACK, 0.0)
                } else {
                    self.settings
                        .surface_background_color(PLAYER_BLACK, BACKDROP_BLUR_VIDEO_SURFACE_ALPHA)
                },
            )
            .border_1()
            .border_color(rgb(FINE_BORDER))
            .when(!is_video_surface_active, |surface| {
                surface.child(self.render_empty_video_plane(
                    is_window_fullscreen,
                    viewport_width,
                    viewport_height,
                    cx,
                ))
            })
            .child(self.render_embedded_video_host())
            .when(should_show_player_overlay, |surface| {
                surface.child(self.render_top_overlay(is_window_fullscreen, cx))
            })
            .when(should_show_player_overlay, |surface| {
                surface.child(self.render_bottom_controls(cx))
            })
            .when_some(
                self.osd_message
                    .clone()
                    .filter(|_| should_show_player_overlay),
                |surface, message| surface.child(self.render_osd_toast(message)),
            )
            .when(
                should_show_player_overlay
                    && (self.is_main_menu_open || self.is_subtitle_menu_open),
                |surface| {
                    surface.child(self.render_menu_clickoff_layer(
                        viewport_width,
                        viewport_height,
                        cx,
                    ))
                },
            )
            .when(
                should_show_player_overlay && self.is_main_menu_open,
                |surface| surface.child(self.render_main_dropdown(cx)),
            )
            .when(
                should_show_player_overlay && self.is_subtitle_menu_open,
                |surface| {
                    surface.child(self.render_subtitle_context_menu(
                        viewport_width,
                        viewport_height,
                        cx,
                    ))
                },
            )
            .when(
                should_show_player_overlay && self.pending_watch_session.is_some(),
                |surface| surface.child(self.render_continue_watching_prompt(cx)),
            )
            .when(self.is_settings_modal_open, |surface| {
                surface.child(self.render_settings_modal(cx))
            })
            .when(self.is_source_search_open, |surface| {
                surface.child(self.render_source_search_overlay(cx))
            })
    }

    fn render_osd_toast(&self, message: SharedString) -> impl IntoElement {
        div()
            .id("osd-toast")
            .absolute()
            .top(px(86.0))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(
                div()
                    .px_4()
                    .py_2()
                    .bg(rgb_alpha(OLED_BLACK, 0.72))
                    .border_1()
                    .border_color(rgb(BRIGHT_BORDER))
                    .text_color(rgb(SOFT_WHITE))
                    .text_sm()
                    .shadow_lg()
                    .child(message),
            )
    }

    fn render_menu_clickoff_layer(
        &self,
        viewport_width: f32,
        viewport_height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let menu_left = if self.is_subtitle_menu_open {
            self.subtitle_context_menu_origin(viewport_width, viewport_height)
                .0
        } else {
            (viewport_width - MAIN_MENU_CLICKOFF_SAFE_COLUMN_WIDTH).max(0.0)
        };
        let menu_width = if self.is_subtitle_menu_open {
            CONTEXT_MENU_WIDTH
        } else {
            MAIN_MENU_WIDTH
        };
        let menu_right = (menu_left + menu_width).min(viewport_width);

        div()
            .id("menu-clickoff-layer")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .child(
                div()
                    .id("menu-clickoff-left")
                    .absolute()
                    .top_0()
                    .left_0()
                    .bottom_0()
                    .w(px(menu_left.max(0.0)))
                    .bg(gpui::transparent_black())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|player, _event, window, cx| {
                            player.close_open_menus(window, cx);
                        }),
                    ),
            )
            .child(
                div()
                    .id("menu-clickoff-right")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(menu_right))
                    .right_0()
                    .bg(gpui::transparent_black())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|player, _event, window, cx| {
                            player.close_open_menus(window, cx);
                        }),
                    ),
            )
    }

    fn render_embedded_video_host(&self) -> impl IntoElement {
        let video_host_window_ids = self
            .video_host_window
            .as_ref()
            .and_then(EmbeddedVideoHost::window_ids);
        let is_video_visible = self.is_video_surface_active();

        canvas(
            move |bounds, window, _cx| {
                if let Some((video_host_window_id, parent_window_id)) = video_host_window_ids {
                    position_embedded_video_host(
                        video_host_window_id,
                        parent_window_id,
                        bounds,
                        window.scale_factor(),
                        is_video_visible,
                    );
                }
            },
            |_bounds, _state, _window, _cx| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
    }

    fn render_empty_video_plane(
        &mut self,
        is_window_fullscreen: bool,
        viewport_width: f32,
        viewport_height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.is_library_open || self.current_media().is_none() {
            return self
                .render_library_mode(is_window_fullscreen, viewport_width, viewport_height, cx)
                .into_any_element();
        }

        div()
            .id("top-player-overlay")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(self
                .settings
                .surface_background_color(OLED_BLACK, BACKDROP_BLUR_BASE_BACKGROUND_ALPHA))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .text_color(rgb(SOFT_WHITE))
                    .child(div().text_lg().child(self.current_media_title()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED_TEXT))
                            .child(self.current_media_detail()),
                    )
                    .when_some(self.status_message.clone(), |panel, message| {
                        panel.child(div().text_xs().text_color(rgb(VLC_ORANGE)).child(message))
                    }),
            )
            .into_any_element()
    }

    fn render_library_mode(
        &mut self,
        is_window_fullscreen: bool,
        viewport_width: f32,
        viewport_height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let library_scale = library_ui_scale(viewport_width, viewport_height);
        let content_width = library_content_width(viewport_width, library_scale);
        let shelves = self.library_shelves_cached();
        let has_library_shelves = !shelves.is_empty();

        div()
            .id("library-plane")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .p(px(32.0 * library_scale))
            .bg(self
                .settings
                .surface_background_color(OLED_BLACK, BACKDROP_BLUR_LIBRARY_BACKGROUND_ALPHA))
            .overflow_y_scroll()
            .scrollbar_width(px((6.0 * library_scale).clamp(6.0, 12.0)))
            .text_color(rgb(SOFT_WHITE))
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .w(px(content_width))
                    .flex()
                    .flex_col()
                    .gap(px(24.0 * library_scale))
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .justify_between()
                            .gap(px(16.0 * library_scale))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0 * library_scale))
                                    .child(
                                        div()
                                            .text_size(px(18.0 * library_scale))
                                            .line_height(px(24.0 * library_scale))
                                            .child("Library"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0 * library_scale))
                                            .line_height(px(16.0 * library_scale))
                                            .text_color(rgb(MUTED_TEXT))
                                            .child(self.status_message.clone().unwrap_or_else(
                                                || "Recent media, quick resume, and series.".into(),
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0 * library_scale))
                                    .child(self.render_live_capture_device_dropdown_button(
                                        library_scale,
                                        cx,
                                    ))
                                    .child(prompt_action_button(
                                        "library-unwatched-only",
                                        if self.show_unwatched_only {
                                            "All Episodes"
                                        } else {
                                            "Unwatched"
                                        },
                                        false,
                                        library_scale,
                                        cx,
                                        |player, _window, cx| {
                                            player.toggle_unwatched_only(cx);
                                        },
                                    ))
                                    .child(prompt_icon_action_button(
                                        "library-open-file",
                                        ICON_FILE,
                                        "Open File",
                                        library_scale,
                                        cx,
                                        |player, window, cx| {
                                            player.open_file_picker(window, cx);
                                        },
                                    ))
                                    .child(prompt_icon_action_button(
                                        "library-open-folder",
                                        ICON_FOLDER,
                                        "Open Folder",
                                        library_scale,
                                        cx,
                                        |player, window, cx| {
                                            player.open_folder_picker(window, cx);
                                        },
                                    ))
                                    .child(prompt_icon_action_button(
                                        "library-source-search",
                                        ICON_SEARCH,
                                        "Search source providers",
                                        library_scale,
                                        cx,
                                        |player, window, cx| {
                                            player.open_source_search_overlay(window, cx);
                                        },
                                    )),
                            ),
                    )
                    .children(shelves.into_iter().map(|shelf| {
                        self.render_library_shelf_section(shelf, viewport_width, library_scale, cx)
                    }))
                    .when(!has_library_shelves, |panel| {
                        panel.child(empty_menu_message("Open media to build your library."))
                    }),
            )
            .when(self.library_context_menu_media_path.is_some(), |plane| {
                plane.child(self.render_library_context_clickoff_layer(cx))
            })
            .when(self.library_context_menu_media_path.is_some(), |plane| {
                plane.child(self.render_library_context_menu(viewport_width, viewport_height, cx))
            })
            .when(self.is_live_capture_menu_open, |plane| {
                plane.child(self.render_live_capture_device_overlay(
                    viewport_width,
                    viewport_height,
                    content_width,
                    library_scale,
                    cx,
                ))
            })
            .child(self.render_library_fullscreen_button(is_window_fullscreen, library_scale, cx))
            .child(self.render_library_settings_button(library_scale, cx))
    }

    fn render_live_capture_device_dropdown_button(
        &self,
        library_scale: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("library-live-capture")
            .relative()
            .child(prompt_action_button(
                "library-live-capture-button",
                "Live Capture",
                false,
                library_scale,
                cx,
                |player, window, cx| {
                    player.toggle_live_capture_device_menu(window, cx);
                },
            ))
    }

    fn render_live_capture_device_overlay(
        &self,
        viewport_width: f32,
        viewport_height: f32,
        content_width: f32,
        library_scale: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_scan_pending = self.is_live_capture_scan_pending;
        let dropdown_width = LIVE_CAPTURE_DROPDOWN_WIDTH * library_scale;
        let content_left = ((viewport_width - content_width) / 2.0).max(0.0);
        let action_group_width = LIBRARY_ACTION_GROUP_ESTIMATED_WIDTH * library_scale;
        let menu_left = (content_left + (content_width - action_group_width).max(0.0))
            .clamp(8.0, (viewport_width - dropdown_width - 8.0).max(8.0));

        div()
            .id("library-live-capture-overlay")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .child(
                div()
                    .id("library-live-capture-clickoff")
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .bg(gpui::transparent_black())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|player, _event, _window, cx| {
                            player.is_live_capture_menu_open = false;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|player, _event, _window, cx| {
                            player.is_live_capture_menu_open = false;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(
                div()
                    .id("library-live-capture-menu")
                    .absolute()
                    .top(px(LIBRARY_LIVE_CAPTURE_OVERLAY_TOP * library_scale))
                    .left(px(menu_left))
                    .w(px(dropdown_width))
                    .max_h(px((viewport_height - (92.0 * library_scale)).max(220.0)))
                    .overflow_y_scroll()
                    .scrollbar_width(px((4.0 * library_scale).clamp(4.0, 10.0)))
                    .p(px(8.0 * library_scale))
                    .bg(rgb_alpha(0x0a0a0a, 0.95))
                    .border_1()
                    .border_color(rgb(BRIGHT_BORDER))
                    .rounded_sm()
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap(px(4.0 * library_scale))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_player, _event, _window, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|_player, _event, _window, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(self.render_live_capture_refresh_row(library_scale, cx))
                    .child(self.render_live_capture_audio_source_section(library_scale, cx))
                    .when(is_scan_pending, |menu| {
                        menu.child(scaled_menu_message(
                            "Scanning capture devices...",
                            library_scale,
                        ))
                    })
                    .when(
                        !is_scan_pending && self.live_capture_devices.is_empty(),
                        |menu| {
                            menu.child(scaled_menu_message(
                                "No capture devices found.",
                                library_scale,
                            ))
                        },
                    )
                    .children(self.live_capture_devices.iter().cloned().map(|device| {
                        self.render_live_capture_device_row(device, library_scale, cx)
                    })),
            )
    }

    fn render_live_capture_refresh_row(
        &self,
        library_scale: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_scan_pending = self.is_live_capture_scan_pending;

        div()
            .id("library-live-capture-refresh")
            .px(px(8.0 * library_scale))
            .py(px(6.0 * library_scale))
            .text_size(px(12.0 * library_scale))
            .line_height(px(16.0 * library_scale))
            .text_color(rgb(MUTED_TEXT))
            .rounded_sm()
            .cursor_pointer()
            .hover(|row| row.bg(rgb(0x121212)))
            .on_click(cx.listener(|player, _, window, cx| {
                player.refresh_live_capture_devices(window, cx);
                cx.stop_propagation();
            }))
            .child(if is_scan_pending {
                "Refreshing..."
            } else {
                "Refresh devices"
            })
    }

    fn render_live_capture_audio_source_section(
        &self,
        library_scale: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("library-live-capture-audio-source-section")
            .flex()
            .flex_col()
            .gap(px(3.0 * library_scale))
            .pt(px(6.0 * library_scale))
            .pb(px(6.0 * library_scale))
            .border_b_1()
            .border_color(rgb(FINE_BORDER))
            .child(
                div()
                    .px(px(8.0 * library_scale))
                    .text_size(px(11.0 * library_scale))
                    .line_height(px(14.0 * library_scale))
                    .text_color(rgb(MUTED_TEXT))
                    .child("Audio source"),
            )
            .child(self.render_live_capture_audio_source_row(
                LIVE_CAPTURE_AUDIO_SOURCE_AUTO.to_string(),
                "Auto",
                "Use best matching source".to_string(),
                library_scale,
                cx,
            ))
            .child(self.render_live_capture_audio_source_row(
                LIVE_CAPTURE_AUDIO_SOURCE_NONE.to_string(),
                "Video only",
                "No captured audio".to_string(),
                library_scale,
                cx,
            ))
            .children(
                self.live_capture_audio_devices
                    .iter()
                    .cloned()
                    .map(|audio_device| {
                        self.render_live_capture_audio_device_source_row(
                            audio_device,
                            library_scale,
                            cx,
                        )
                    }),
            )
    }

    fn render_live_capture_audio_device_source_row(
        &self,
        audio_device: LiveCaptureAudioDevice,
        library_scale: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.render_live_capture_audio_source_row(
            audio_device.backend_name,
            audio_device.display_name,
            "DirectShow audio source".to_string(),
            library_scale,
            cx,
        )
    }

    fn render_live_capture_audio_source_row(
        &self,
        audio_source: String,
        title: impl Into<SharedString>,
        detail: String,
        library_scale: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = self.settings.live_capture_audio_source == audio_source;
        let audio_source_id = library_safe_element_key(&audio_source);

        div()
            .id(format!(
                "library-live-capture-audio-source-{audio_source_id}"
            ))
            .flex()
            .flex_col()
            .gap(px(2.0 * library_scale))
            .px(px(8.0 * library_scale))
            .py(px(5.0 * library_scale))
            .bg(if is_selected {
                rgb_alpha(VLC_ORANGE, 0.10)
            } else {
                rgb_alpha(OLED_BLACK, 0.0)
            })
            .rounded_sm()
            .cursor_pointer()
            .hover(|row| row.bg(rgb(0x121212)))
            .on_click(cx.listener(move |player, _, window, cx| {
                player.set_live_capture_audio_source(audio_source.clone(), window, cx);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .text_size(px(12.0 * library_scale))
                    .line_height(px(16.0 * library_scale))
                    .truncate()
                    .child(title.into()),
            )
            .child(
                div()
                    .text_size(px(10.0 * library_scale))
                    .line_height(px(13.0 * library_scale))
                    .text_color(rgb(MUTED_TEXT))
                    .truncate()
                    .child(if is_selected {
                        "selected".to_string()
                    } else {
                        detail
                    }),
            )
    }

    fn render_live_capture_device_row(
        &self,
        device: LiveCaptureDevice,
        library_scale: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let launch_device = self.live_capture_device_with_selected_audio_source(device);
        let device_id = library_safe_element_key(&launch_device.backend_name);
        let device_for_click = launch_device.clone();
        let device_display_name = launch_device.display_name.clone();
        let device_detail = launch_device.detail_label();

        div()
            .id(format!("library-live-capture-device-{device_id}"))
            .flex()
            .flex_col()
            .gap(px(2.0 * library_scale))
            .px(px(8.0 * library_scale))
            .py(px(7.0 * library_scale))
            .rounded_sm()
            .cursor_pointer()
            .hover(|row| row.bg(rgb(0x121212)))
            .on_click(cx.listener(move |player, _, window, cx| {
                player.load_live_capture_device(device_for_click.clone(), window, cx);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .text_size(px(13.0 * library_scale))
                    .line_height(px(17.0 * library_scale))
                    .truncate()
                    .child(device_display_name),
            )
            .child(
                div()
                    .text_size(px(11.0 * library_scale))
                    .line_height(px(14.0 * library_scale))
                    .text_color(rgb(MUTED_TEXT))
                    .truncate()
                    .child(device_detail),
            )
    }

    fn render_library_fullscreen_button(
        &self,
        is_window_fullscreen: bool,
        library_scale: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let fullscreen_icon_key = "library-fullscreen";
        let is_fullscreen_icon_hovered =
            self.hovered_library_icon_key.as_deref() == Some(fullscreen_icon_key);
        let is_fullscreen_icon_exiting =
            self.exiting_library_icon_key.as_deref() == Some(fullscreen_icon_key);
        let library_icon_scale_animation_generation = self.library_icon_scale_animation_generation;
        let fullscreen_icon_path = if is_window_fullscreen {
            ICON_MINIMIZE
        } else {
            ICON_MAXIMIZE
        };
        let tooltip_text = if is_window_fullscreen {
            "Exit fullscreen"
        } else {
            "Enter fullscreen"
        };

        div()
            .id("library-fullscreen-button")
            .absolute()
            .right(px(76.0 * library_scale))
            .bottom(px(28.0 * library_scale))
            .w(px(48.0 * library_scale))
            .h(px(48.0 * library_scale))
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(SOFT_WHITE))
            .cursor_pointer()
            .active(|button| button.opacity(0.78))
            .on_hover(cx.listener(move |player, is_hovered: &bool, _window, cx| {
                player.update_library_icon_hover(fullscreen_icon_key.to_string(), *is_hovered, cx);
            }))
            .on_click(cx.listener(|player, _, window, cx| {
                player.toggle_window_fullscreen(window, cx);
                cx.stop_propagation();
            }))
            .child(render_scaled_library_icon(
                fullscreen_icon_path,
                fullscreen_icon_key,
                21.0 * library_scale,
                21.0 * library_scale,
                is_fullscreen_icon_hovered,
                is_fullscreen_icon_exiting,
                library_icon_scale_animation_generation,
            ))
            .tooltip(move |_window, cx| {
                cx.new(|_| TooltipText {
                    text: tooltip_text.into(),
                })
                .into()
            })
    }

    fn render_library_settings_button(
        &self,
        library_scale: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let settings_icon_key = "library-settings";
        let is_settings_icon_hovered =
            self.hovered_library_icon_key.as_deref() == Some(settings_icon_key);
        let is_settings_icon_exiting =
            self.exiting_library_icon_key.as_deref() == Some(settings_icon_key);
        let library_icon_scale_animation_generation = self.library_icon_scale_animation_generation;

        div()
            .id("library-settings-button")
            .absolute()
            .right(px(28.0 * library_scale))
            .bottom(px(28.0 * library_scale))
            .w(px(48.0 * library_scale))
            .h(px(48.0 * library_scale))
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(SOFT_WHITE))
            .cursor_pointer()
            .active(|button| button.opacity(0.78))
            .on_hover(cx.listener(move |player, is_hovered: &bool, _window, cx| {
                player.update_library_icon_hover(settings_icon_key.to_string(), *is_hovered, cx);
            }))
            .on_click(cx.listener(|player, _, window, cx| {
                player.open_settings_modal(window, cx);
                cx.stop_propagation();
            }))
            .child(render_scaled_library_icon(
                ICON_SETTINGS,
                settings_icon_key,
                21.0 * library_scale,
                21.0 * library_scale,
                is_settings_icon_hovered,
                is_settings_icon_exiting,
                library_icon_scale_animation_generation,
            ))
            .tooltip(move |_window, cx| {
                cx.new(|_| TooltipText {
                    text: "Settings".into(),
                })
                .into()
            })
    }

    fn library_shelves_cached(&mut self) -> Vec<LibraryShelf> {
        if self.library_view_model.generation != self.library_generation {
            self.library_view_model.shelves =
                build_library_shelves(&self.library, self.show_unwatched_only);
            self.library_view_model.generation = self.library_generation;
        }

        self.library_view_model.shelves.clone()
    }

    fn render_library_shelf_section(
        &self,
        shelf: LibraryShelf,
        viewport_width: f32,
        library_scale: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let visible_card_count = library_visible_card_count(viewport_width, library_scale);
        let card_width = library_card_width(viewport_width, library_scale);
        let max_offset = shelf.items.len().saturating_sub(visible_card_count);
        let shelf_offset = self
            .library_shelf_offsets
            .get(&shelf.key)
            .copied()
            .unwrap_or(0)
            .min(max_offset);
        let can_page_left = shelf_offset > 0;
        let can_page_right = shelf_offset + visible_card_count < shelf.items.len();
        let visible_items = shelf
            .items
            .iter()
            .skip(shelf_offset)
            .take(visible_card_count)
            .cloned()
            .collect::<Vec<_>>();
        let shelf_key = shelf.key.clone();
        let shelf_title = shelf.title.clone();
        let shelf_subtitle = shelf.subtitle.clone();
        let shelf_empty_message = shelf.empty_message;
        let item_count = shelf.items.len();
        let is_collapsed = self.collapsed_library_shelf_keys.contains(&shelf.key);
        let collapse_icon_path = if is_collapsed {
            ICON_CHEVRON_RIGHT
        } else {
            ICON_CHEVRON_DOWN
        };
        let collapse_shelf_key = shelf_key.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(12.0 * library_scale))
            .child(
                div()
                    .id(format!("library-shelf-header-{shelf_key}"))
                    .h(px(34.0 * library_scale))
                    .flex()
                    .items_center()
                    .gap(px(8.0 * library_scale))
                    .cursor_pointer()
                    .on_click(cx.listener(move |player, _, _window, cx| {
                        player.toggle_library_shelf_collapse(collapse_shelf_key.clone(), cx);
                        cx.stop_propagation();
                    }))
                    .child(
                        svg()
                            .external_path(crate::icon_path(collapse_icon_path))
                            .w(px(16.0 * library_scale))
                            .h(px(16.0 * library_scale))
                            .text_color(rgb(MUTED_TEXT)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0 * library_scale))
                            .child(
                                div()
                                    .text_size(px(14.0 * library_scale))
                                    .line_height(px(16.0 * library_scale))
                                    .text_color(rgb(MUTED_TEXT))
                                    .child(shelf_title.clone()),
                            )
                            .when_some(shelf_subtitle, |label, subtitle| {
                                label.child(
                                    div()
                                        .text_size(px(12.0 * library_scale))
                                        .line_height(px(14.0 * library_scale))
                                        .text_color(rgb(SOFT_WHITE))
                                        .child(subtitle),
                                )
                            }),
                    ),
            )
            .when(item_count == 0, |section| {
                section.child(empty_menu_message(shelf_empty_message))
            })
            .when(item_count > 0 && !is_collapsed, |section| {
                section.child(
                    div()
                        .relative()
                        .overflow_hidden()
                        .child(
                            div()
                                .flex()
                                .gap(px(LIBRARY_CARD_GAP_PX * library_scale))
                                .children(visible_items.into_iter().map(|item| {
                                    self.render_library_grid_card(
                                        shelf_title.clone(),
                                        card_width,
                                        library_scale,
                                        item,
                                        cx,
                                    )
                                })),
                        )
                        .when(can_page_left, |row| {
                            row.child(self.render_library_shelf_arrow(
                                shelf_key.clone(),
                                item_count,
                                -1,
                                viewport_width,
                                library_scale,
                                cx,
                            ))
                        })
                        .when(can_page_right, |row| {
                            row.child(self.render_library_shelf_arrow(
                                shelf_key.clone(),
                                item_count,
                                1,
                                viewport_width,
                                library_scale,
                                cx,
                            ))
                        }),
                )
            })
    }

    fn render_library_shelf_arrow(
        &self,
        shelf_key: String,
        item_count: usize,
        direction: i32,
        viewport_width: f32,
        library_scale: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_left_arrow = direction < 0;
        let icon_path = if is_left_arrow {
            ICON_CHEVRON_LEFT
        } else {
            ICON_CHEVRON_RIGHT
        };
        let arrow_direction_label = if is_left_arrow { "left" } else { "right" };
        let arrow_icon_key = format!("library-shelf-arrow-{shelf_key}-{arrow_direction_label}");
        let arrow_element_id = arrow_icon_key.clone();
        let arrow_hover_key = arrow_icon_key.clone();
        let click_shelf_key = shelf_key.clone();
        let is_arrow_icon_hovered = self.hovered_library_icon_key.as_ref() == Some(&arrow_icon_key);
        let is_arrow_icon_exiting = self.exiting_library_icon_key.as_ref() == Some(&arrow_icon_key);
        let library_icon_scale_animation_generation = self.library_icon_scale_animation_generation;
        let mut arrow = div()
            .id(arrow_element_id)
            .absolute()
            .top(px(34.0 * library_scale))
            .w(px(40.0 * library_scale))
            .h(px(72.0 * library_scale))
            .flex()
            .items_center()
            .justify_center()
            .bg(if self.settings.is_backdrop_blur_enabled {
                rgb_alpha(OLED_BLACK, BACKDROP_BLUR_FLOATING_CONTROL_ALPHA)
            } else {
                rgb_alpha(OLED_BLACK, 0.76)
            })
            .text_color(rgb(SOFT_WHITE))
            .cursor_pointer()
            .hover(|button| {
                button.bg(if self.settings.is_backdrop_blur_enabled {
                    rgb_alpha(OLED_BLACK, BACKDROP_BLUR_MENU_BACKGROUND_ALPHA)
                } else {
                    rgb_alpha(OLED_BLACK, 0.92)
                })
            })
            .on_hover(cx.listener(move |player, is_hovered: &bool, _window, cx| {
                player.update_library_icon_hover(arrow_hover_key.clone(), *is_hovered, cx);
            }))
            .on_click(cx.listener(move |player, _, _window, cx| {
                player.shift_library_shelf(
                    click_shelf_key.clone(),
                    item_count,
                    direction,
                    viewport_width,
                    library_scale,
                );
                cx.stop_propagation();
                cx.notify();
            }))
            .child(render_scaled_library_icon(
                icon_path,
                &arrow_icon_key,
                22.0 * library_scale,
                22.0 * library_scale,
                is_arrow_icon_hovered,
                is_arrow_icon_exiting,
                library_icon_scale_animation_generation,
            ));

        arrow = if is_left_arrow {
            arrow.left_0()
        } else {
            arrow.right_0()
        };
        arrow
    }

    fn shift_library_shelf(
        &mut self,
        shelf_key: String,
        item_count: usize,
        direction: i32,
        viewport_width: f32,
        library_scale: f32,
    ) {
        let visible_card_count = library_visible_card_count(viewport_width, library_scale);
        let max_offset = item_count.saturating_sub(visible_card_count);
        let current_offset = self
            .library_shelf_offsets
            .get(&shelf_key)
            .copied()
            .unwrap_or(0)
            .min(max_offset);
        let page_step = visible_card_count.saturating_sub(1).max(1);
        let next_offset = if direction < 0 {
            current_offset.saturating_sub(page_step)
        } else {
            (current_offset + page_step).min(max_offset)
        };

        self.library_shelf_offsets.insert(shelf_key, next_offset);
    }

    fn toggle_library_shelf_collapse(&mut self, shelf_key: String, cx: &mut Context<Self>) {
        if !self.collapsed_library_shelf_keys.insert(shelf_key.clone()) {
            self.collapsed_library_shelf_keys.remove(&shelf_key);
        }

        cx.notify();
    }

    fn toggle_unwatched_only(&mut self, cx: &mut Context<Self>) {
        self.show_unwatched_only = !self.show_unwatched_only;
        self.mark_library_dirty();
        cx.notify();
    }

    #[allow(dead_code)]
    fn update_continue_remove_hover(
        &mut self,
        media_path: PathBuf,
        is_hovered: bool,
        cx: &mut Context<Self>,
    ) {
        if is_hovered {
            if self.hovered_continue_remove_media_path.as_ref() == Some(&media_path) {
                return;
            }

            self.hovered_continue_remove_media_path = Some(media_path);
            self.exiting_continue_remove_media_path = None;
        } else if self.hovered_continue_remove_media_path.as_ref() == Some(&media_path) {
            self.hovered_continue_remove_media_path = None;
            self.exiting_continue_remove_media_path = Some(media_path);
        } else {
            return;
        }

        self.continue_remove_scale_animation_generation = self
            .continue_remove_scale_animation_generation
            .wrapping_add(1);
        cx.notify();
    }

    fn update_library_icon_hover(
        &mut self,
        icon_key: String,
        is_hovered: bool,
        cx: &mut Context<Self>,
    ) {
        if is_hovered {
            if self.hovered_library_icon_key.as_ref() == Some(&icon_key) {
                return;
            }

            self.hovered_library_icon_key = Some(icon_key);
            self.exiting_library_icon_key = None;
        } else if self.hovered_library_icon_key.as_ref() == Some(&icon_key) {
            self.hovered_library_icon_key = None;
            self.exiting_library_icon_key = Some(icon_key);
        } else {
            return;
        }

        self.library_icon_scale_animation_generation =
            self.library_icon_scale_animation_generation.wrapping_add(1);
        cx.notify();
    }

    fn dismiss_continue_watching_entry(&mut self, media_path: PathBuf, cx: &mut Context<Self>) {
        self.library
            .media_history
            .retain(|entry| entry.path != media_path);
        if self.hovered_continue_remove_media_path.as_ref() == Some(&media_path) {
            self.hovered_continue_remove_media_path = None;
        }
        if self.exiting_continue_remove_media_path.as_ref() == Some(&media_path) {
            self.exiting_continue_remove_media_path = None;
        }
        self.mark_library_dirty();
        save_player_library_atomic(&self.library);
        self.status_message = Some("Removed from Continue Watching.".into());
        cx.notify();
    }

    fn internet_media_for_library_path(&self, media_path: &Path) -> Option<InternetMedia> {
        self.library
            .recent_internet_media
            .iter()
            .find(|media| internet_media_library_path(media) == media_path)
            .cloned()
            .or_else(|| {
                self.library
                    .internet_media_history
                    .iter()
                    .map(|entry| &entry.media)
                    .find(|media| internet_media_library_path(media) == media_path)
                    .cloned()
            })
    }

    fn remove_internet_media_from_library(
        &mut self,
        internet_media: InternetMedia,
        cx: &mut Context<Self>,
    ) {
        let removed_media_key = internet_media_key(&internet_media);
        let removed_media_path = internet_media_library_path(&internet_media);
        let is_current_media_removed = self
            .current_media()
            .and_then(LoadedMedia::internet_media)
            .is_some_and(|current_media| internet_media_key(current_media) == removed_media_key);

        self.library.recent_internet_media.retain(|media| {
            internet_media_key(media) != removed_media_key
                && internet_media_library_path(media) != removed_media_path
        });
        self.library.internet_media_history.retain(|entry| {
            internet_media_key(&entry.media) != removed_media_key
                && internet_media_library_path(&entry.media) != removed_media_path
        });

        if is_current_media_removed {
            self.pending_watch_session = None;
            clear_saved_watch_session();
            self.playback_progress_generation = self.playback_progress_generation.wrapping_add(1);
            self.stop_playback_process();
            self.playback_queue.clear();
            self.current_queue_index = None;
            self.selected_audio_track_id = None;
            self.selected_subtitle_path = None;
            self.selected_embedded_subtitle_track_id = None;
            self.playback_position_seconds = 0.0;
            self.is_playing = false;
            self.is_eof_reached = false;
        }

        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.library_context_menu_internet_media = None;
        self.status_message = Some("Removed from Source.".into());
        self.mark_library_dirty();
        save_player_library_atomic(&self.library);
        cx.notify();
    }

    fn mark_library_media_watched(&mut self, media_path: PathBuf, cx: &mut Context<Self>) {
        let media_path_key = library_media_path_key(&media_path);
        let duration_seconds = self
            .current_media()
            .filter(|media| {
                media
                    .file_path()
                    .is_some_and(|path| library_media_path_key(path) == media_path_key)
            })
            .and_then(|media| media.duration_seconds)
            .or_else(|| {
                self.library
                    .media_history
                    .iter()
                    .find(|entry| library_media_path_key(&entry.path) == media_path_key)
                    .and_then(|entry| entry.duration_seconds)
            });
        let playback_position_seconds = duration_seconds.unwrap_or(0.0).max(0.0);

        self.library
            .media_history
            .retain(|entry| library_media_path_key(&entry.path) != media_path_key);
        self.library.media_history.insert(
            0,
            MediaHistoryEntry {
                path: media_path.clone(),
                playback_position_seconds,
                duration_seconds,
                is_completed: true,
                updated_at_millis: current_time_millis(),
                selected_audio_track_id: None,
                selected_embedded_subtitle_track_id: None,
                selected_subtitle_path: None,
            },
        );
        self.library.media_history.truncate(MAX_RECENT_MEDIA * 2);
        promote_recent_path(
            &mut self.library.recent_media_paths,
            media_path,
            MAX_RECENT_MEDIA,
        );
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.library_context_menu_internet_media = None;
        self.status_message = Some("Marked watched.".into());
        self.mark_library_dirty();
        save_player_library_atomic(&self.library);
        cx.notify();
    }

    fn mark_library_media_unwatched(&mut self, media_path: PathBuf, cx: &mut Context<Self>) {
        let media_path_key = library_media_path_key(&media_path);

        if let Some(entry) = self
            .library
            .media_history
            .iter_mut()
            .find(|entry| library_media_path_key(&entry.path) == media_path_key)
        {
            entry.is_completed = false;
            entry.playback_position_seconds = 0.0;
            entry.updated_at_millis = current_time_millis();
        } else {
            self.library.media_history.insert(
                0,
                MediaHistoryEntry {
                    path: media_path.clone(),
                    playback_position_seconds: 0.0,
                    duration_seconds: None,
                    is_completed: false,
                    updated_at_millis: current_time_millis(),
                    selected_audio_track_id: None,
                    selected_embedded_subtitle_track_id: None,
                    selected_subtitle_path: None,
                },
            );
        }

        promote_recent_path(
            &mut self.library.recent_media_paths,
            media_path,
            MAX_RECENT_MEDIA,
        );
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.library_context_menu_internet_media = None;
        self.status_message = Some("Marked unwatched.".into());
        self.mark_library_dirty();
        save_player_library_atomic(&self.library);
        cx.notify();
    }

    fn pin_library_folder(&mut self, folder_path: PathBuf, cx: &mut Context<Self>) {
        if !folder_path.is_dir() {
            return;
        }

        promote_recent_path(
            &mut self.library.pinned_folder_paths,
            folder_path,
            MAX_RECENT_FOLDERS,
        );
        self.status_message = Some("Folder pinned.".into());
        self.mark_library_dirty();
        save_player_library_atomic(&self.library);
        cx.notify();
    }

    fn render_library_context_clickoff_layer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("library-context-clickoff")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .bg(gpui::transparent_black())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|player, _event, _window, cx| {
                    player.close_library_context_menu(cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|player, _event, _window, cx| {
                    player.close_library_context_menu(cx);
                    cx.stop_propagation();
                }),
            )
    }

    fn render_library_context_menu(
        &self,
        viewport_width: f32,
        viewport_height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let media_path = self
            .library_context_menu_media_path
            .clone()
            .unwrap_or_default();
        let internet_media = self
            .library_context_menu_internet_media
            .clone()
            .or_else(|| self.internet_media_for_library_path(&media_path));
        let (menu_left, menu_top) =
            self.library_context_menu_origin(viewport_width, viewport_height);

        let menu = div()
            .id("library-context-menu")
            .absolute()
            .left(px(menu_left))
            .top(px(menu_top))
            .w(px(LIBRARY_CONTEXT_MENU_WIDTH))
            .p_2()
            .bg(rgb_alpha(0x0a0a0a, 0.95))
            .rounded_sm()
            .border_1()
            .border_color(rgb(BRIGHT_BORDER))
            .rounded_sm()
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_1()
            .text_color(rgb(SOFT_WHITE));

        if let Some(internet_media) = internet_media {
            menu.child(simple_menu_action(
                "Remove",
                cx,
                move |player, _window, cx| {
                    player.remove_internet_media_from_library(internet_media.clone(), cx);
                },
            ))
        } else {
            let is_in_continue_watching = self.library.media_history.iter().any(|entry| {
                entry.path == media_path
                    && !entry.is_completed
                    && entry.playback_position_seconds >= MINIMUM_RESUME_POSITION_SECONDS
            });

            menu.child(simple_menu_action("Mark Watched", cx, {
                let media_path = media_path.clone();
                move |player, _window, cx| {
                    player.mark_library_media_watched(media_path.clone(), cx);
                }
            }))
            .child(simple_menu_action("Mark Unwatched", cx, {
                let media_path = media_path.clone();
                move |player, _window, cx| {
                    player.mark_library_media_unwatched(media_path.clone(), cx);
                }
            }))
            .child(simple_menu_action("Pin Folder", cx, {
                let media_path = media_path.clone();
                move |player, _window, cx| {
                    let folder_path = if media_path.is_dir() {
                        media_path.clone()
                    } else {
                        media_path
                            .parent()
                            .map(Path::to_path_buf)
                            .unwrap_or_default()
                    };
                    player.pin_library_folder(folder_path, cx);
                }
            }))
            .when(is_in_continue_watching, |menu| {
                let media_path = media_path.clone();
                menu.child(simple_menu_action("Remove", cx, move |player, _window, cx| {
                    player.dismiss_continue_watching_entry(media_path.clone(), cx);
                }))
            })
        }
    }

    fn library_context_menu_origin(&self, viewport_width: f32, viewport_height: f32) -> (f32, f32) {
        let fallback_left = viewport_width - LIBRARY_CONTEXT_MENU_WIDTH - MENU_RIGHT_MARGIN;
        let fallback_top =
            viewport_height - LIBRARY_CONTEXT_MENU_ESTIMATED_HEIGHT - MENU_RIGHT_MARGIN;
        let (requested_left, requested_top) = self
            .library_context_menu_anchor
            .map(|anchor| {
                (
                    anchor.x.as_f32() + CONTEXT_MENU_OFFSET,
                    anchor.y.as_f32() + CONTEXT_MENU_OFFSET,
                )
            })
            .unwrap_or((fallback_left, fallback_top));
        let max_left = (viewport_width - LIBRARY_CONTEXT_MENU_WIDTH - MENU_RIGHT_MARGIN)
            .max(MENU_RIGHT_MARGIN);
        let max_top = (viewport_height - LIBRARY_CONTEXT_MENU_ESTIMATED_HEIGHT - MENU_RIGHT_MARGIN)
            .max(MENU_RIGHT_MARGIN);

        (
            clamp_menu_axis(requested_left, MENU_RIGHT_MARGIN, max_left),
            clamp_menu_axis(requested_top, MENU_RIGHT_MARGIN, max_top),
        )
    }

    fn render_library_grid_card(
        &self,
        section_title: String,
        card_width: f32,
        library_scale: f32,
        item: LibraryGridItem,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let item_path = item.path.clone();
        let item_path_for_click = item.path.clone();
        let item_path_for_context_menu = item.path.clone();
        let resume_history_entry = item.resume_history_entry.clone();
        let internet_media_for_click = item.internet_media.clone();
        let internet_media_for_context_menu = item.internet_media.clone();
        let is_watched = item.is_watched;
        let is_internet_media = item.is_internet_media;
        let episode_badge = item.episode_badge.clone();
        let has_episode_badge = episode_badge.is_some();
        let item_title = item.title.clone();
        let has_item_title = !item_title.is_empty();
        let title_overlay_width = if has_episode_badge {
            (card_width - (66.0 * library_scale)).max(1.0)
        } else {
            (card_width - (16.0 * library_scale)).max(1.0)
        };
        let thumbnail_path = item
            .thumbnail_media_path
            .as_ref()
            .and_then(|media_path| existing_library_thumbnail_path(media_path));
        let remote_thumbnail_path = item
            .thumbnail_url
            .as_deref()
            .and_then(existing_remote_thumbnail_path);
        let thumbnail_url = if remote_thumbnail_path.is_none() {
            item.thumbnail_url.clone()
        } else {
            None
        };
        let has_thumbnail =
            thumbnail_path.is_some() || remote_thumbnail_path.is_some() || thumbnail_url.is_some();
        let empty_thumbnail_label = if is_internet_media {
            item.title
                .chars()
                .find(|character| character.is_ascii_alphanumeric())
                .map(|character| character.to_ascii_uppercase().to_string())
                .unwrap_or_else(|| "S".to_string())
        } else {
            "Folder".to_string()
        };
        let placeholder_label = item
            .title
            .chars()
            .find(|character| character.is_ascii_alphanumeric())
            .map(|character| character.to_ascii_uppercase().to_string())
            .unwrap_or_else(|| "W".to_string());

        let card_id = stable_path_ui_id(
            &format!("library-card-{}", library_safe_element_key(&section_title)),
            &item_path,
        );

        div()
            .id(card_id)
            .flex()
            .flex_col()
            .gap(px(8.0 * library_scale))
            .w(px(card_width))
            .flex_none()
            .min_w_0()
            .cursor_pointer()
            .on_click(cx.listener(move |player, _, window, cx| {
                if let Some(resume_history_entry) = resume_history_entry.clone() {
                    player.continue_library_entry(resume_history_entry, window, cx);
                } else if let Some(internet_media) = internet_media_for_click.clone() {
                    player.load_internet_media(internet_media, window, cx);
                } else {
                    player.load_library_path(item_path_for_click.clone(), window, cx);
                }
            }))
            .child(
                div()
                    .relative()
                    .w_full()
                    .aspect_ratio(16.0 / 9.0)
                    .overflow_hidden()
                    .rounded_sm()
                    .bg(rgb(0x101010))
                    .border_1()
                    .border_color(rgb(FINE_BORDER))
                    .hover(|thumbnail| thumbnail.border_color(rgb(BRIGHT_BORDER)))
                    .when(is_internet_media, |frame| {
                        frame.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(rgb(MUTED_TEXT))
                                .text_size(px(12.0 * library_scale))
                                .line_height(px(16.0 * library_scale))
                                .child(empty_thumbnail_label.clone()),
                        )
                    })
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |player, event: &MouseDownEvent, _window, cx| {
                            if let Some(internet_media) = internet_media_for_context_menu.clone() {
                                player.show_internet_library_context_menu(
                                    internet_media,
                                    event.position,
                                    cx,
                                );
                            } else {
                                player.show_library_context_menu(
                                    item_path_for_context_menu.clone(),
                                    event.position,
                                    cx,
                                );
                            }
                            cx.stop_propagation();
                        }),
                    )
                    .when_some(thumbnail_path, |frame, thumbnail_path| {
                        frame.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .child(
                                    img(thumbnail_path)
                                        .w_full()
                                        .h_full()
                                        .object_fit(ObjectFit::Cover),
                                ),
                        )
                    })
                    .when_some(remote_thumbnail_path, |frame, remote_thumbnail_path| {
                        frame.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .child(
                                    img(remote_thumbnail_path)
                                        .w_full()
                                        .h_full()
                                        .object_fit(ObjectFit::Cover),
                                ),
                        )
                    })
                    .when_some(thumbnail_url, |frame, thumbnail_url| {
                        frame.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .child(
                                    img(thumbnail_url)
                                        .w_full()
                                        .h_full()
                                        .object_fit(ObjectFit::Cover),
                                ),
                        )
                    })
                    .when(
                        !is_internet_media && !has_thumbnail && item.thumbnail_media_path.is_none(),
                        |frame| {
                            frame
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(rgb(MUTED_TEXT))
                                .text_size(px(12.0 * library_scale))
                                .line_height(px(16.0 * library_scale))
                                .child(empty_thumbnail_label)
                        },
                    )
                    .when(
                        item.thumbnail_media_path.is_some() && !has_thumbnail,
                        |frame| {
                            frame.child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .right_0()
                                    .bottom_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(18.0 * library_scale))
                                    .line_height(px(24.0 * library_scale))
                                    .text_color(rgb(MUTED_TEXT))
                                    .child(placeholder_label),
                            )
                        },
                    )
                    .when(has_item_title, |frame| {
                        frame.child(
                            div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .px(px(8.0 * library_scale))
                                .py(px(8.0 * library_scale))
                                .bg(linear_gradient(
                                    180.0,
                                    linear_color_stop(gpui::transparent_black(), 0.0),
                                    linear_color_stop(gpui::black().opacity(0.86), 1.0),
                                ))
                                .child(div().w(px(title_overlay_width)).child(
                                    render_autoscrolling_library_title(
                                        item_title.clone(),
                                        title_overlay_width,
                                        &section_title,
                                        &item_path,
                                        library_scale,
                                    ),
                                )),
                        )
                    })
                    .when(is_internet_media, |frame| {
                        frame.child(
                            div()
                                .id(format!("library-internet-badge-{}", item_path.display()))
                                .absolute()
                                .top(px(8.0 * library_scale))
                                .right(px(8.0 * library_scale))
                                .w(px(28.0 * library_scale))
                                .h(px(28.0 * library_scale))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .bg(linear_gradient(
                                    135.0,
                                    linear_color_stop(rgb_alpha(OLED_BLACK, 0.62), 0.0),
                                    linear_color_stop(rgb_alpha(OLED_BLACK, 0.18), 1.0),
                                ))
                                .shadow_lg()
                                .child(
                                    svg()
                                        .external_path(crate::icon_path(ICON_GLOBE))
                                        .w(px(14.0 * library_scale))
                                        .h(px(14.0 * library_scale))
                                        .text_color(rgb(SOFT_WHITE)),
                                ),
                        )
                    })
                    .when_some(episode_badge, |frame, episode_badge| {
                        frame.child(
                            div()
                                .absolute()
                                .right(px(8.0 * library_scale))
                                .bottom(px(8.0 * library_scale))
                                .px(px(6.0 * library_scale))
                                .py(px(3.0 * library_scale))
                                .text_size(px(10.0 * library_scale))
                                .line_height(px(12.0 * library_scale))
                                .text_color(rgb(SOFT_WHITE))
                                .rounded_sm()
                                .bg(linear_gradient(
                                    135.0,
                                    linear_color_stop(rgb_alpha(OLED_BLACK, 0.62), 0.0),
                                    linear_color_stop(rgb_alpha(OLED_BLACK, 0.18), 1.0),
                                ))
                                .shadow_lg()
                                .child(episode_badge),
                        )
                    })
                    .when(is_watched, |frame| {
                        frame.child(
                            div()
                                .id(format!("library-watched-badge-{}", item_path.display()))
                                .absolute()
                                .top(px(8.0 * library_scale))
                                .left(px(8.0 * library_scale))
                                .w(px(28.0 * library_scale))
                                .h(px(28.0 * library_scale))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .bg(linear_gradient(
                                    135.0,
                                    linear_color_stop(rgb_alpha(OLED_BLACK, 0.62), 0.0),
                                    linear_color_stop(rgb_alpha(OLED_BLACK, 0.18), 1.0),
                                ))
                                .shadow_lg()
                                .child(
                                    svg()
                                        .external_path(crate::icon_path(ICON_EYE))
                                        .w(px(14.0 * library_scale))
                                        .h(px(14.0 * library_scale))
                                        .text_color(rgb(SOFT_WHITE)),
                                ),
                        )
                    }),
            )
            .when_some(item.subtitle, |card, subtitle| {
                card.child(
                    div()
                        .min_w_0()
                        .text_size(px(12.0 * library_scale))
                        .line_height(px(16.0 * library_scale))
                        .text_color(rgb(MUTED_TEXT))
                        .truncate()
                        .child(subtitle),
                )
            })
    }

    fn render_top_overlay(
        &self,
        is_window_fullscreen: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_live_capture = self.is_current_media_live_capture();
        let fullscreen_icon_path = if is_window_fullscreen {
            ICON_MINIMIZE
        } else {
            ICON_MAXIMIZE
        };
        let fullscreen_tooltip = if is_window_fullscreen {
            "Exit full screen"
        } else {
            "Full screen"
        };

        div()
            .id("top-player-overlay")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .p_4()
            .flex()
            .items_center()
            .justify_between()
            .bg(linear_gradient(
                180.0,
                linear_color_stop(gpui::black().opacity(0.72), 0.0),
                linear_color_stop(gpui::transparent_black(), 1.0),
            ))
            .opacity(if self.are_controls_visible { 1.0 } else { 0.0 })
            .on_hover(cx.listener(|player, is_hovered: &bool, window, cx| {
                player.set_player_overlay_hover(*is_hovered, window, cx);
            }))
            .when(is_live_capture, |overlay| overlay.child(div().flex_1()))
            .when(!is_live_capture, |overlay| {
                overlay.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .text_color(rgb(SOFT_WHITE))
                        .child(div().text_lg().child(self.current_media_title()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MUTED_TEXT))
                                .child(self.current_media_detail()),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(icon_button(
                        "source-search",
                        ICON_SEARCH,
                        "Search source providers",
                        cx,
                        |player, window, cx| {
                            player.open_source_search_overlay(window, cx);
                        },
                    ))
                    .child(icon_button(
                        "fullscreen",
                        fullscreen_icon_path,
                        fullscreen_tooltip,
                        cx,
                        |player, window, cx| {
                            player.toggle_window_fullscreen(window, cx);
                        },
                    ))
                    .child(square_button(
                        "menu-button",
                        "☰",
                        "Open menu",
                        cx,
                        |player, window, cx| {
                            player.toggle_main_menu(window, cx);
                        },
                    )),
            )
    }

    fn render_bottom_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_live_capture = self.is_current_media_live_capture();

        div()
            .id("bottom-player-overlay")
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .bg(linear_gradient(
                0.0,
                linear_color_stop(gpui::black().opacity(0.78), 0.0),
                linear_color_stop(gpui::transparent_black(), 1.0),
            ))
            .opacity(if self.are_controls_visible { 1.0 } else { 0.0 })
            .on_hover(cx.listener(|player, is_hovered: &bool, window, cx| {
                player.set_player_overlay_hover(*is_hovered, window, cx);
            }))
            .when(!is_live_capture, |controls| {
                controls.child(self.render_progress_bar(cx))
            })
            .child(if is_live_capture {
                self.render_live_capture_controls(cx)
            } else {
                self.render_transport_controls(cx)
            })
    }

    fn render_progress_bar(&self, cx: &mut Context<Self>) -> Div {
        let duration_seconds = self.current_media_duration_seconds();
        let playback_position_seconds = self.playback_position_seconds;
        let progress_fraction = duration_seconds
            .map(|duration_seconds| playback_position_seconds / duration_seconds)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let duration_label = duration_seconds
            .map(format_timestamp)
            .unwrap_or_else(|| "--:--".to_string());

        div()
            .relative()
            .flex()
            .items_center()
            .gap_4()
            .text_color(rgb(SOFT_WHITE))
            .text_sm()
            .child(
                div()
                    .w(px(64.0))
                    .child(format_timestamp(playback_position_seconds)),
            )
            .child(
                div()
                    .id("timeline")
                    .relative()
                    .flex()
                    .flex_1()
                    .items_center()
                    .h(px(34.0))
                    .cursor_pointer()
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(px(14.0))
                            .h(px(5.0))
                            .bg(rgb(0x282828)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top(px(14.0))
                            .h(px(5.0))
                            .w(relative(progress_fraction as f32))
                            .bg(rgb(SOFT_WHITE)),
                    )
                    .child(self.render_timeline_interaction_layer(cx))
                    .when_some(self.timeline_hover_preview.clone(), |timeline, preview| {
                        timeline.child(self.render_timeline_hover_preview(preview))
                    }),
            )
            .child(div().w(px(64.0)).child(duration_label))
    }

    fn render_timeline_interaction_layer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("timeline-hitbox")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|player, event: &MouseDownEvent, window, cx| {
                    let timeline_fraction =
                        timeline_fraction_from_window_position(window, event.position);
                    player.is_timeline_scrubbing = true;
                    player.scrub_to_timeline_fraction(timeline_fraction, window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|player, event: &MouseMoveEvent, window, cx| {
                let timeline_fraction =
                    timeline_fraction_from_window_position(window, event.position);
                let timeline_width_px = timeline_width_from_window(window);

                if event.dragging() || player.is_timeline_scrubbing {
                    player.scrub_to_timeline_fraction(timeline_fraction, window, cx);
                    cx.stop_propagation();
                } else {
                    player.update_timeline_hover_preview(
                        timeline_fraction,
                        timeline_width_px,
                        window,
                        cx,
                    );
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|player, event: &MouseUpEvent, window, cx| {
                    if player.is_timeline_scrubbing {
                        let timeline_fraction =
                            timeline_fraction_from_window_position(window, event.position);
                        player.finish_timeline_scrub(timeline_fraction, window, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .on_hover(cx.listener(|player, is_hovered: &bool, _window, cx| {
                if !*is_hovered && !player.is_timeline_scrubbing {
                    player.clear_timeline_hover_preview(cx);
                }
            }))
    }

    fn render_timeline_hover_preview(&self, preview: TimelineHoverPreview) -> impl IntoElement {
        let cursor_left = preview.timeline_fraction as f32 * preview.timeline_width_px;
        let max_preview_left = (preview.timeline_width_px - THUMBNAIL_PREVIEW_WIDTH).max(0.0);
        let preview_left =
            (cursor_left - (THUMBNAIL_PREVIEW_WIDTH / 2.0)).clamp(0.0, max_preview_left);
        let thumbnail_path = preview.thumbnail_path.clone();
        let has_thumbnail = thumbnail_path.is_some();

        div()
            .id("timeline-hover-preview")
            .absolute()
            .left(px(preview_left))
            .bottom(px(34.0))
            .w(px(THUMBNAIL_PREVIEW_WIDTH))
            .p_1()
            .bg(rgb_alpha(0x0a0a0a, 0.95))
            .border_1()
            .border_color(rgb(BRIGHT_BORDER))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .w(px(THUMBNAIL_PREVIEW_WIDTH - 8.0))
                    .h(px(THUMBNAIL_PREVIEW_HEIGHT))
                    .bg(rgb(0x111111))
                    .overflow_hidden()
                    .rounded_sm()
                    .when_some(thumbnail_path, |frame, thumbnail_path| {
                        frame.child(
                            img(thumbnail_path)
                                .w(px(THUMBNAIL_PREVIEW_WIDTH - 8.0))
                                .h(px(THUMBNAIL_PREVIEW_HEIGHT))
                                .object_fit(ObjectFit::Cover),
                        )
                    })
                    .when(!has_thumbnail, |frame| {
                        frame
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(rgb(MUTED_TEXT))
                            .child("Preview")
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(SOFT_WHITE))
                    .child(format_timestamp(preview.position_seconds)),
            )
    }

    fn render_transport_controls(&self, cx: &mut Context<Self>) -> Div {
        let play_icon_path = if self.is_playing {
            ICON_PAUSE
        } else {
            ICON_PLAY
        };

        div()
            .flex()
            .items_center()
            .gap_2()
            .text_color(rgb(SOFT_WHITE))
            .child(icon_button(
                "play",
                play_icon_path,
                "Play or pause",
                cx,
                |player, window, cx| {
                    player.toggle_playback(&TogglePlayback, window, cx);
                },
            ))
            .child(icon_button(
                "previous",
                ICON_PREVIOUS,
                "Previous queue item",
                cx,
                |player, window, cx| {
                    player.play_previous_queue_item(window, cx);
                },
            ))
            .child(icon_button(
                "stop",
                ICON_STOP,
                "Stop playback",
                cx,
                |player, window, cx| {
                    player.stop_current_playback(window, cx);
                },
            ))
            .child(icon_button(
                "next",
                ICON_NEXT,
                "Next queue item",
                cx,
                |player, window, cx| {
                    player.play_next_queue_item(window, cx);
                },
            ))
            .child(div().flex_1())
            .child(icon_button(
                "mute",
                if self.is_muted {
                    ICON_VOLUME_MUTED
                } else {
                    ICON_VOLUME
                },
                if self.is_muted { "Unmute" } else { "Mute" },
                cx,
                |player, window, cx| {
                    player.toggle_mute(window, cx);
                },
            ))
            .child(volume_slider(self.volume_percent, cx))
    }

    fn render_live_capture_controls(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap_2()
            .text_color(rgb(SOFT_WHITE))
            .child(div().flex_1())
            .child(icon_button(
                "live-capture-mute",
                if self.is_muted {
                    ICON_VOLUME_MUTED
                } else {
                    ICON_VOLUME
                },
                if self.is_muted { "Unmute" } else { "Mute" },
                cx,
                |player, window, cx| {
                    player.toggle_mute(window, cx);
                },
            ))
            .child(volume_slider(self.volume_percent, cx))
    }

    fn render_playback_mode_toggles(&self, id_prefix: &'static str, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(playback_mode_icon_button(
                format!("{id_prefix}-shuffle"),
                ICON_SHUFFLE,
                if self.is_shuffle_enabled {
                    "Shuffle on".to_string()
                } else {
                    "Shuffle off".to_string()
                },
                self.is_shuffle_enabled,
                cx,
                |player, window, cx| {
                    player.toggle_shuffle(&ToggleShuffle, window, cx);
                },
            ))
            .child(playback_mode_icon_button(
                format!("{id_prefix}-repeat"),
                ICON_REPEAT,
                self.repeat_mode.label().to_string(),
                self.repeat_mode != PlaybackRepeatMode::Off,
                cx,
                |player, window, cx| {
                    player.cycle_repeat_mode(&CycleRepeatMode, window, cx);
                },
            ))
    }

    fn render_main_dropdown(&self, cx: &mut Context<Self>) -> Div {
        let is_queue_open = self.open_context_menu_section == Some(ContextMenuSection::Queue);

        div()
            .absolute()
            .top(px(58.0))
            .right(px(MENU_RIGHT_MARGIN))
            .w(px(MAIN_MENU_WIDTH))
            .p_2()
            .bg(rgb_alpha(0x0a0a0a, 0.95))
            .rounded_sm()
            .border_1()
            .border_color(rgb(BRIGHT_BORDER))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_1()
            .text_color(rgb(SOFT_WHITE))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_1p5()
                    .mb_1()
                    .bg(rgb(PLAYER_BLACK))
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(FINE_BORDER))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(context_icon_button(
                                "main-menu-previous",
                                ICON_PREVIOUS,
                                "Previous queue item",
                                cx,
                                |player, window, cx| {
                                    player.play_previous_queue_item(window, cx);
                                },
                            ))
                            .child(context_icon_button(
                                "main-menu-play",
                                if self.is_playing {
                                    ICON_PAUSE
                                } else {
                                    ICON_PLAY
                                },
                                "Play or pause",
                                cx,
                                |player, window, cx| {
                                    player.toggle_playback(&TogglePlayback, window, cx);
                                },
                            ))
                            .child(context_icon_button(
                                "main-menu-next",
                                ICON_NEXT,
                                "Next queue item",
                                cx,
                                |player, window, cx| {
                                    player.play_next_queue_item(window, cx);
                                },
                            )),
                    )
                    .child(self.render_playback_mode_toggles("main-menu", cx)),
            )
            .child(simple_menu_action_with_icon("Open File", ICON_FILE, cx, |player, window, cx| {
                player.open_file_picker(window, cx);
            }))
            .child(simple_menu_action_with_icon(
                "Open Multiple Files",
                ICON_FILE,
                cx,
                |player, window, cx| {
                    player.open_queue_picker(window, cx);
                },
            ))
            .child(simple_menu_action_with_icon(
                "Open Folder",
                ICON_FOLDER,
                cx,
                |player, window, cx| {
                    player.open_folder_picker(window, cx);
                },
            ))
            .child(context_section_button(
                "Queue",
                self.queue_summary_label(),
                is_queue_open,
                cx,
                |player, window, cx| {
                    player.toggle_context_menu_section(ContextMenuSection::Queue, window, cx);
                },
            ))
            .when(is_queue_open, |menu| {
                menu.child(self.render_queue_selector(cx))
            })
            .child(simple_menu_action_with_icon("Library", ICON_GLOBE, cx, |player, window, cx| {
                player.reveal_library_mode(window, cx);
            }))
            .child(simple_menu_action_with_icon("Settings", ICON_SETTINGS, cx, |player, window, cx| {
                player.open_settings_modal(window, cx);
            }))
    }

    fn subtitle_context_menu_origin(
        &self,
        viewport_width: f32,
        viewport_height: f32,
    ) -> (f32, f32) {
        let fallback_left = viewport_width - CONTEXT_MENU_WIDTH - MENU_RIGHT_MARGIN;
        let fallback_top = viewport_height - CONTEXT_MENU_ESTIMATED_HEIGHT - MENU_RIGHT_MARGIN;
        let (requested_left, requested_top) = self
            .subtitle_menu_anchor
            .map(|anchor| {
                (
                    anchor.x.as_f32() + CONTEXT_MENU_OFFSET,
                    anchor.y.as_f32() + CONTEXT_MENU_OFFSET,
                )
            })
            .unwrap_or((fallback_left, fallback_top));
        let max_left =
            (viewport_width - CONTEXT_MENU_WIDTH - MENU_RIGHT_MARGIN).max(MENU_RIGHT_MARGIN);
        let max_top = (viewport_height - CONTEXT_MENU_ESTIMATED_HEIGHT - MENU_RIGHT_MARGIN)
            .max(MENU_RIGHT_MARGIN);

        (
            clamp_menu_axis(requested_left, MENU_RIGHT_MARGIN, max_left),
            clamp_menu_axis(requested_top, MENU_RIGHT_MARGIN, max_top),
        )
    }

    fn render_subtitle_context_menu(
        &self,
        viewport_width: f32,
        viewport_height: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        let (menu_left, menu_top) =
            self.subtitle_context_menu_origin(viewport_width, viewport_height);
        let is_audio_open = self.open_context_menu_section == Some(ContextMenuSection::Audio);
        let is_subtitles_open =
            self.open_context_menu_section == Some(ContextMenuSection::Subtitles);
        let is_open_media_open =
            self.open_context_menu_section == Some(ContextMenuSection::OpenMedia);
        let is_queue_open = self.open_context_menu_section == Some(ContextMenuSection::Queue);
        let is_live_capture = self.is_current_media_live_capture();

        div()
            .absolute()
            .left(px(menu_left))
            .top(px(menu_top))
            .w(px(CONTEXT_MENU_WIDTH))
            .p_2()
            .bg(rgb_alpha(0x0a0a0a, 0.95))
            .rounded_sm()
            .border_1()
            .border_color(rgb(BRIGHT_BORDER))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_1()
            .text_color(rgb(SOFT_WHITE))
            .when(!is_live_capture, |menu| {
                menu.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .p_1p5()
                        .mb_1()
                        .bg(rgb(PLAYER_BLACK))
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(FINE_BORDER))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(context_icon_button(
                                    "context-play",
                                    if self.is_playing {
                                        ICON_PAUSE
                                    } else {
                                        ICON_PLAY
                                    },
                                    "Play or pause",
                                    cx,
                                    |player, window, cx| {
                                        player.toggle_playback(&TogglePlayback, window, cx);
                                    },
                                ))
                                .child(context_icon_button(
                                    "context-previous",
                                    ICON_PREVIOUS,
                                    "Previous queue item",
                                    cx,
                                    |player, window, cx| {
                                        player.play_previous_queue_item(window, cx);
                                    },
                                ))
                                .child(context_icon_button(
                                    "context-next",
                                    ICON_NEXT,
                                    "Next queue item",
                                    cx,
                                    |player, window, cx| {
                                        player.play_next_queue_item(window, cx);
                                    },
                                )),
                        )
                        .child(self.render_playback_mode_toggles("context-menu", cx)),
                )
            })
            .child(context_section_button(
                "Audio",
                self.selected_audio_track_label(),
                is_audio_open,
                cx,
                |player, window, cx| {
                    player.toggle_context_menu_section(ContextMenuSection::Audio, window, cx);
                },
            ))
            .when(is_audio_open, |menu| {
                menu.child(self.render_audio_track_selector(cx))
            })
            .child(context_section_button(
                "Subtitles",
                self.selected_subtitle_label(),
                is_subtitles_open,
                cx,
                |player, window, cx| {
                    player.toggle_context_menu_section(ContextMenuSection::Subtitles, window, cx);
                },
            ))
            .when(is_subtitles_open, |menu| {
                menu.child(self.render_subtitle_track_selector(cx))
            })
            .child(context_section_button(
                "Open Media",
                "Open file, multiple files, or folder".to_string(),
                is_open_media_open,
                cx,
                |player, window, cx| {
                    player.toggle_context_menu_section(ContextMenuSection::OpenMedia, window, cx);
                },
            ))
            .when(is_open_media_open, |menu| {
                menu.child(self.render_open_media_selector(cx))
            })
            .child(context_section_button(
                "Queue",
                self.queue_summary_label(),
                is_queue_open,
                cx,
                |player, window, cx| {
                    player.toggle_context_menu_section(ContextMenuSection::Queue, window, cx);
                },
            ))
            .when(is_queue_open, |menu| {
                menu.child(self.render_queue_selector(cx))
            })
    }

    fn render_source_search_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let provider_count = available_source_provider_count(&self.settings);
        let has_source_providers = provider_count > 0;
        let status_message = self
            .source_search_status
            .clone()
            .unwrap_or_else(|| "Search source providers.".into());

        div()
            .id("source-search-overlay")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .child(
                div()
                    .id("source-search-clickoff")
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .bg(gpui::transparent_black())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|player, _event, _window, cx| {
                            player.close_source_search_overlay(cx);
                            cx.stop_propagation();
                        }),
                    ),
            )
            .child(
                div()
                    .id("source-search-powerbar")
                    .absolute()
                    .top(px(18.0))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .w(px(SOURCE_SEARCH_BAR_WIDTH))
                            .max_w(relative(0.92))
                            .p_3()
                            .bg(self.settings.surface_background_color(
                                MENU_BLACK,
                                BACKDROP_BLUR_MODAL_BACKGROUND_ALPHA,
                            ))
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(BRIGHT_BORDER))
                            .shadow_lg()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .text_color(rgb(SOFT_WHITE))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|_player, _event, _window, cx| {
                                    cx.stop_propagation();
                                }),
                            )
                            .on_scroll_wheel(cx.listener(
                                |_player, _event: &ScrollWheelEvent, _window, cx| {
                                    cx.stop_propagation();
                                },
                            ))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        svg()
                                            .external_path(crate::icon_path(ICON_SEARCH))
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .text_color(rgb(SOFT_WHITE)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(if has_source_providers {
                                                self.source_search_input.clone().into_any_element()
                                            } else {
                                                self.source_provider_input
                                                    .clone()
                                                    .into_any_element()
                                            }),
                                    )
                                    .when(has_source_providers, |row| {
                                        row.child(prompt_action_button(
                                            "source-search-submit",
                                            if self.is_source_search_pending {
                                                "Searching"
                                            } else {
                                                "Search"
                                            },
                                            true,
                                            1.0,
                                            cx,
                                            |player, window, cx| {
                                                player.search_source_providers(window, cx);
                                            },
                                        ))
                                    })
                                    .when(!has_source_providers, |row| {
                                        row.child(prompt_action_button(
                                            "source-provider-add-from-search",
                                            "Add Source",
                                            true,
                                            1.0,
                                            cx,
                                            |player, window, cx| {
                                                player.add_source_provider_from_input(window, cx);
                                            },
                                        ))
                                    })
                                    .child(context_icon_button(
                                        "source-search-close",
                                        ICON_X,
                                        "Close search",
                                        cx,
                                        |player, _window, cx| {
                                            player.close_source_search_overlay(cx);
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_xs()
                                    .text_color(rgb(MUTED_TEXT))
                                    .child(status_message)
                                    .when(provider_count > 0, |row| {
                                        row.child(div().flex_1()).child(format!(
                                            "{provider_count} provider(s)"
                                        ))
                                    }),
                            )
                            .when(!has_source_providers, |bar| {
                                bar.child(
                                    div()
                                        .text_xs()
                                        .line_height(px(16.0))
                                        .text_color(rgb(MUTED_TEXT))
                                        .child("Use templates with {query}, {id}, and {episode_id}; JSON can return series, episodes, streams, or direct media."),
                                )
                            })
                            .when(has_source_providers, |bar| {
                                bar.child(self.render_source_browser_navigation(cx))
                                    .child(self.render_source_browser_content(cx))
                            }),
                    ),
            )
    }

    fn render_source_browser_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("source-browser-navigation")
            .flex()
            .items_center()
            .gap_2()
            .when(
                self.source_browser_view == SourceBrowserView::Episodes,
                |row| {
                    row.child(source_back_button(
                        "source-back-to-search",
                        "Search",
                        cx,
                        |player, _window, cx| {
                            player.show_source_search_results(cx);
                        },
                    ))
                },
            )
            .when(
                self.source_browser_view == SourceBrowserView::Streams,
                |row| {
                    row.child(source_back_button(
                        "source-back-to-episodes",
                        if self.source_episode_results.is_empty() {
                            "Search"
                        } else {
                            "Episodes"
                        },
                        cx,
                        |player, _window, cx| {
                            if player.source_episode_results.is_empty() {
                                player.show_source_search_results(cx);
                            } else {
                                player.show_source_episode_results(cx);
                            }
                        },
                    ))
                },
            )
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(rgb(MUTED_TEXT))
                    .truncate()
                    .child(self.source_browser_title()),
            )
    }

    fn source_browser_title(&self) -> String {
        match self.source_browser_view {
            SourceBrowserView::SearchResults => "Search results".to_string(),
            SourceBrowserView::Episodes => self
                .selected_source_series_title
                .as_ref()
                .map(|title| format!("Episodes: {title}"))
                .unwrap_or_else(|| "Episodes".to_string()),
            SourceBrowserView::Streams => self
                .selected_source_episode_title
                .as_ref()
                .map(|title| format!("Streams: {title}"))
                .or_else(|| {
                    self.selected_source_series_title
                        .as_ref()
                        .map(|title| format!("Streams: {title}"))
                })
                .unwrap_or_else(|| "Streams".to_string()),
        }
    }

    fn render_source_browser_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        match self.source_browser_view {
            SourceBrowserView::SearchResults => {
                let current_input = self.source_search_input.read(cx).text().trim().to_string();
                let show_suggestions = current_input != self.last_source_search_query
                    || self.last_source_search_query.is_empty();

                if show_suggestions {
                    self.render_type_ahead_suggestions(cx).into_any_element()
                } else {
                    self.render_source_search_results(cx).into_any_element()
                }
            }
            SourceBrowserView::Episodes => {
                self.render_source_episode_results(cx).into_any_element()
            }
            SourceBrowserView::Streams => self.render_source_stream_results(cx).into_any_element(),
        }
    }

    fn render_source_search_results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("source-search-results")
            .max_h(px(SOURCE_SEARCH_RESULT_MAX_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&self.source_search_scroll_handle)
            .scrollbar_width(px(4.0))
            .flex()
            .flex_col()
            .gap_1()
            .when(
                !self.is_source_search_pending && self.source_search_results.is_empty(),
                |results| results.child(empty_menu_message("No source results yet.")),
            )
            .children(self.source_search_results.iter().cloned().enumerate().map(
                |(index, search_result)| {
                    self.render_source_search_result_row(
                        search_result,
                        index == self.selected_source_result_index,
                        cx,
                    )
                },
            ))
    }

    fn render_source_search_result_row(
        &self,
        search_result: SourceSearchResult,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let result_for_click = search_result.clone();
        let result_id = source_search_result_key(&search_result);
        let title = search_result.title.clone();
        let provider_name = search_result.provider.name.clone();
        let thumbnail_url = search_result.thumbnail_url.clone();
        let has_thumbnail_url = thumbnail_url.is_some();
        let kind_label = source_search_result_kind_label(&search_result);
        let detail = search_result
            .subtitle
            .clone()
            .unwrap_or_else(|| kind_label.to_string());

        div()
            .id(format!("source-search-result-{result_id}"))
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .py_2()
            .rounded_sm()
            .border_1()
            .border_color(if is_selected {
                rgb(BRIGHT_BORDER)
            } else {
                rgb_alpha(SOFT_WHITE, 0.0)
            })
            .cursor_pointer()
            .when(is_selected, |row| row.bg(rgb(0x161616)))
            .hover(|row| row.bg(rgb(0x121212)))
            .on_click(cx.listener(move |player, _, window, cx| {
                player.load_source_search_result(result_for_click.clone(), window, cx);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .w(px(42.0))
                    .h(px(42.0))
                    .flex_none()
                    .rounded_sm()
                    .bg(rgb(0x101010))
                    .border_1()
                    .border_color(rgb(FINE_BORDER))
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .when_some(thumbnail_url, |thumbnail, thumbnail_url| {
                        thumbnail.child(
                            img(thumbnail_url)
                                .w_full()
                                .h_full()
                                .object_fit(ObjectFit::Cover),
                        )
                    })
                    .when(!has_thumbnail_url, |thumbnail| {
                        thumbnail.child(
                            svg()
                                .external_path(crate::icon_path(ICON_GLOBE))
                                .w(px(18.0))
                                .h(px(18.0))
                                .text_color(rgb(SOFT_WHITE)),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().truncate().child(title))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED_TEXT))
                            .truncate()
                            .child(detail),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MUTED_TEXT))
                    .child(format!("{provider_name} / {kind_label}")),
            )
    }

    fn render_source_episode_results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("source-episode-results")
            .max_h(px(SOURCE_SEARCH_RESULT_MAX_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&self.source_episode_scroll_handle)
            .scrollbar_width(px(4.0))
            .flex()
            .flex_col()
            .gap_1()
            .when(
                !self.is_source_search_pending && self.source_episode_results.is_empty(),
                |results| results.child(empty_menu_message("No episodes found.")),
            )
            .children(self.source_episode_results.iter().cloned().enumerate().map(
                |episode_result| {
                    let (index, episode_result) = episode_result;
                    self.render_source_episode_result_row(
                        episode_result,
                        index == self.selected_source_result_index,
                        cx,
                    )
                },
            ))
    }

    fn render_source_episode_result_row(
        &self,
        episode_result: SourceEpisodeResult,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let episode_for_click = episode_result.clone();
        let episode_id = source_episode_result_key(&episode_result);
        let title = episode_result.title.clone();
        let kind_label = source_episode_result_kind_label(&episode_result);
        let detail = episode_result
            .subtitle
            .clone()
            .unwrap_or_else(|| kind_label.to_string());

        div()
            .id(format!("source-episode-result-{episode_id}"))
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .py_2()
            .rounded_sm()
            .border_1()
            .border_color(if is_selected {
                rgb(BRIGHT_BORDER)
            } else {
                rgb_alpha(SOFT_WHITE, 0.0)
            })
            .cursor_pointer()
            .when(is_selected, |row| row.bg(rgb(0x161616)))
            .hover(|row| row.bg(rgb(0x121212)))
            .on_click(cx.listener(move |player, _, window, cx| {
                player.load_source_episode_result(episode_for_click.clone(), window, cx);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .w(px(42.0))
                    .h(px(42.0))
                    .flex_none()
                    .rounded_sm()
                    .bg(rgb(0x101010))
                    .border_1()
                    .border_color(rgb(FINE_BORDER))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(SOFT_WHITE))
                    .child("Ep"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().truncate().child(title))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED_TEXT))
                            .truncate()
                            .child(detail),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(MUTED_TEXT))
                    .child(kind_label),
            )
    }

    fn render_source_stream_results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("source-stream-results")
            .max_h(px(SOURCE_SEARCH_RESULT_MAX_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&self.source_stream_scroll_handle)
            .scrollbar_width(px(4.0))
            .flex()
            .flex_col()
            .gap_1()
            .when(
                !self.is_source_search_pending && self.source_stream_results.is_empty(),
                |results| results.child(empty_menu_message("No playable streams found.")),
            )
            .children(self.source_stream_results.iter().cloned().enumerate().map(
                |(index, stream_result)| {
                    self.render_source_stream_result_row(
                        stream_result,
                        index == self.selected_source_result_index,
                        cx,
                    )
                },
            ))
    }

    fn render_source_stream_result_row(
        &self,
        stream_result: SourceStreamResult,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let stream_for_click = stream_result.clone();
        let stream_id = internet_media_key(&stream_result.media);
        let title = stream_result.media.title.clone();
        let detail = stream_result
            .media
            .subtitle
            .clone()
            .unwrap_or_else(|| stream_result.media.stream_url.clone());
        let quality = stream_result
            .quality
            .clone()
            .unwrap_or_else(|| "Stream".to_string());

        div()
            .id(format!("source-stream-result-{stream_id}"))
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .py_2()
            .rounded_sm()
            .border_1()
            .border_color(if is_selected {
                rgb(BRIGHT_BORDER)
            } else {
                rgb_alpha(SOFT_WHITE, 0.0)
            })
            .cursor_pointer()
            .when(is_selected, |row| row.bg(rgb(0x161616)))
            .hover(|row| row.bg(rgb(0x121212)))
            .on_click(cx.listener(move |player, _, window, cx| {
                player.load_source_stream_result(stream_for_click.clone(), window, cx);
                cx.stop_propagation();
            }))
            .child(
                div()
                    .w(px(42.0))
                    .h(px(42.0))
                    .flex_none()
                    .rounded_sm()
                    .bg(rgb(0x101010))
                    .border_1()
                    .border_color(rgb(FINE_BORDER))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .external_path(crate::icon_path(ICON_PLAY))
                            .w(px(18.0))
                            .h(px(18.0))
                            .text_color(rgb(SOFT_WHITE)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().truncate().child(title))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED_TEXT))
                            .truncate()
                            .child(detail),
                    ),
            )
            .child(div().text_xs().text_color(rgb(MUTED_TEXT)).child(quality))
    }

    fn render_continue_watching_prompt(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let media_title = self
            .pending_watch_session
            .as_ref()
            .map(|watch_session| display_name(&watch_session.current_media_path))
            .unwrap_or_else(|| "Saved media".to_string());
        let position_label = self
            .pending_watch_session
            .as_ref()
            .map(|watch_session| {
                format!(
                    "Resume from {}",
                    format_timestamp(watch_session.playback_position_seconds)
                )
            })
            .unwrap_or_else(|| "Resume from where you left off".to_string());

        div()
            .id("continue-watching-overlay")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(if self.settings.is_backdrop_blur_enabled {
                rgb_alpha(OLED_BLACK, BACKDROP_BLUR_MODAL_BACKDROP_ALPHA)
            } else {
                rgb_alpha(OLED_BLACK, 0.78)
            })
            .child(
                div()
                    .id("continue-watching-dialog")
                    .w(px(CONTINUE_WATCHING_PROMPT_WIDTH))
                    .p_4()
                    .bg(self
                        .settings
                        .surface_background_color(MENU_BLACK, BACKDROP_BLUR_MODAL_BACKGROUND_ALPHA))
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(BRIGHT_BORDER))
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .text_color(rgb(SOFT_WHITE))
                    .child(div().text_lg().child("Continue Watching?"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_sm().truncate().child(media_title))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MUTED_TEXT))
                                    .child(position_label),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1())
                            .child(prompt_action_button(
                                "continue-watching-no",
                                "No",
                                false,
                                1.0,
                                cx,
                                |player, window, cx| {
                                    player.decline_saved_watch_session(window, cx);
                                },
                            ))
                            .child(prompt_action_button(
                                "continue-watching-yes",
                                "Yes",
                                true,
                                1.0,
                                cx,
                                |player, window, cx| {
                                    player.continue_saved_watch_session(window, cx);
                                },
                            )),
                    ),
            )
    }

    fn render_settings_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let tabs = [
            SettingsTab::General,
            SettingsTab::Audio,
            SettingsTab::Subtitles,
            SettingsTab::Providers,
        ];

        div()
            .w(px(180.0))
            .h_full()
            .bg(self.settings.surface_background_color(PLAYER_BLACK, 0.24))
            .p_2()
            .flex()
            .flex_col()
            .gap_1()
            .children(
                tabs.into_iter().map(|tab| {
                    let is_active = self.active_settings_tab == tab;
                    div()
                        .id(format!("settings-tab-{}", tab.label()))
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .rounded_sm()
                        .cursor_pointer()
                        .bg(if is_active {
                            rgb_alpha(VLC_ORANGE, 0.08)
                        } else {
                            rgb_alpha(OLED_BLACK, 0.0)
                        })
                        .hover(|style| {
                            if !is_active {
                                style.bg(rgb(0x121212))
                            } else {
                                style
                            }
                        })
                        .on_click(cx.listener(move |player, _, _window, cx| {
                            player.active_settings_tab = tab;
                            cx.notify();
                        }))
                        .child(
                            svg()
                                .external_path(crate::icon_path(tab.icon()))
                                .w(px(16.0))
                                .h(px(16.0))
                                .text_color(if is_active { rgb(VLC_ORANGE) } else { rgb(SOFT_WHITE) })
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(if is_active { rgb(VLC_ORANGE) } else { rgb(SOFT_WHITE) })
                                .child(tab.label())
                        )
                })
            )
    }

    fn render_settings_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-content-pane")
            .flex_1()
            .h_full()
            .p_4()
            .overflow_y_scroll()
            .scrollbar_width(px(4.0))
            .flex()
            .flex_col()
            .gap_4()
            .when(self.active_settings_tab == SettingsTab::General, |pane| {
                pane.child(self.render_general_tab_content(cx))
            })
            .when(self.active_settings_tab == SettingsTab::Audio, |pane| {
                pane.child(self.render_audio_tab_content(cx))
            })
            .when(self.active_settings_tab == SettingsTab::Subtitles, |pane| {
                pane.child(self.render_subtitles_tab_content(cx))
            })
            .when(self.active_settings_tab == SettingsTab::Providers, |pane| {
                pane.child(self.render_source_provider_settings_section(cx))
            })
    }

    fn render_settings_toggle(
        &self,
        id: &'static str,
        label: &'static str,
        description: &'static str,
        value: bool,
        cx: &mut Context<Self>,
        on_toggle: impl Fn(&mut WatchPlayer, &mut Window, &mut Context<WatchPlayer>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .rounded_sm()
            .hover(|style| style.bg(rgb(0x121212)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap_0p5()
                    .child(div().text_sm().child(label))
                    .child(div().text_xs().text_color(rgb(MUTED_TEXT)).child(description))
            )
            .child(
                div()
                    .id(format!("{}-switch", id))
                    .w(px(36.0))
                    .h(px(20.0))
                    .rounded_full()
                    .p_0p5()
                    .cursor_pointer()
                    .bg(if value { rgb(VLC_ORANGE) } else { rgb(0x282828) })
                    .on_click(cx.listener(move |player, _event, window, cx| {
                        on_toggle(player, window, cx);
                    }))
                    .child(
                        div()
                            .w(px(14.0))
                            .h(px(14.0))
                            .rounded_full()
                            .bg(rgb(SOFT_WHITE))
                            .when(value, |thumb| thumb.ml(px(16.0)))
                    )
            )
    }

    fn render_settings_segments<T: Clone + PartialEq + 'static>(
        &self,
        id: &'static str,
        label: &'static str,
        description: &'static str,
        options: Vec<(String, T)>,
        current_value: T,
        cx: &mut Context<Self>,
        on_change: impl Fn(&mut WatchPlayer, T, &mut Window, &mut Context<WatchPlayer>) + 'static,
    ) -> impl IntoElement {
        let on_change = Arc::new(on_change);
        div()
            .id(id)
            .flex()
            .flex_col()
            .gap_1p5()
            .px_3()
            .py_2()
            .rounded_sm()
            .hover(|style| style.bg(rgb(0x121212)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(div().text_sm().child(label))
                    .child(div().text_xs().text_color(rgb(MUTED_TEXT)).child(description))
            )
            .child(
                div()
                    .flex()
                    .bg(self.settings.surface_background_color(PLAYER_BLACK, 0.45))
                    .p_0p5()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(FINE_BORDER))
                    .children(
                        options.into_iter().enumerate().map(|(idx, (option_label, option_val))| {
                            let is_selected = option_val == current_value;
                            let on_change = Arc::clone(&on_change);
                            let option_val_clone = option_val.clone();
                            div()
                                .id(format!("{}-opt-{}", id, idx))
                                .flex_1()
                                .flex()
                                .items_center()
                                .justify_center()
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .cursor_pointer()
                                .bg(if is_selected {
                                    rgb_alpha(VLC_ORANGE, 0.12)
                                } else {
                                    rgb_alpha(OLED_BLACK, 0.0)
                                })
                                .hover(|style| {
                                    if !is_selected {
                                        style.bg(rgb(0x1a1a1a))
                                    } else {
                                        style
                                    }
                                })
                                .on_click(cx.listener(move |player, _event, window, cx| {
                                    on_change(player, option_val_clone.clone(), window, cx);
                                }))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if is_selected {
                                            rgb(VLC_ORANGE)
                                        } else {
                                            rgb(SOFT_WHITE)
                                        })
                                        .child(option_label)
                                )
                        })
                    )
            )
    }

    fn render_subtitle_color_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = vec![
            ("#FFFFFF", "White", 0xffffff),
            ("#FFD27A", "Yellow", 0xffd27a),
            ("#80D8FF", "Blue", 0x80d8ff),
            ("#B6FFB0", "Green", 0xb6ffb0),
        ];
        let current_color = self.settings.subtitle_color.clone();

        div()
            .id("subtitle-color-selector")
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .rounded_sm()
            .hover(|style| style.bg(rgb(0x121212)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap_0p5()
                    .child(div().text_sm().child("Subtitle Color"))
                    .child(div().text_xs().text_color(rgb(MUTED_TEXT)).child("Foreground color of the subtitles."))
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .children(
                        colors.into_iter().map(|(hex, name, rgb_val)| {
                            let is_selected = current_color == hex;
                            div()
                                .id(format!("color-{}", name))
                                .w(px(22.0))
                                .h(px(22.0))
                                .rounded_full()
                                .border_2()
                                .border_color(if is_selected { rgb(VLC_ORANGE) } else { rgb_alpha(OLED_BLACK, 0.0) })
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .hover(|style| style.border_color(rgb(VLC_ORANGE)))
                                .on_click(cx.listener(move |player, _event, window, cx| {
                                    player.settings.subtitle_color = hex.to_string();
                                    save_player_settings(&player.settings);
                                    player.apply_subtitle_style_in_backend();
                                    player.show_osd(format!("Subtitle color {name}"), window, cx);
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .w(px(14.0))
                                        .h(px(14.0))
                                        .rounded_full()
                                        .bg(rgb(rgb_val))
                                )
                        })
                    )
            )
    }

    fn render_default_volume_row_new(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-default-volume-row")
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .rounded_sm()
            .hover(|style| style.bg(rgb(0x121212)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap_0p5()
                    .child(div().text_sm().child("Default Volume"))
                    .child(div().text_xs().text_color(rgb(MUTED_TEXT)).child("Initial volume level for newly loaded media files."))
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(default_volume_slider(
                        self.settings.default_volume_percent,
                        cx,
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED_TEXT))
                            .w(px(36.0))
                            .text_right()
                            .child(format!("{}%", self.settings.default_volume_percent)),
                    )
            )
    }

    fn render_general_tab_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let resume_options = vec![
            ("Ask".to_string(), ResumeBehavior::Ask),
            ("Always".to_string(), ResumeBehavior::Always),
            ("Never".to_string(), ResumeBehavior::Never),
        ];
        let seek_options = vec![
            ("3s".to_string(), 3.0),
            ("5s".to_string(), 5.0),
            ("10s".to_string(), 10.0),
            ("30s".to_string(), 30.0),
        ];
        let volume_options = vec![
            ("1%".to_string(), 1),
            ("2%".to_string(), 2),
            ("5%".to_string(), 5),
            ("10%".to_string(), 10),
        ];

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.render_settings_toggle(
                "toggle-start-fullscreen",
                "Start Fullscreen",
                "Start playback in fullscreen mode by default.",
                self.settings.start_fullscreen,
                cx,
                |player, window, cx| {
                    player.set_start_fullscreen_setting(!player.settings.start_fullscreen, window, cx);
                }
            ))
            .child(self.render_settings_toggle(
                "toggle-backdrop-blur",
                "Backdrop Blur",
                "Enable blurred glass visual effects behind windows.",
                self.settings.is_backdrop_blur_enabled,
                cx,
                |player, window, cx| {
                    player.set_backdrop_blur_setting(!player.settings.is_backdrop_blur_enabled, window, cx);
                }
            ))
            .child(self.render_settings_segments(
                "segments-resume",
                "Resume Playback",
                "Control how playback position is restored when reopening media.",
                resume_options,
                self.settings.resume_behavior,
                cx,
                |player, val, window, cx| {
                    player.set_resume_behavior_setting(val, window, cx);
                }
            ))
            .child(self.render_settings_segments(
                "segments-seek-step",
                "Seek Step",
                "Interval when seeking forward or backward with arrow keys.",
                seek_options,
                self.settings.seek_step_seconds,
                cx,
                |player, val, window, cx| {
                    player.settings.seek_step_seconds = val;
                    player.save_settings_and_show(format!("Seek step {} seconds", val), window, cx);
                }
            ))
            .child(self.render_settings_segments(
                "segments-volume-step",
                "Volume Step",
                "Percentage change when adjusting volume with mouse scroll.",
                volume_options,
                self.settings.volume_step_percent,
                cx,
                |player, val, window, cx| {
                    player.settings.volume_step_percent = val;
                    player.save_settings_and_show(format!("Volume step {}%", val), window, cx);
                }
            ))
    }

    fn render_audio_tab_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let lang_options = vec![
            ("English".to_string(), "eng".to_string()),
            ("Japanese".to_string(), "jpn".to_string()),
            ("Spanish".to_string(), "spa".to_string()),
            ("French".to_string(), "fra".to_string()),
            ("Any".to_string(), "any".to_string()),
        ];

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.render_default_volume_row_new(cx))
            .child(self.render_settings_toggle(
                "toggle-live-lowest-latency",
                "Live Lowest Latency",
                "Reduce audio and capture delay for live capture feeds.",
                self.settings.is_lowest_latency_live_capture_enabled,
                cx,
                |player, window, cx| {
                    player.set_lowest_latency_live_capture_setting(!player.settings.is_lowest_latency_live_capture_enabled, window, cx);
                }
            ))
            .child(self.render_settings_toggle(
                "toggle-live-exclusive-audio",
                "Exclusive Audio Capture",
                "Bypass the system mixer to stream audio directly.",
                self.settings.is_live_capture_exclusive_audio_enabled,
                cx,
                |player, window, cx| {
                    player.set_live_capture_exclusive_audio_setting(!player.settings.is_live_capture_exclusive_audio_enabled, window, cx);
                }
            ))
            .child(self.render_settings_segments(
                "segments-audio-lang",
                "Preferred Audio Language",
                "Default language for multi-language media audio tracks.",
                lang_options,
                self.settings.preferred_audio_language.clone(),
                cx,
                |player, val, window, cx| {
                    player.set_preferred_audio_language_setting(val, window, cx);
                }
            ))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_sm().child("Audio Output Device"))
                    .child(div().text_xs().text_color(rgb(MUTED_TEXT)).child("Select default device for audio playback."))
                    .child(self.render_audio_device_inline_list(cx))
            )
    }

    fn render_audio_device_inline_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_device_id = &self.settings.audio_output_device;
        div()
            .flex()
            .flex_col()
            .gap_1()
            .p_1()
            .bg(self.settings.surface_background_color(PLAYER_BLACK, 0.45))
            .rounded_sm()
            .border_1()
            .border_color(rgb(FINE_BORDER))
            .children(
                self.audio_output_devices.iter().map(|device| {
                    let is_selected = device.device_id == *current_device_id;
                    div()
                        .id(format!("audio-device-{}", library_safe_element_key(&device.device_id)))
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .cursor_pointer()
                        .bg(if is_selected {
                            rgb_alpha(VLC_ORANGE, 0.12)
                        } else {
                            rgb_alpha(OLED_BLACK, 0.0)
                        })
                        .hover(|style| {
                            if !is_selected {
                                style.bg(rgb(0x1a1a1a))
                            } else {
                                style
                            }
                        })
                        .on_click(cx.listener({
                            let device_id = device.device_id.clone();
                            move |player, _event, window, cx| {
                                player.set_audio_output_device(device_id.clone(), window, cx);
                            }
                        }))
                        .child(div().text_xs().text_color(if is_selected { rgb(VLC_ORANGE) } else { rgb(SOFT_WHITE) }).child(device.label.clone()))
                        .when(is_selected, |row| {
                            row.child(div().text_xs().text_color(rgb(VLC_ORANGE)).child("Selected"))
                        })
                })
            )
    }

    fn render_subtitles_tab_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let hw_options = vec![
            ("Auto Safe".to_string(), "auto-safe".to_string()),
            ("D3D11VA".to_string(), "d3d11va".to_string()),
            ("Off".to_string(), "no".to_string()),
        ];
        let font_size_options = vec![
            ("42px".to_string(), 42),
            ("48px".to_string(), 48),
            ("60px".to_string(), 60),
            ("72px".to_string(), 72),
        ];
        let position_options = vec![
            ("85%".to_string(), 85),
            ("90%".to_string(), 90),
            ("95%".to_string(), 95),
            ("100%".to_string(), 100),
        ];
        let sub_lang_options = vec![
            ("English".to_string(), "eng".to_string()),
            ("Japanese".to_string(), "jpn".to_string()),
            ("Spanish".to_string(), "spa".to_string()),
            ("French".to_string(), "fra".to_string()),
            ("Any".to_string(), "any".to_string()),
        ];

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.render_settings_segments(
                "segments-hw-decode",
                "Hardware Decode Mode",
                "Choose API priority for decoding video streams on your GPU.",
                hw_options,
                self.settings.hardware_decoding_mode.clone(),
                cx,
                |player, val, window, cx| {
                    player.set_hardware_decoding_setting(val, window, cx);
                }
            ))
            .child(self.render_settings_segments(
                "segments-subtitle-font-size",
                "Subtitle Font Size",
                "Adjust text size of subtitles rendered over the video stream.",
                font_size_options,
                self.settings.subtitle_font_size,
                cx,
                |player, val, window, cx| {
                    player.settings.subtitle_font_size = val;
                    save_player_settings(&player.settings);
                    player.apply_subtitle_style_in_backend();
                    player.show_osd(format!("Subtitle size {}", val), window, cx);
                }
            ))
            .child(self.render_subtitle_color_selector(cx))
            .child(self.render_settings_segments(
                "segments-subtitle-position",
                "Subtitle Position",
                "Set subtitle vertical coordinate alignment percent.",
                position_options,
                self.settings.subtitle_position_percent,
                cx,
                |player, val, window, cx| {
                    player.settings.subtitle_position_percent = val;
                    save_player_settings(&player.settings);
                    player.apply_subtitle_style_in_backend();
                    player.show_osd(format!("Subtitle position {}%", val), window, cx);
                }
            ))
            .child(self.render_settings_segments(
                "segments-sub-lang",
                "Preferred Subtitle Language",
                "Select subtitle language code when media is loaded.",
                sub_lang_options,
                self.settings.preferred_subtitle_language.clone(),
                cx,
                |player, val, window, cx| {
                    player.set_preferred_subtitle_language_setting(val, window, cx);
                }
            ))
    }

    fn render_settings_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-modal-backdrop")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb_alpha(OLED_BLACK, 0.92))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|player, _event, _window, cx| {
                    player.close_settings_modal(cx);
                }),
            )
            .child(
                div()
                    .id("settings-modal")
                    .w(px(700.0))
                    .h(px(460.0))
                    .bg(self
                        .settings
                        .surface_background_color(MENU_BLACK, BACKDROP_BLUR_MODAL_BACKGROUND_ALPHA))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(BRIGHT_BORDER))
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_player, _event, _window, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .px_4()
                            .py_3()
                            .border_b_1()
                            .border_color(rgb(FINE_BORDER))
                            .child(div().text_lg().flex_1().child("Settings"))
                            .child(context_icon_button(
                                "settings-close",
                                ICON_X,
                                "Close settings",
                                cx,
                                |player, _window, cx| {
                                    player.close_settings_modal(cx);
                                },
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .overflow_hidden()
                            .child(self.render_settings_sidebar(cx))
                            .child(
                                div()
                                    .w(px(1.0))
                                    .h_full()
                                    .bg(rgb(FINE_BORDER))
                            )
                            .child(self.render_settings_content(cx))
                    )
            )
            .when_some(self.open_settings_selector, |backdrop, selector_kind| {
                backdrop.child(self.render_settings_selector_overlay(selector_kind, cx))
            })
    }

    fn render_audio_track_selector(&self, cx: &mut Context<Self>) -> Div {
        let audio_tracks = self
            .current_media()
            .map(|media| media.audio_tracks.as_slice())
            .unwrap_or(&[]);

        div()
            .flex()
            .flex_col()
            .gap_0()
            .pl_2()
            .when(self.current_media().is_none(), |menu| {
                menu.child(empty_menu_message("No media loaded."))
            })
            .when(
                self.current_media().is_some() && audio_tracks.is_empty(),
                |menu| menu.child(empty_menu_message("No audio tracks found.")),
            )
            .children(
                audio_tracks
                    .iter()
                    .cloned()
                    .map(|audio_track| self.render_audio_track_row(audio_track, cx)),
            )
    }

    fn render_subtitle_track_selector(&self, cx: &mut Context<Self>) -> Div {
        let embedded_subtitle_tracks = self
            .current_media()
            .map(|media| media.embedded_subtitle_tracks.as_slice())
            .unwrap_or(&[]);
        let subtitle_paths = self
            .current_media()
            .map(|media| media.subtitle_paths.as_slice())
            .unwrap_or(&[]);
        let has_subtitle_options =
            !embedded_subtitle_tracks.is_empty() || !subtitle_paths.is_empty();

        div()
            .flex()
            .flex_col()
            .gap_0()
            .pl_2()
            .when(self.current_media().is_none(), |menu| {
                menu.child(empty_menu_message("No media loaded."))
            })
            .when(
                self.current_media().is_some() && !has_subtitle_options,
                |menu| menu.child(empty_menu_message("No subtitle tracks found.")),
            )
            .children(
                embedded_subtitle_tracks
                    .iter()
                    .cloned()
                    .map(|subtitle_track| {
                        self.render_embedded_subtitle_track_row(subtitle_track, cx)
                    }),
            )
            .children(
                subtitle_paths
                    .iter()
                    .cloned()
                    .map(|subtitle_path| self.render_subtitle_path_row(subtitle_path, cx)),
            )
            .when(has_subtitle_options, |menu| {
                menu.child(simple_menu_action("Off", cx, |player, window, cx| {
                    player.disable_subtitles(window, cx);
                }))
            })
            .child(simple_menu_action(
                "Reset Delay",
                cx,
                |player, window, cx| {
                    player.reset_subtitle_delay(&ResetSubtitleDelay, window, cx);
                },
            ))
            .child(self.render_settings_action_row(
                "Size",
                self.settings.subtitle_font_size.to_string(),
                cx,
                |player, window, cx| {
                    player.cycle_subtitle_size(window, cx);
                },
            ))
            .child(self.render_settings_action_row(
                "Color",
                self.settings.subtitle_color.clone(),
                cx,
                |player, window, cx| {
                    player.cycle_subtitle_color(window, cx);
                },
            ))
            .child(self.render_settings_action_row(
                "Position",
                format!("{}%", self.settings.subtitle_position_percent),
                cx,
                |player, window, cx| {
                    player.cycle_subtitle_position(window, cx);
                },
            ))
            .child(simple_menu_action(
                "Load Subtitle File",
                cx,
                |player, window, cx| {
                    player.open_subtitle_file_picker(window, cx);
                },
            ))
            .child(simple_menu_action(
                "Search Subtitles",
                cx,
                |player, _window, cx| {
                    player.open_subtitle_search_hook(cx);
                },
            ))
    }

    fn render_open_media_selector(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_0()
            .pl_2()
            .child(simple_menu_action("Open File", cx, |player, window, cx| {
                player.open_file_picker(window, cx);
            }))
            .child(simple_menu_action(
                "Open Multiple Files",
                cx,
                |player, window, cx| {
                    player.open_queue_picker(window, cx);
                },
            ))
            .child(simple_menu_action(
                "Open Folder",
                cx,
                |player, window, cx| {
                    player.open_folder_picker(window, cx);
                },
            ))
    }


    fn render_source_provider_settings_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let provider_count = self.settings.source_providers.len();

        div()
            .id("settings-source-providers")
            .flex()
            .flex_col()
            .gap_2()
            .pt_2()
            .border_t_1()
            .border_color(rgb(FINE_BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .child(div().text_sm().flex_1().child("Source Providers"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED_TEXT))
                            .child(format!("{provider_count} saved")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .child(div().flex_1().child(self.source_provider_input.clone()))
                    .child(prompt_action_button(
                        "settings-source-provider-add",
                        "Add",
                        false,
                        1.0,
                        cx,
                        |player, window, cx| {
                            player.add_source_provider_from_input(window, cx);
                        },
                    )),
            )
            .child(
                div()
                    .id("settings-source-provider-list")
                    .max_h(px(SOURCE_PROVIDER_SETTINGS_MAX_HEIGHT))
                    .overflow_y_scroll()
                    .scrollbar_width(px(4.0))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when(self.settings.source_providers.is_empty(), |list| {
                        list.child(empty_menu_message("No source providers saved."))
                    })
                    .children(
                        self.settings
                            .source_providers
                            .iter()
                            .cloned()
                            .map(|provider| self.render_source_provider_settings_row(provider, cx)),
                    ),
            )
    }

    fn render_source_provider_settings_row(
        &self,
        provider: SourceProvider,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let provider_id = provider.id.clone();
        let row_id = library_safe_element_key(&provider.id);

        div()
            .id(format!("settings-source-provider-{row_id}"))
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded_sm()
            .hover(|row| row.bg(rgb(0x121212)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().truncate().child(provider.name))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED_TEXT))
                            .truncate()
                            .child(provider.search_url_template),
                    ),
            )
            .child(queue_row_icon_button(
                format!("settings-source-provider-remove-{row_id}"),
                ICON_X,
                "Remove source provider",
                true,
                cx,
                move |player, _window, cx| {
                    player.remove_source_provider(provider_id.clone(), cx);
                },
            ))
    }

    fn render_settings_selector_overlay(
        &self,
        selector_kind: SettingsSelectorKind,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selector_options = self.settings_selector_options(selector_kind);

        div()
            .id("settings-selector-overlay")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb_alpha(OLED_BLACK, 0.34))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|player, _event, _window, cx| {
                    player.close_settings_selector(cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .id(format!(
                        "settings-selector-{}",
                        library_safe_element_key(selector_kind.title())
                    ))
                    .w(px(360.0))
                    .max_h(px(430.0))
                    .overflow_y_scroll()
                    .scrollbar_width(px(4.0))
                    .p_2()
                    .bg(self
                        .settings
                        .surface_background_color(MENU_BLACK, BACKDROP_BLUR_MODAL_BACKGROUND_ALPHA))
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(BRIGHT_BORDER))
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .text_color(rgb(SOFT_WHITE))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_player, _event, _window, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_sm()
                            .text_color(rgb(MUTED_TEXT))
                            .child(selector_kind.title()),
                    )
                    .children(selector_options.into_iter().map(|selector_option| {
                        self.render_settings_selector_option(selector_option, cx)
                    })),
            )
    }

    fn render_settings_selector_option(
        &self,
        selector_option: SettingsSelectorOption,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selector_choice = selector_option.choice.clone();

        div()
            .id(format!(
                "settings-selector-option-{}",
                library_safe_element_key(&selector_option.label)
            ))
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .bg(if selector_option.is_selected {
                rgb_alpha(VLC_ORANGE, 0.12)
            } else {
                rgb_alpha(OLED_BLACK, 0.0)
            })
            .cursor_pointer()
            .hover(|row| row.bg(rgb(0x121212)))
            .on_click(cx.listener(move |player, _event, window, cx| {
                player.apply_settings_selector_choice(selector_choice.clone(), window, cx);
                cx.stop_propagation();
            }))
            .child(div().text_sm().flex_1().child(selector_option.label))
            .when_some(selector_option.detail, |option_row, detail| {
                option_row.child(div().text_xs().text_color(rgb(MUTED_TEXT)).child(detail))
            })
            .when(selector_option.is_selected, |option_row| {
                option_row.child(
                    div()
                        .text_xs()
                        .text_color(rgb(VLC_ORANGE))
                        .child("Selected"),
                )
            })
    }

    fn settings_selector_options(
        &self,
        selector_kind: SettingsSelectorKind,
    ) -> Vec<SettingsSelectorOption> {
        match selector_kind {
            SettingsSelectorKind::AudioOutput => self
                .audio_output_devices
                .iter()
                .map(|audio_output_device| SettingsSelectorOption {
                    label: audio_output_device.label.clone(),
                    detail: Some(audio_output_device.device_id.clone()),
                    is_selected: self.settings.audio_output_device == audio_output_device.device_id,
                    choice: SettingsSelectorChoice::AudioOutputDevice(
                        audio_output_device.device_id.clone(),
                    ),
                })
                .collect(),
            SettingsSelectorKind::ResumeBehavior => [
                ResumeBehavior::Ask,
                ResumeBehavior::Always,
                ResumeBehavior::Never,
            ]
            .into_iter()
            .map(|resume_behavior| SettingsSelectorOption {
                label: resume_behavior.label().to_string(),
                detail: None,
                is_selected: self.settings.resume_behavior == resume_behavior,
                choice: SettingsSelectorChoice::ResumeBehavior(resume_behavior),
            })
            .collect(),
            SettingsSelectorKind::SeekStep => [3.0, 5.0, 10.0, 30.0]
                .into_iter()
                .map(|seek_step_seconds| SettingsSelectorOption {
                    label: format!("{seek_step_seconds} seconds"),
                    detail: None,
                    is_selected: (self.settings.seek_step_seconds - seek_step_seconds).abs()
                        < f64::EPSILON,
                    choice: SettingsSelectorChoice::SeekStepSeconds(seek_step_seconds),
                })
                .collect(),
            SettingsSelectorKind::VolumeStep => [1, 2, 5, 10]
                .into_iter()
                .map(|volume_step_percent| SettingsSelectorOption {
                    label: format!("{volume_step_percent}%"),
                    detail: None,
                    is_selected: self.settings.volume_step_percent == volume_step_percent,
                    choice: SettingsSelectorChoice::VolumeStepPercent(volume_step_percent),
                })
                .collect(),
            SettingsSelectorKind::PreferredAudioLanguage => self
                .language_settings_selector_options(
                    &self.settings.preferred_audio_language,
                    SettingsSelectorChoice::PreferredAudioLanguage,
                ),
            SettingsSelectorKind::PreferredSubtitleLanguage => self
                .language_settings_selector_options(
                    &self.settings.preferred_subtitle_language,
                    SettingsSelectorChoice::PreferredSubtitleLanguage,
                ),
            SettingsSelectorKind::HardwareDecoding => [
                ("Auto Safe", "auto-safe"),
                ("D3D11VA", "d3d11va"),
                ("Off", "no"),
            ]
            .into_iter()
            .map(|(label, hardware_decoding_mode)| SettingsSelectorOption {
                label: label.to_string(),
                detail: Some(hardware_decoding_mode.to_string()),
                is_selected: self.settings.hardware_decoding_mode == hardware_decoding_mode,
                choice: SettingsSelectorChoice::HardwareDecodingMode(
                    hardware_decoding_mode.to_string(),
                ),
            })
            .collect(),
            SettingsSelectorKind::StartFullscreen => self.boolean_settings_selector_options(
                self.settings.start_fullscreen,
                SettingsSelectorChoice::StartFullscreen,
            ),
            SettingsSelectorKind::LiveLowestLatency => self.boolean_settings_selector_options(
                self.settings.is_lowest_latency_live_capture_enabled,
                SettingsSelectorChoice::LiveLowestLatency,
            ),
            SettingsSelectorKind::LiveCaptureExclusiveAudio => self
                .boolean_settings_selector_options(
                    self.settings.is_live_capture_exclusive_audio_enabled,
                    SettingsSelectorChoice::LiveCaptureExclusiveAudio,
                ),
            SettingsSelectorKind::BackdropBlur => self.boolean_settings_selector_options(
                self.settings.is_backdrop_blur_enabled,
                SettingsSelectorChoice::BackdropBlur,
            ),
        }
    }

    fn language_settings_selector_options(
        &self,
        selected_language: &str,
        build_selector_choice: fn(String) -> SettingsSelectorChoice,
    ) -> Vec<SettingsSelectorOption> {
        [
            ("English", "eng"),
            ("Japanese", "jpn"),
            ("Spanish", "spa"),
            ("French", "fra"),
            ("Any", "any"),
        ]
        .into_iter()
        .map(|(label, language_code)| SettingsSelectorOption {
            label: label.to_string(),
            detail: Some(language_code.to_string()),
            is_selected: selected_language == language_code,
            choice: build_selector_choice(language_code.to_string()),
        })
        .collect()
    }

    fn boolean_settings_selector_options(
        &self,
        is_enabled: bool,
        build_selector_choice: fn(bool) -> SettingsSelectorChoice,
    ) -> Vec<SettingsSelectorOption> {
        [false, true]
            .into_iter()
            .map(|is_option_enabled| SettingsSelectorOption {
                label: on_off_label(is_option_enabled).to_string(),
                detail: None,
                is_selected: is_enabled == is_option_enabled,
                choice: build_selector_choice(is_option_enabled),
            })
            .collect()
    }


    fn render_settings_action_row(
        &self,
        label: &'static str,
        detail: String,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut WatchPlayer, &mut Window, &mut Context<WatchPlayer>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(stable_ui_id("settings-row", label))
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .cursor_pointer()
            .hover(|row| row.bg(rgb(0x121212)))
            .on_click(cx.listener(move |player, _, window, cx| {
                on_click(player, window, cx);
            }))
            .child(div().text_sm().flex_1().child(label))
            .child(div().text_xs().text_color(rgb(MUTED_TEXT)).child(detail))
    }

    fn render_queue_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("queue-list")
            .flex()
            .flex_col()
            .gap_0()
            .pl_2()
            .max_h(px(QUEUE_LIST_MAX_HEIGHT))
            .overflow_y_scroll()
            .scrollbar_width(px(4.0))
            .when(self.playback_queue.is_empty(), |menu| {
                menu.child(empty_menu_message("No queued media."))
            })
            .children(
                self.playback_queue
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(queue_index, media)| self.render_queue_item_row(queue_index, media, cx)),
            )
    }

    fn render_queue_item_row(
        &self,
        queue_index: usize,
        media: LoadedMedia,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_current = self.current_queue_index == Some(queue_index);
        let can_move_up = queue_index > 0;
        let can_move_down = queue_index + 1 < self.playback_queue.len();
        let media_title = media.queue_title();
        let media_detail = media.display_detail();

        div()
            .id(format!("queue-item-{queue_index}"))
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .bg(if is_current {
                rgb_alpha(VLC_ORANGE, 0.10)
            } else {
                rgb_alpha(OLED_BLACK, 0.0)
            })
            .hover(|row| row.bg(rgb(0x121212)))
            .child(
                div()
                    .id(format!("queue-item-{queue_index}-details"))
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .cursor_pointer()
                    .on_click(cx.listener(move |player, _, window, cx| {
                        player.play_queue_item(queue_index, window, cx);
                    }))
                    .child(div().text_sm().truncate().child(media_title))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED_TEXT))
                            .truncate()
                            .child(media_detail),
                    ),
            )
            .child(queue_row_icon_button(
                format!("queue-item-{queue_index}-up"),
                ICON_CHEVRON_UP,
                "Move earlier",
                can_move_up,
                cx,
                move |player, window, cx| {
                    player.move_queue_item_up(queue_index, window, cx);
                },
            ))
            .child(queue_row_icon_button(
                format!("queue-item-{queue_index}-down"),
                ICON_CHEVRON_DOWN,
                "Move later",
                can_move_down,
                cx,
                move |player, window, cx| {
                    player.move_queue_item_down(queue_index, window, cx);
                },
            ))
            .child(queue_row_icon_button(
                format!("queue-item-{queue_index}-play"),
                ICON_PLAY,
                "Play",
                true,
                cx,
                move |player, window, cx| {
                    player.play_queue_item(queue_index, window, cx);
                },
            ))
            .child(queue_row_icon_button(
                format!("queue-item-{queue_index}-play-next"),
                ICON_NEXT,
                "Play next",
                true,
                cx,
                move |player, window, cx| {
                    player.play_queue_item_next(queue_index, window, cx);
                },
            ))
    }

    fn render_audio_track_row(
        &self,
        audio_track: AudioTrack,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = self
            .selected_audio_track_id
            .is_some_and(|selected_track_id| selected_track_id == audio_track.track_id);
        let track_id = audio_track.track_id;
        let detail = audio_track_detail(&audio_track);

        div()
            .id(format!("audio-track-{track_id}"))
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .bg(if is_selected {
                rgb_alpha(VLC_ORANGE, 0.10)
            } else {
                rgb_alpha(OLED_BLACK, 0.0)
            })
            .cursor_pointer()
            .hover(|row| row.bg(rgb(0x121212)))
            .on_click(cx.listener(move |player, _, window, cx| {
                player.select_audio_track(track_id, window, cx);
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().truncate().child(audio_track.title))
                    .child(div().text_xs().text_color(rgb(MUTED_TEXT)).child(detail)),
            )
            .child(if is_selected { "selected" } else { "" })
    }

    fn render_embedded_subtitle_track_row(
        &self,
        subtitle_track: EmbeddedSubtitleTrack,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = self
            .selected_embedded_subtitle_track_id
            .is_some_and(|selected_track_id| selected_track_id == subtitle_track.track_id)
            || (self.selected_subtitle_path.is_none()
                && self.selected_embedded_subtitle_track_id.is_none()
                && subtitle_track.is_selected);
        let track_id = subtitle_track.track_id;
        let detail = embedded_subtitle_detail(&subtitle_track);

        div()
            .id(format!("embedded-subtitle-{track_id}"))
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .bg(if is_selected {
                rgb_alpha(VLC_ORANGE, 0.10)
            } else {
                rgb_alpha(OLED_BLACK, 0.0)
            })
            .cursor_pointer()
            .hover(|row| row.bg(rgb(0x121212)))
            .on_click(cx.listener(move |player, _, window, cx| {
                player.select_embedded_subtitle_track(track_id, window, cx);
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .child(div().text_sm().child(subtitle_track.title))
                    .child(div().text_xs().text_color(rgb(MUTED_TEXT)).child(detail)),
            )
            .child(if is_selected { "selected" } else { "" })
    }

    fn render_subtitle_path_row(
        &self,
        subtitle_path: PathBuf,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = self
            .selected_subtitle_path
            .as_ref()
            .is_some_and(|selected_path| selected_path == &subtitle_path);
        let subtitle_path_for_click = subtitle_path.clone();

        div()
            .id(stable_path_ui_id("subtitle", &subtitle_path))
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .bg(if is_selected {
                rgb_alpha(VLC_ORANGE, 0.10)
            } else {
                rgb_alpha(OLED_BLACK, 0.0)
            })
            .cursor_pointer()
            .hover(|row| row.bg(rgb(0x121212)))
            .on_click(cx.listener(move |player, _, window, cx| {
                player.selected_subtitle_path = Some(subtitle_path_for_click.clone());
                player.selected_embedded_subtitle_track_id = None;
                player.is_subtitle_menu_open = false;
                player.load_selected_subtitle_in_backend();
                player.show_osd(
                    format!("Subtitles: {}", player.selected_subtitle_label()),
                    window,
                    cx,
                );
                player.reveal_controls(window, cx);
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .child(div().text_sm().child(display_name(&subtitle_path)))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED_TEXT))
                            .child(subtitle_path.display().to_string()),
                    ),
            )
            .child(if is_selected { "selected" } else { "" })
    }
}

impl Focusable for WatchPlayer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TooltipText {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .bg(rgb(0x151515))
            .text_color(rgb(SOFT_WHITE))
            .text_xs()
            .child(self.text.clone())
    }
}

const SOURCE_TEXT_INPUT_KEY_CONTEXT: &str = "SourceTextInput";

struct InlineTextInput {
    focus_handle: FocusHandle,
    content: String,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

impl InlineTextInput {
    fn new(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: String::new(),
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    fn text(&self) -> String {
        self.content.clone()
    }

    fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.content = text.to_string();
        self.selected_range = self.content.len()..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
        cx.notify();
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        self.content.clear();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let previous_boundary = self.previous_boundary(self.cursor_offset());
            if previous_boundary == self.cursor_offset() {
                window.play_system_bell();
                return;
            }
            self.select_to(previous_boundary, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next_boundary = self.next_boundary(self.cursor_offset());
            if next_boundary == self.cursor_offset() {
                window.play_system_bell();
                return;
            }
            self.select_to(next_boundary, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let single_line_text = text.replace('\r', " ").replace('\n', " ");
            self.replace_text_in_range(None, &single_line_text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = self.clamp_to_char_boundary(offset);
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        cx.notify();
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = self.clamp_to_char_boundary(offset);
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return self.content.len();
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        self.clamp_to_char_boundary(line.closest_index_for_x(position.x - bounds.left()))
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        let clamped_offset = offset.min(self.content.len());
        self.content[..clamped_offset]
            .char_indices()
            .last()
            .map(|(index, _)| index)
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        let clamped_offset = offset.min(self.content.len());
        self.content[clamped_offset..]
            .char_indices()
            .nth(1)
            .map(|(index, _)| clamped_offset + index)
            .unwrap_or(self.content.len())
    }

    fn clamp_to_char_boundary(&self, offset: usize) -> usize {
        let mut clamped_offset = offset.min(self.content.len());
        while clamped_offset > 0 && !self.content.is_char_boundary(clamped_offset) {
            clamped_offset -= 1;
        }
        clamped_offset
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for character in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += character.len_utf16();
            utf8_offset += character.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        self.content[..self.clamp_to_char_boundary(offset)]
            .chars()
            .map(char::len_utf16)
            .sum()
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }
}

impl EntityInputHandler for InlineTextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let single_line_text = new_text.replace('\r', " ").replace('\n', " ");

        self.content = format!(
            "{}{}{}",
            &self.content[..range.start],
            single_line_text,
            &self.content[range.end..]
        );
        let cursor_offset = range.start + single_line_text.len();
        self.selected_range = cursor_offset..cursor_offset;
        self.selection_reversed = false;
        self.marked_range = None;
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let single_line_text = new_text.replace('\r', " ").replace('\n', " ");

        self.content = format!(
            "{}{}{}",
            &self.content[..range.start],
            single_line_text,
            &self.content[range.end..]
        );
        self.marked_range = (!single_line_text.is_empty())
            .then_some(range.start..range.start + single_line_text.len());
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .map(|selected_range| {
                range.start + selected_range.start..range.start + selected_range.end
            })
            .unwrap_or_else(|| {
                range.start + single_line_text.len()..range.start + single_line_text.len()
            });
        self.selection_reversed = false;
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);

        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        let utf8_index = last_layout.index_for_x(point.x - line_point.x)?;

        Some(self.offset_to_utf16(utf8_index))
    }
}

struct InlineTextElement {
    input: Entity<InlineTextInput>,
}

struct InlineTextPrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for InlineTextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for InlineTextElement {
    type RequestLayoutState = ();
    type PrepaintState = InlineTextPrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = window.line_height().into();

        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor_offset = input.cursor_offset();
        let style = window.text_style();
        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), rgb(MUTED_TEXT).into())
        } else {
            (SharedString::from(content), style.color)
        };
        let base_run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = input.marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..base_run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(base_run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..base_run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..base_run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect::<Vec<_>>()
        } else {
            vec![base_run]
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);
        let cursor_position = line.x_for_index(cursor_offset);
        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_position, bounds.top()),
                        size(px(1.5), bounds.bottom() - bounds.top()),
                    ),
                    rgb(SOFT_WHITE),
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end),
                            bounds.bottom(),
                        ),
                    ),
                    rgba(0xffffff28),
                )),
                None,
            )
        };

        InlineTextPrepaintState {
            line: Some(line),
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }
        let line = prepaint.line.take().unwrap();
        line.paint(
            bounds.origin,
            window.line_height(),
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        )
        .unwrap();

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for InlineTextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(SOURCE_TEXT_INPUT_KEY_CONTEXT)
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full()
            .h(px(34.0))
            .px_3()
            .py_1()
            .bg(rgb(0x101010))
            .border_1()
            .border_color(rgb(FINE_BORDER))
            .rounded_sm()
            .text_size(px(14.0))
            .line_height(px(20.0))
            .text_color(rgb(SOFT_WHITE))
            .child(InlineTextElement { input: cx.entity() })
    }
}

impl Focusable for InlineTextInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for WatchPlayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.save_current_window_bounds_if_due(window);
        let is_window_fullscreen = window.is_fullscreen();
        let viewport_size = window.viewport_size();
        let viewport_width = viewport_size.width.as_f32();
        let viewport_height = viewport_size.height.as_f32();
        let is_video_surface_active = self.is_video_surface_active();

        let should_show_blurred_library_backdrop =
            self.settings.is_backdrop_blur_enabled && self.is_library_surface_visible();
        let active_key_context = if self.is_source_search_open || self.is_settings_modal_open {
            WATCH_DIALOG_KEY_CONTEXT
        } else {
            WATCH_PLAYER_KEY_CONTEXT
        };

        div()
            .id("watch-player")
            .key_context(active_key_context)
            .on_action(cx.listener(Self::toggle_playback))
            .on_action(cx.listener(Self::toggle_mute_action))
            .on_action(cx.listener(Self::toggle_fullscreen))
            .on_action(cx.listener(Self::previous_queue_item))
            .on_action(cx.listener(Self::next_queue_item))
            .on_action(cx.listener(Self::frame_step_backward))
            .on_action(cx.listener(Self::frame_step_forward))
            .on_action(cx.listener(Self::set_ab_loop_a))
            .on_action(cx.listener(Self::set_ab_loop_b))
            .on_action(cx.listener(Self::clear_ab_loop))
            .on_action(cx.listener(Self::seek_to_previous_chapter))
            .on_action(cx.listener(Self::seek_to_next_chapter))
            .on_action(cx.listener(Self::jump_to_percent_0))
            .on_action(cx.listener(Self::jump_to_percent_1))
            .on_action(cx.listener(Self::jump_to_percent_2))
            .on_action(cx.listener(Self::jump_to_percent_3))
            .on_action(cx.listener(Self::jump_to_percent_4))
            .on_action(cx.listener(Self::jump_to_percent_5))
            .on_action(cx.listener(Self::jump_to_percent_6))
            .on_action(cx.listener(Self::jump_to_percent_7))
            .on_action(cx.listener(Self::jump_to_percent_8))
            .on_action(cx.listener(Self::jump_to_percent_9))
            .on_action(cx.listener(Self::increase_subtitle_delay))
            .on_action(cx.listener(Self::decrease_subtitle_delay))
            .on_action(cx.listener(Self::reset_subtitle_delay))
            .on_action(cx.listener(Self::seek_backward))
            .on_action(cx.listener(Self::seek_forward))
            .on_action(cx.listener(Self::increase_volume))
            .on_action(cx.listener(Self::decrease_volume))
            .on_action(cx.listener(Self::increase_playback_speed))
            .on_action(cx.listener(Self::decrease_playback_speed))
            .on_action(cx.listener(Self::toggle_shuffle))
            .on_action(cx.listener(Self::cycle_repeat_mode))
            .on_action(cx.listener(Self::submit_source_search_or_selection))
            .on_action(cx.listener(Self::select_previous_source_result))
            .on_action(cx.listener(Self::select_next_source_result))
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(
                if is_video_surface_active || should_show_blurred_library_backdrop {
                    rgb_alpha(OLED_BLACK, 0.0)
                } else {
                    self.settings
                        .surface_background_color(OLED_BLACK, BACKDROP_BLUR_BASE_BACKGROUND_ALPHA)
                },
            )
            .text_color(rgb(SOFT_WHITE))
            .p_0()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_move(
                cx.listener(|player, _event: &gpui::MouseMoveEvent, window, cx| {
                    player.reveal_controls(window, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|player, event: &MouseDownEvent, window, cx| {
                    if event.click_count >= 2
                        && player.should_toggle_fullscreen_from_video_double_click(
                            event.position,
                            window,
                        )
                    {
                        player.toggle_window_fullscreen(window, cx);
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|player, event: &gpui::MouseDownEvent, window, cx| {
                    if player.is_library_open || player.is_settings_modal_open {
                        return;
                    }
                    player.show_subtitle_context_menu(event.position, window, cx);
                }),
            )
            .on_scroll_wheel(cx.listener(|player, event: &ScrollWheelEvent, window, cx| {
                if player.is_library_open
                    || player.is_main_menu_open
                    || player.is_subtitle_menu_open
                    || player.is_source_search_open
                    || player.open_context_menu_section.is_some()
                {
                    return;
                }

                let scroll_delta = match event.delta {
                    ScrollDelta::Pixels(delta) => delta.y.as_f32(),
                    ScrollDelta::Lines(delta) => delta.y,
                };
                if scroll_delta < 0.0 {
                    player.change_volume_by(player.settings.volume_step_percent, window, cx);
                } else if scroll_delta > 0.0 {
                    player.change_volume_by(-player.settings.volume_step_percent, window, cx);
                }
            }))
            .on_drop(
                cx.listener(|player, external_paths: &ExternalPaths, window, cx| {
                    let subtitle_paths = external_paths
                        .paths()
                        .iter()
                        .filter(|path| path.is_file() && is_subtitle_path(path))
                        .cloned()
                        .collect::<Vec<_>>();
                    if let Some(subtitle_path) = subtitle_paths.first().cloned() {
                        player.selected_subtitle_path = Some(subtitle_path.clone());
                        player.selected_embedded_subtitle_track_id = None;
                        if let Some(current_queue_index) = player.current_queue_index {
                            if let Some(current_media) =
                                player.playback_queue.get_mut(current_queue_index)
                            {
                                if !current_media.subtitle_paths.contains(&subtitle_path) {
                                    current_media.subtitle_paths.push(subtitle_path);
                                }
                            }
                        }
                        player.load_selected_subtitle_in_backend();
                        player.show_osd("Subtitle loaded".to_string(), window, cx);
                        return;
                    }

                    let media_paths = external_paths
                        .paths()
                        .iter()
                        .cloned()
                        .flat_map(media_paths_from_path)
                        .collect::<Vec<_>>();
                    player.load_media_paths(media_paths, window, cx);
                }),
            )
            .drag_over::<ExternalPaths>(|style, _, _, _| style.border_color(rgb(VLC_ORANGE)))
            .child(self.render_video_surface(
                is_window_fullscreen,
                viewport_width,
                viewport_height,
                cx,
            ))
    }
}

impl Drop for WatchPlayer {
    fn drop(&mut self) {
        self.record_current_media_progress();
        self.flush_player_library_if_due(true);
        self.save_current_watch_session();
        self.stop_playback_process();
    }
}

#[allow(dead_code)]
fn render_continue_remove_icon(
    media_path: &Path,
    is_hovered: bool,
    is_exiting: bool,
    animation_generation: u64,
    library_scale: f32,
) -> AnyElement {
    let icon = svg()
        .external_path(crate::icon_path(ICON_X))
        .w(px(16.0 * library_scale))
        .h(px(16.0 * library_scale))
        .text_color(rgb(SOFT_WHITE));

    if !is_hovered && !is_exiting {
        return icon.into_any_element();
    }

    let animation_direction = if is_hovered { "grow" } else { "shrink" };
    let animation_id = format!(
        "continue-remove-scale-{:016x}-{animation_direction}-{animation_generation}",
        stable_hash_bytes(library_media_path_key(media_path).as_bytes())
    );

    icon.with_animation(
        animation_id,
        Animation::new(Duration::from_millis(CONTINUE_REMOVE_SCALE_ANIMATION_MS))
            .with_easing(ease_in_out),
        move |icon, delta| {
            let scale_delta = if is_hovered { delta } else { 1.0 - delta };
            let icon_scale = 1.0 + ((CONTINUE_REMOVE_HOVER_SCALE - 1.0) * scale_delta);

            icon.with_transformation(Transformation::scale(size(icon_scale, icon_scale)))
        },
    )
    .into_any_element()
}

fn render_scaled_library_icon(
    icon_file_name: &'static str,
    icon_key: &str,
    icon_width_px: f32,
    icon_height_px: f32,
    is_hovered: bool,
    is_exiting: bool,
    animation_generation: u64,
) -> AnyElement {
    let icon = svg()
        .external_path(crate::icon_path(icon_file_name))
        .w(px(icon_width_px))
        .h(px(icon_height_px))
        .text_color(rgb(SOFT_WHITE));

    if !is_hovered && !is_exiting {
        return icon.into_any_element();
    }

    let animation_direction = if is_hovered { "grow" } else { "shrink" };
    let animation_id = format!(
        "library-icon-scale-{}-{animation_direction}-{animation_generation}",
        library_safe_element_key(icon_key)
    );

    icon.with_animation(
        animation_id,
        Animation::new(Duration::from_millis(LIBRARY_ICON_SCALE_ANIMATION_MS))
            .with_easing(ease_in_out),
        move |icon, delta| {
            let scale_delta = if is_hovered { delta } else { 1.0 - delta };
            let icon_scale = 1.0 + ((LIBRARY_ICON_HOVER_SCALE - 1.0) * scale_delta);

            icon.with_transformation(Transformation::scale(size(icon_scale, icon_scale)))
        },
    )
    .into_any_element()
}

fn render_autoscrolling_library_title(
    title: String,
    card_width: f32,
    section_title: &str,
    media_path: &Path,
    library_scale: f32,
) -> AnyElement {
    let title_line_height = LIBRARY_TITLE_LINE_HEIGHT_PX * library_scale;
    let scroll_distance = library_title_scroll_distance(&title, card_width, library_scale);

    if scroll_distance <= 0.0 {
        return div()
            .min_w_0()
            .h(px(title_line_height))
            .text_size(px(14.0 * library_scale))
            .line_height(px(title_line_height))
            .text_color(rgb(SOFT_WHITE))
            .truncate()
            .child(title)
            .into_any_element();
    }

    let animation_id = format!(
        "library-title-scroll-{}-{:016x}",
        library_safe_element_key(section_title),
        stable_hash_bytes(library_media_path_key(media_path).as_bytes())
    );
    let animation_duration = library_title_scroll_duration(scroll_distance);

    div()
        .relative()
        .w_full()
        .h(px(title_line_height))
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .text_size(px(14.0 * library_scale))
                .line_height(px(title_line_height))
                .text_color(rgb(SOFT_WHITE))
                .whitespace_nowrap()
                .child(title)
                .with_animation(
                    animation_id,
                    Animation::new(animation_duration).repeat(),
                    move |title_element, delta| {
                        title_element.left(px(-library_title_scroll_offset(scroll_distance, delta)))
                    },
                ),
        )
        .into_any_element()
}

fn library_title_scroll_distance(title: &str, card_width: f32, library_scale: f32) -> f32 {
    let estimated_title_width =
        title.chars().count() as f32 * LIBRARY_TITLE_AVERAGE_CHARACTER_WIDTH_PX * library_scale;
    let available_title_width = card_width.max(1.0);

    (estimated_title_width - available_title_width
        + (LIBRARY_TITLE_SCROLL_END_PADDING_PX * library_scale))
        .max(0.0)
}

fn library_title_scroll_duration(scroll_distance: f32) -> Duration {
    let seconds = (scroll_distance / 22.0 + 5.0).clamp(6.0, 14.0).round() as u64;

    Duration::from_secs(seconds)
}

fn library_title_scroll_offset(scroll_distance: f32, animation_delta: f32) -> f32 {
    let start_pause_fraction = 0.16;
    let end_pause_fraction = 0.84;

    if animation_delta <= start_pause_fraction {
        return 0.0;
    }

    if animation_delta >= end_pause_fraction {
        return scroll_distance;
    }

    let travel_fraction =
        (animation_delta - start_pause_fraction) / (end_pause_fraction - start_pause_fraction);
    scroll_distance * travel_fraction
}

fn library_safe_element_key(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

fn empty_menu_message(message: &'static str) -> Div {
    div()
        .px_2()
        .py_1()
        .text_color(rgb(MUTED_TEXT))
        .text_xs()
        .child(message)
}

fn scaled_menu_message(message: &'static str, menu_scale: f32) -> Div {
    div()
        .px(px(8.0 * menu_scale))
        .py(px(4.0 * menu_scale))
        .text_color(rgb(MUTED_TEXT))
        .text_size(px(12.0 * menu_scale))
        .line_height(px(16.0 * menu_scale))
        .child(message)
}

fn simple_menu_action_with_icon(
    label: &'static str,
    icon_path: &'static str,
    cx: &mut Context<WatchPlayer>,
    on_click: impl Fn(&mut WatchPlayer, &mut Window, &mut Context<WatchPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(format!("simple-menu-action-{label}"))
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .text_sm()
        .rounded_sm()
        .cursor_pointer()
        .hover(|menu_item| menu_item.bg(rgb(0x121212)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |player, _event, window, cx| {
                on_click(player, window, cx);
                cx.stop_propagation();
            }),
        )
        .child(
            svg()
                .external_path(crate::icon_path(icon_path))
                .w(px(14.0))
                .h(px(14.0))
                .text_color(rgb(MUTED_TEXT)),
        )
        .child(label)
}

fn simple_menu_action(
    label: &'static str,
    cx: &mut Context<WatchPlayer>,
    on_click: impl Fn(&mut WatchPlayer, &mut Window, &mut Context<WatchPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(format!("simple-menu-action-{label}"))
        .px_2()
        .py_1()
        .text_sm()
        .rounded_sm()
        .cursor_pointer()
        .hover(|menu_item| menu_item.bg(rgb(0x121212)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |player, _event, window, cx| {
                on_click(player, window, cx);
                cx.stop_propagation();
            }),
        )
        .child(label)
}

fn context_section_button(
    label: &'static str,
    detail: String,
    is_open: bool,
    cx: &mut Context<WatchPlayer>,
    on_click: impl Fn(&mut WatchPlayer, &mut Window, &mut Context<WatchPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(format!("context-section-{label}"))
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .bg(if is_open {
            rgb_alpha(VLC_ORANGE, 0.08)
        } else {
            rgb_alpha(OLED_BLACK, 0.0)
        })
        .rounded_sm()
        .cursor_pointer()
        .hover(|row| row.bg(rgb(0x121212)))
        .on_click(cx.listener(move |player, _, window, cx| {
            on_click(player, window, cx);
        }))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(div().text_sm().child(label))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(MUTED_TEXT))
                        .truncate()
                        .child(detail),
                ),
        )
        .child(
            svg()
                .external_path(crate::icon_path(if is_open {
                    ICON_CHEVRON_DOWN
                } else {
                    ICON_CHEVRON_RIGHT
                }))
                .w(px(12.0))
                .h(px(12.0))
                .text_color(rgb(MUTED_TEXT)),
        )
}

fn playback_mode_icon_button(
    id: String,
    icon_path: &'static str,
    tooltip: String,
    is_active: bool,
    cx: &mut Context<WatchPlayer>,
    on_click: impl Fn(&mut WatchPlayer, &mut Window, &mut Context<WatchPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(34.0))
        .h(px(30.0))
        .bg(if is_active {
            rgb_alpha(VLC_ORANGE, 0.16)
        } else {
            rgb_alpha(OLED_BLACK, 0.0)
        })
        .border_1()
        .border_color(if is_active {
            rgb_alpha(VLC_ORANGE, 0.42)
        } else {
            rgb_alpha(OLED_BLACK, 0.0)
        })
        .rounded_sm()
        .text_color(if is_active {
            rgb(VLC_ORANGE)
        } else {
            rgb(SOFT_WHITE)
        })
        .cursor_pointer()
        .hover(|button| button.bg(rgb(0x151515)))
        .active(|button| button.opacity(0.76))
        .on_click(cx.listener(move |player, _, window, cx| {
            on_click(player, window, cx);
            cx.stop_propagation();
        }))
        .child(
            svg()
                .external_path(crate::icon_path(icon_path))
                .w(px(17.0))
                .h(px(17.0))
                .text_color(if is_active {
                    rgb(VLC_ORANGE)
                } else {
                    rgb(SOFT_WHITE)
                }),
        )
        .tooltip(move |_window, cx| {
            cx.new(|_| TooltipText {
                text: tooltip.clone().into(),
            })
            .into()
        })
}

fn context_icon_button(
    id: &'static str,
    icon_path: &'static str,
    tooltip: &'static str,
    cx: &mut Context<WatchPlayer>,
    on_click: impl Fn(&mut WatchPlayer, &mut Window, &mut Context<WatchPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(34.0))
        .h(px(30.0))
        .text_color(rgb(SOFT_WHITE))
        .rounded_sm()
        .cursor_pointer()
        .hover(|button| button.bg(rgb(0x151515)))
        .active(|button| button.opacity(0.76))
        .on_click(cx.listener(move |player, _, window, cx| {
            on_click(player, window, cx);
        }))
        .child(
            svg()
                .external_path(crate::icon_path(icon_path))
                .w(px(17.0))
                .h(px(17.0))
                .text_color(rgb(SOFT_WHITE)),
        )
        .tooltip(move |_window, cx| {
            cx.new(|_| TooltipText {
                text: tooltip.into(),
            })
            .into()
        })
}

fn queue_row_icon_button(
    id: String,
    icon_path: &'static str,
    tooltip: &'static str,
    is_enabled: bool,
    cx: &mut Context<WatchPlayer>,
    on_click: impl Fn(&mut WatchPlayer, &mut Window, &mut Context<WatchPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(24.0))
        .h(px(24.0))
        .text_color(rgb(SOFT_WHITE))
        .rounded_sm()
        .opacity(if is_enabled { 1.0 } else { 0.32 })
        .cursor_pointer()
        .hover(|button| button.bg(rgb(0x151515)))
        .active(|button| button.opacity(0.76))
        .on_click(cx.listener(move |player, _, window, cx| {
            if is_enabled {
                on_click(player, window, cx);
            } else {
                player.reveal_controls(window, cx);
            }
        }))
        .child(
            svg()
                .external_path(crate::icon_path(icon_path))
                .w(px(15.0))
                .h(px(15.0))
                .text_color(rgb(SOFT_WHITE)),
        )
        .tooltip(move |_window, cx| {
            cx.new(|_| TooltipText {
                text: tooltip.into(),
            })
            .into()
        })
}

fn prompt_action_button(
    id: &'static str,
    label: &'static str,
    is_primary: bool,
    button_scale: f32,
    cx: &mut Context<WatchPlayer>,
    on_click: impl Fn(&mut WatchPlayer, &mut Window, &mut Context<WatchPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(px(16.0 * button_scale))
        .py(px(8.0 * button_scale))
        .text_size(px(14.0 * button_scale))
        .line_height(px(20.0 * button_scale))
        .text_color(if is_primary {
            rgb(OLED_BLACK)
        } else {
            rgb(SOFT_WHITE)
        })
        .bg(if is_primary {
            rgb(SOFT_WHITE)
        } else {
            rgb(0x151515)
        })
        .border_1()
        .border_color(if is_primary {
            rgb(SOFT_WHITE)
        } else {
            rgb(FINE_BORDER)
        })
        .rounded_sm()
        .cursor_pointer()
        .hover(|button| button.opacity(0.82))
        .active(|button| button.opacity(0.72))
        .on_click(cx.listener(move |player, _, window, cx| {
            on_click(player, window, cx);
        }))
        .child(label)
}

fn source_back_button(
    id: &'static str,
    label: &'static str,
    cx: &mut Context<WatchPlayer>,
    on_click: impl Fn(&mut WatchPlayer, &mut Window, &mut Context<WatchPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(px(12.0))
        .py(px(7.0))
        .text_size(px(13.0))
        .line_height(px(18.0))
        .text_color(rgb(SOFT_WHITE))
        .bg(rgb(0x151515))
        .border_1()
        .border_color(rgb(FINE_BORDER))
        .rounded_sm()
        .cursor_pointer()
        .flex()
        .items_center()
        .gap_1()
        .hover(|button| button.opacity(0.82))
        .active(|button| button.opacity(0.72))
        .on_click(cx.listener(move |player, _, window, cx| {
            on_click(player, window, cx);
        }))
        .child(
            svg()
                .external_path(crate::icon_path(ICON_CHEVRON_LEFT))
                .w(px(14.0))
                .h(px(14.0))
                .text_color(rgb(SOFT_WHITE)),
        )
        .child(label)
}

fn prompt_icon_action_button(
    id: &'static str,
    icon_path: &'static str,
    tooltip: &'static str,
    button_scale: f32,
    cx: &mut Context<WatchPlayer>,
    on_click: impl Fn(&mut WatchPlayer, &mut Window, &mut Context<WatchPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .w(px(38.0 * button_scale))
        .h(px(38.0 * button_scale))
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgb(SOFT_WHITE))
        .bg(rgb(0x151515))
        .border_1()
        .border_color(rgb(FINE_BORDER))
        .rounded_sm()
        .cursor_pointer()
        .hover(|button| button.opacity(0.82))
        .active(|button| button.opacity(0.72))
        .on_click(cx.listener(move |player, _, window, cx| {
            on_click(player, window, cx);
        }))
        .child(
            svg()
                .external_path(crate::icon_path(icon_path))
                .w(px(18.0 * button_scale))
                .h(px(18.0 * button_scale))
                .text_color(rgb(SOFT_WHITE)),
        )
        .tooltip(move |_window, cx| {
            cx.new(|_| TooltipText {
                text: tooltip.into(),
            })
            .into()
        })
}

fn square_button(
    id: &'static str,
    label: &'static str,
    tooltip: &'static str,
    cx: &mut Context<WatchPlayer>,
    on_click: impl Fn(&mut WatchPlayer, &mut Window, &mut Context<WatchPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(42.0))
        .h(px(42.0))
        .text_color(rgb(SOFT_WHITE))
        .text_sm()
        .rounded_sm()
        .cursor_pointer()
        .hover(|button| button.opacity(0.72))
        .active(|button| button.opacity(0.76))
        .on_click(cx.listener(move |player, _, window, cx| {
            on_click(player, window, cx);
        }))
        .child(label)
        .tooltip(move |_window, cx| {
            cx.new(|_| TooltipText {
                text: tooltip.into(),
            })
            .into()
        })
}

fn icon_button(
    id: &'static str,
    icon_path: &'static str,
    tooltip: &'static str,
    cx: &mut Context<WatchPlayer>,
    on_click: impl Fn(&mut WatchPlayer, &mut Window, &mut Context<WatchPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(42.0))
        .h(px(42.0))
        .text_color(rgb(SOFT_WHITE))
        .cursor_pointer()
        .hover(|button| button.opacity(0.72))
        .active(|button| button.opacity(0.76))
        .on_click(cx.listener(move |player, _, window, cx| {
            on_click(player, window, cx);
        }))
        .child(
            svg()
                .external_path(crate::icon_path(icon_path))
                .w(px(19.0))
                .h(px(19.0))
                .text_color(rgb(SOFT_WHITE)),
        )
        .tooltip(move |_window, cx| {
            cx.new(|_| TooltipText {
                text: tooltip.into(),
            })
            .into()
        })
}

#[cfg(target_os = "windows")]
impl EmbeddedVideoHost {
    fn window_id(&self) -> Option<isize> {
        Some(self.window_id)
    }

    fn window_ids(&self) -> Option<(isize, isize)> {
        Some((self.window_id, self.parent_window_id))
    }
}

#[cfg(not(target_os = "windows"))]
impl EmbeddedVideoHost {
    fn window_id(&self) -> Option<isize> {
        None
    }

    fn window_ids(&self) -> Option<(isize, isize)> {
        None
    }
}

#[cfg(target_os = "windows")]
impl Drop for EmbeddedVideoHost {
    fn drop(&mut self) {
        unsafe {
            if self.window_id != 0 {
                DestroyWindow(self.window_id as HWND);
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn native_window_id(window: &Window) -> Option<isize> {
    let window_handle = HasWindowHandle::window_handle(window).ok()?;

    match window_handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
fn native_window_id(_window: &Window) -> Option<isize> {
    None
}

fn create_playback_ipc_path() -> String {
    let timestamp_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    create_platform_playback_ipc_path(timestamp_millis)
}

fn create_playback_log_path() -> PathBuf {
    let timestamp_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let log_directory = watch_session_directory().join("mpv-logs");
    let _ = fs::create_dir_all(&log_directory);

    log_directory.join(format!(
        "watch-mpv-{}-{timestamp_millis}.log",
        std::process::id()
    ))
}

fn concise_mpv_log_message(log_path: &Path) -> Option<String> {
    let log_contents = fs::read_to_string(log_path).ok()?;
    let cleaned_log_lines = log_contents
        .lines()
        .rev()
        .map(|line| clean_mpv_log_line(line.trim()))
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let priority_error_fragments = [
        "could not run graph",
        "avformat_open_input() failed",
        "could not find audio only device",
        "could not find",
    ];
    let priority_error_line = priority_error_fragments.iter().find_map(|error_fragment| {
        cleaned_log_lines
            .iter()
            .find(|line| line.to_ascii_lowercase().contains(error_fragment))
            .cloned()
    });
    let actionable_error_line = cleaned_log_lines
        .iter()
        .find(|line| is_actionable_mpv_error_line(line))
        .cloned();
    let fallback_line = cleaned_log_lines
        .iter()
        .find(|line| !line.to_ascii_lowercase().contains("exiting"))
        .cloned()?;
    let message = priority_error_line
        .or(actionable_error_line)
        .unwrap_or(fallback_line);

    Some(truncate_status_message(&message, 180))
}

fn clean_mpv_log_line(line: &str) -> &str {
    line.rsplit_once("] ")
        .map(|(_, message)| message)
        .unwrap_or(line)
}

fn is_actionable_mpv_error_line(line: &str) -> bool {
    let lowercase_line = line.to_ascii_lowercase();
    if lowercase_line.contains("exiting")
        || lowercase_line.contains("failed to recognize file format")
        || lowercase_line.contains("failed sending hook command")
    {
        return false;
    }

    lowercase_line.contains("error")
        || lowercase_line.contains("failed")
        || lowercase_line.contains("could not")
        || lowercase_line.contains("no such")
}

fn truncate_status_message(message: &str, max_character_count: usize) -> String {
    let mut truncated_message = message
        .chars()
        .take(max_character_count)
        .collect::<String>();

    if message.chars().count() > max_character_count {
        truncated_message.push_str("...");
    }

    truncated_message
}

#[cfg(target_os = "windows")]
fn create_platform_playback_ipc_path(timestamp_millis: u128) -> String {
    format!(
        r"\\.\pipe\watch-mpv-{}-{timestamp_millis}",
        std::process::id()
    )
}

#[cfg(not(target_os = "windows"))]
fn create_platform_playback_ipc_path(timestamp_millis: u128) -> String {
    std::env::temp_dir()
        .join(format!(
            "watch-mpv-{}-{timestamp_millis}.sock",
            std::process::id()
        ))
        .display()
        .to_string()
}

fn send_mpv_ipc_command(ipc_path: &str, command: Value) -> bool {
    let command_payload = json!({ "command": command }).to_string();

    for _ in 0..40 {
        if write_mpv_ipc_payload(ipc_path, &command_payload) {
            return true;
        }

        std::thread::sleep(Duration::from_millis(25));
    }

    false
}

fn read_mpv_playback_snapshot(ipc_path: &str) -> Option<MpvPlaybackSnapshot> {
    let properties = read_mpv_properties(ipc_path, MPV_POLL_PROPERTIES)?;
    let time_pos_seconds = properties.get("time-pos").cloned().and_then(json_f64);
    let duration_seconds = properties.get("duration").cloned().and_then(json_f64);
    let is_paused = properties.get("pause").and_then(Value::as_bool);
    let is_eof_reached = properties.get("eof-reached").and_then(Value::as_bool);
    let volume_percent = properties
        .get("volume")
        .cloned()
        .and_then(json_f64)
        .map(|volume| volume.round().clamp(0.0, 100.0) as u8);
    let is_muted = properties.get("mute").and_then(Value::as_bool);
    let playback_speed = properties.get("speed").cloned().and_then(json_f64);
    let audio_track_id = properties.get("aid").cloned().and_then(json_i64);
    let subtitle_track_id = properties.get("sid").cloned().and_then(json_i64);
    let (audio_tracks, embedded_subtitle_tracks) = properties
        .get("track-list")
        .map(mpv_tracks_from_track_list)
        .unwrap_or_default();
    let chapters = properties
        .get("chapter-list")
        .map(mpv_chapters_from_value)
        .unwrap_or_default();
    let current_chapter_index = properties
        .get("chapter")
        .and_then(Value::as_i64)
        .filter(|chapter| *chapter >= 0)
        .map(|chapter| chapter as usize);

    if time_pos_seconds.is_none()
        && duration_seconds.is_none()
        && is_paused.is_none()
        && is_eof_reached.is_none()
        && volume_percent.is_none()
        && is_muted.is_none()
        && playback_speed.is_none()
        && audio_track_id.is_none()
        && subtitle_track_id.is_none()
        && audio_tracks.is_empty()
        && embedded_subtitle_tracks.is_empty()
        && chapters.is_empty()
        && current_chapter_index.is_none()
    {
        return None;
    }

    Some(MpvPlaybackSnapshot {
        time_pos_seconds,
        duration_seconds,
        is_paused,
        is_eof_reached,
        volume_percent,
        is_muted,
        playback_speed,
        audio_track_id,
        subtitle_track_id,
        chapters,
        current_chapter_index,
        audio_tracks,
        embedded_subtitle_tracks,
    })
}

fn read_mpv_properties(ipc_path: &str, property_names: &[&str]) -> Option<HashMap<String, Value>> {
    let mut ipc_stream = open_mpv_ipc_read_write(ipc_path)?;
    let mut responses = HashMap::new();

    for property_name in property_names {
        let payload = json!({
            "command": ["get_property", property_name],
            "request_id": property_name,
        })
        .to_string();

        ipc_stream.write_all(payload.as_bytes()).ok()?;
        ipc_stream.write_all(b"\n").ok()?;
    }

    ipc_stream.flush().ok()?;
    let mut reader = BufReader::new(ipc_stream);

    for _ in 0..property_names.len() {
        let mut response_line = String::new();
        if reader.read_line(&mut response_line).ok()? == 0 {
            break;
        }

        let response = serde_json::from_str::<Value>(&response_line).ok()?;
        if response.get("error").and_then(Value::as_str) != Some("success") {
            continue;
        }
        let Some(request_id) = response.get("request_id").and_then(Value::as_str) else {
            continue;
        };
        if let Some(data) = response.get("data") {
            responses.insert(request_id.to_string(), data.clone());
        }
    }

    Some(responses)
}

fn json_f64(value: Value) -> Option<f64> {
    value.as_f64().filter(|number| number.is_finite())
}

fn json_i64(value: Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str()?.parse::<i64>().ok())
}

fn mpv_tracks_from_track_list(track_list: &Value) -> (Vec<AudioTrack>, Vec<EmbeddedSubtitleTrack>) {
    let mut audio_tracks = Vec::new();
    let mut subtitle_tracks = Vec::new();

    for track in track_list.as_array().into_iter().flatten() {
        let Some(track_id) = track.get("id").and_then(Value::as_i64) else {
            continue;
        };
        let track_type = track
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let is_external = track
            .get("external")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let title = track_title_from_mpv_track(track, track_id, track_type);
        let language = track
            .get("lang")
            .and_then(Value::as_str)
            .filter(|language| !language.is_empty())
            .map(ToString::to_string);
        let codec = track
            .get("codec")
            .and_then(Value::as_str)
            .filter(|codec| !codec.is_empty())
            .map(ToString::to_string);

        match track_type {
            "audio" => audio_tracks.push(AudioTrack {
                track_id,
                title,
                language,
                codec,
            }),
            "sub" if !is_external => subtitle_tracks.push(EmbeddedSubtitleTrack {
                track_id,
                title,
                language,
                codec,
                is_selected: track
                    .get("selected")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            }),
            _ => {}
        }
    }

    (audio_tracks, subtitle_tracks)
}

fn mpv_chapters_from_value(value: &Value) -> Vec<Chapter> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|chapter| {
            let time_seconds = chapter.get("time").and_then(Value::as_f64)?;
            let title = chapter
                .get("title")
                .and_then(Value::as_str)
                .filter(|title| !title.is_empty())
                .unwrap_or("Chapter")
                .to_string();

            Some(Chapter {
                title,
                time_seconds,
            })
        })
        .collect()
}

fn track_title_from_mpv_track(track: &Value, track_id: i64, track_type: &str) -> String {
    track
        .get("title")
        .and_then(Value::as_str)
        .filter(|title| !title.is_empty())
        .or_else(|| track.get("external-filename").and_then(Value::as_str))
        .map(ToString::to_string)
        .unwrap_or_else(|| match track_type {
            "audio" => format!("Audio track {track_id}"),
            "sub" => format!("Subtitle track {track_id}"),
            _ => format!("Track {track_id}"),
        })
}

#[cfg(target_os = "windows")]
fn write_mpv_ipc_payload(ipc_path: &str, payload: &str) -> bool {
    let Ok(mut ipc_stream) = OpenOptions::new().write(true).open(ipc_path) else {
        return false;
    };

    ipc_stream.write_all(payload.as_bytes()).is_ok()
        && ipc_stream.write_all(b"\n").is_ok()
        && ipc_stream.flush().is_ok()
}

#[cfg(not(target_os = "windows"))]
fn write_mpv_ipc_payload(ipc_path: &str, payload: &str) -> bool {
    use std::os::unix::net::UnixStream;

    let Ok(mut ipc_stream) = UnixStream::connect(ipc_path) else {
        return false;
    };

    ipc_stream.write_all(payload.as_bytes()).is_ok()
        && ipc_stream.write_all(b"\n").is_ok()
        && ipc_stream.flush().is_ok()
}

#[cfg(target_os = "windows")]
fn open_mpv_ipc_read_write(ipc_path: &str) -> Option<std::fs::File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(ipc_path)
        .ok()
}

#[cfg(not(target_os = "windows"))]
fn open_mpv_ipc_read_write(ipc_path: &str) -> Option<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(ipc_path).ok()
}

#[cfg(target_os = "windows")]
fn create_embedded_video_host(parent_window_id: isize) -> Option<EmbeddedVideoHost> {
    let class_name = wide_null("STATIC");
    let window_name = [0u16];
    let parent_window_handle = parent_window_id as HWND;

    let video_host_window_id = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_name.as_ptr(),
            WS_CHILD | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            0,
            0,
            16,
            16,
            parent_window_handle,
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null_mut(),
        )
    };

    if video_host_window_id.is_null() {
        None
    } else {
        Some(EmbeddedVideoHost {
            window_id: video_host_window_id as isize,
            parent_window_id,
        })
    }
}

#[cfg(not(target_os = "windows"))]
fn create_embedded_video_host(_parent_window_id: isize) -> Option<EmbeddedVideoHost> {
    None
}

#[cfg(target_os = "windows")]
fn position_embedded_video_host(
    video_host_window_id: isize,
    _parent_window_id: isize,
    bounds: Bounds<Pixels>,
    scale_factor: f32,
    is_visible: bool,
) {
    let x = (bounds.origin.x.as_f32() * scale_factor).round() as i32;
    let y = (bounds.origin.y.as_f32() * scale_factor).round() as i32;
    let width = (bounds.size.width.as_f32() * scale_factor).round().max(1.0) as i32;
    let height = (bounds.size.height.as_f32() * scale_factor)
        .round()
        .max(1.0) as i32;

    unsafe {
        SetWindowPos(
            video_host_window_id as HWND,
            std::ptr::null_mut(),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | if is_visible { SWP_SHOWWINDOW } else { 0 },
        );
        set_embedded_video_host_visible(video_host_window_id, is_visible);
    }
}

#[cfg(not(target_os = "windows"))]
fn position_embedded_video_host(
    _video_host_window_id: isize,
    _parent_window_id: isize,
    _bounds: Bounds<Pixels>,
    _scale_factor: f32,
    _is_visible: bool,
) {
}

#[cfg(target_os = "windows")]
fn set_embedded_video_host_visible(video_host_window_id: isize, is_visible: bool) {
    unsafe {
        ShowWindow(
            video_host_window_id as HWND,
            if is_visible { SW_SHOW } else { SW_HIDE },
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn set_embedded_video_host_visible(_video_host_window_id: isize, _is_visible: bool) {}

#[cfg(target_os = "windows")]
fn wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn volume_slider(volume_percent: u8, cx: &mut Context<WatchPlayer>) -> impl IntoElement {
    let filled_width = VOLUME_SLIDER_WIDTH * f32::from(volume_percent.min(100)) / 100.0;

    div()
        .id("volume-slider")
        .relative()
        .w(px(VOLUME_SLIDER_WIDTH))
        .h(px(42.0))
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(20.0))
                .h(px(3.0))
                .bg(rgb(0x282828)),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .top(px(20.0))
                .w(px(filled_width))
                .h(px(3.0))
                .bg(rgb(SOFT_WHITE)),
        )
        .child(
            div()
                .id("volume-slider-hitbox")
                .absolute()
                .left_0()
                .right_0()
                .top_0()
                .bottom_0()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|player, event: &MouseDownEvent, window, cx| {
                        let volume_percent =
                            volume_percent_from_slider_event(window, event.position);
                        player.set_volume_percent_if_changed(volume_percent, window, cx);
                    }),
                )
                .on_mouse_move(cx.listener(|player, event: &MouseMoveEvent, window, cx| {
                    if event.dragging() {
                        let volume_percent =
                            volume_percent_from_slider_event(window, event.position);
                        player.set_volume_percent_if_changed(volume_percent, window, cx);
                    }
                })),
        )
}

fn default_volume_slider(
    default_volume_percent: u8,
    cx: &mut Context<WatchPlayer>,
) -> impl IntoElement {
    let filled_width = VOLUME_SLIDER_WIDTH * f32::from(default_volume_percent.min(100)) / 100.0;

    div()
        .id("default-volume-slider")
        .relative()
        .w(px(VOLUME_SLIDER_WIDTH))
        .h(px(34.0))
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(16.0))
                .h(px(3.0))
                .bg(rgb(0x282828)),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .top(px(16.0))
                .w(px(filled_width))
                .h(px(3.0))
                .bg(rgb(SOFT_WHITE)),
        )
        .child(
            div()
                .id("default-volume-slider-hitbox")
                .absolute()
                .left_0()
                .right_0()
                .top_0()
                .bottom_0()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|player, event: &MouseDownEvent, window, cx| {
                        let default_volume_percent =
                            volume_percent_from_slider_event(window, event.position);
                        player.set_default_volume_percent(default_volume_percent, cx);
                        cx.stop_propagation();
                    }),
                )
                .on_mouse_move(cx.listener(|player, event: &MouseMoveEvent, window, cx| {
                    if event.dragging() {
                        let default_volume_percent =
                            volume_percent_from_slider_event(window, event.position);
                        player.set_default_volume_percent(default_volume_percent, cx);
                        cx.stop_propagation();
                    }
                })),
        )
}

fn volume_percent_from_slider_event(window: &Window, position: Point<Pixels>) -> u8 {
    let viewport_width = window.viewport_size().width.as_f32();
    let slider_right_padding = 16.0;
    let slider_left = viewport_width - slider_right_padding - VOLUME_SLIDER_WIDTH;
    let local_x = position.x.as_f32() - slider_left;

    ((local_x / VOLUME_SLIDER_WIDTH) * 100.0)
        .round()
        .clamp(0.0, 100.0) as u8
}

fn format_timestamp(seconds: f64) -> String {
    let total_seconds = seconds.max(0.0).round() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(OsStr::to_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

fn startup_options_from_args() -> StartupOptions {
    let mut options = StartupOptions::default();
    let mut args = std::env::args_os().skip(1);

    while let Some(arg) = args.next() {
        match arg.to_string_lossy().as_ref() {
            "--fullscreen" => options.start_fullscreen = Some(true),
            "--windowed" => options.start_fullscreen = Some(false),
            "--resume-never" => options.resume_behavior = Some(ResumeBehavior::Never),
            "--resume-always" => options.resume_behavior = Some(ResumeBehavior::Always),
            "--resume-ask" => options.resume_behavior = Some(ResumeBehavior::Ask),
            "--folder" => {
                if let Some(folder) = args.next() {
                    options
                        .media_paths
                        .extend(media_paths_in_folder(&PathBuf::from(folder)));
                }
            }
            _ => {
                options
                    .media_paths
                    .extend(media_paths_from_path(PathBuf::from(arg)));
            }
        }
    }

    options
}

fn queue_display_name(path: &Path) -> String {
    let file_name = display_name(path);
    strip_leading_bracket_tags(&file_name)
        .filter(|cleaned_file_name| !cleaned_file_name.is_empty())
        .map(ToString::to_string)
        .unwrap_or(file_name)
}

fn playback_display_title(path: &Path) -> String {
    let episode_identity = parse_episode_identity(path);
    let Some(episode_number) = episode_identity.episode_number else {
        return queue_display_name(path);
    };

    let episode_label = episode_display_label(episode_identity.season_number, episode_number);
    let mut title_segments = vec![episode_identity.display_series_title, episode_label];

    if let Some(episode_title) = episode_identity.episode_title {
        if !episode_title.is_empty() {
            title_segments.push(episode_title);
        }
    }

    title_segments
        .into_iter()
        .filter(|title_segment| !title_segment.is_empty())
        .collect::<Vec<_>>()
        .join(" - ")
}

fn episode_display_label(season_number: Option<u32>, episode_number: u32) -> String {
    season_number
        .map(|season_number| format!("S{season_number:02}E{episode_number:02}"))
        .unwrap_or_else(|| format!("Episode {episode_number}"))
}

fn strip_leading_bracket_tags(file_name: &str) -> Option<&str> {
    let mut remaining_file_name = file_name.trim_start();
    let mut removed_tag_count = 0;

    while let Some(tag_end_index) = remaining_file_name.find(']') {
        if !remaining_file_name.starts_with('[') || tag_end_index <= 1 {
            break;
        }

        removed_tag_count += 1;
        remaining_file_name = remaining_file_name[tag_end_index + 1..].trim_start();
    }

    (removed_tag_count > 0).then_some(remaining_file_name)
}

fn extension_lowercase(path: &Path) -> Option<String> {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|extension| extension.to_ascii_lowercase())
}

fn is_video_path(path: &Path) -> bool {
    extension_lowercase(path)
        .as_deref()
        .is_some_and(|extension| VIDEO_EXTENSIONS.contains(&extension))
}

fn is_subtitle_path(path: &Path) -> bool {
    extension_lowercase(path)
        .as_deref()
        .is_some_and(|extension| SUBTITLE_EXTENSIONS.contains(&extension))
}

fn is_playlist_path(path: &Path) -> bool {
    extension_lowercase(path)
        .as_deref()
        .is_some_and(|extension| PLAYLIST_EXTENSIONS.contains(&extension))
}

fn media_paths_from_path(path: PathBuf) -> Vec<PathBuf> {
    if path.is_dir() {
        media_paths_in_folder(&path)
    } else if path.is_file() && is_video_path(&path) {
        vec![path]
    } else if path.is_file() && is_playlist_path(&path) {
        media_paths_from_playlist(&path)
    } else {
        Vec::new()
    }
}

fn video_file_dialog(title: &str) -> rfd::FileDialog {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter("Video files", &VIDEO_EXTENSIONS)
        .add_filter("Playlists", &PLAYLIST_EXTENSIONS)
}

fn media_paths_in_folder(folder_path: &Path) -> Vec<PathBuf> {
    let mut media_paths = Vec::new();
    let mut pending_dirs = vec![folder_path.to_path_buf()];

    while let Some(directory) = pending_dirs.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };

        for entry in entries.filter_map(Result::ok) {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();

            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                pending_dirs.push(path);
                continue;
            }
            if file_type.is_file() && is_video_path(&path) {
                media_paths.push(path);
            }
        }
    }

    media_paths.sort_by(|left, right| compare_natural_paths(left, right));
    media_paths.dedup();
    media_paths
}

fn media_paths_from_playlist(playlist_path: &Path) -> Vec<PathBuf> {
    let Ok(contents) = fs::read_to_string(playlist_path) else {
        return Vec::new();
    };
    let base_dir = playlist_path.parent().unwrap_or_else(|| Path::new("."));

    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let path = PathBuf::from(line);
            if path.is_absolute() {
                path
            } else {
                base_dir.join(path)
            }
        })
        .filter(|path| path.is_file() && is_video_path(path))
        .collect()
}

fn media_paths_in_folder_cached(
    folder_path: &Path,
    cache: &mut HashMap<String, FolderListingCacheEntry>,
) -> Vec<PathBuf> {
    let folder_key = library_media_path_key(folder_path);
    let modified_millis = folder_modified_millis(folder_path).unwrap_or_default();

    if let Some(entry) = cache.get(&folder_key) {
        if entry.modified_millis == modified_millis {
            return entry.media_paths.clone();
        }
    }

    let media_paths = media_paths_in_folder(folder_path);
    cache.insert(
        folder_key,
        FolderListingCacheEntry {
            folder_path: folder_path.to_path_buf(),
            modified_millis,
            media_paths: media_paths.clone(),
        },
    );

    media_paths
}

fn folder_modified_millis(path: &Path) -> Option<u128> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}

fn normalize_media_paths(media_paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen_paths = HashSet::new();
    let mut normalized_paths = media_paths
        .into_iter()
        .flat_map(media_paths_from_path)
        .filter(|path| seen_paths.insert(path.clone()))
        .collect::<Vec<_>>();

    normalized_paths.sort_by(|left, right| compare_natural_paths(left, right));
    normalized_paths
}

fn compare_natural_paths(left: &PathBuf, right: &PathBuf) -> std::cmp::Ordering {
    compare_natural_text(&left.display().to_string(), &right.display().to_string())
}

fn compare_natural_text(left: &str, right: &str) -> std::cmp::Ordering {
    let mut left_chars = left.chars().peekable();
    let mut right_chars = right.chars().peekable();

    loop {
        match (left_chars.peek(), right_chars.peek()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(left_char), Some(right_char)) => {
                if left_char.is_ascii_digit() && right_char.is_ascii_digit() {
                    let left_number = take_ascii_number(&mut left_chars);
                    let right_number = take_ascii_number(&mut right_chars);
                    let number_order = compare_number_text(&left_number, &right_number);
                    if number_order != std::cmp::Ordering::Equal {
                        return number_order;
                    }
                } else {
                    let left_char = left_chars.next().unwrap_or_default().to_ascii_lowercase();
                    let right_char = right_chars.next().unwrap_or_default().to_ascii_lowercase();
                    let char_order = left_char.cmp(&right_char);
                    if char_order != std::cmp::Ordering::Equal {
                        return char_order;
                    }
                }
            }
        }
    }
}

fn take_ascii_number(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut number = String::new();

    while chars
        .peek()
        .is_some_and(|character| character.is_ascii_digit())
    {
        if let Some(character) = chars.next() {
            number.push(character);
        }
    }

    number
}

fn compare_number_text(left: &str, right: &str) -> std::cmp::Ordering {
    let trimmed_left = left.trim_start_matches('0');
    let trimmed_right = right.trim_start_matches('0');
    let normalized_left = if trimmed_left.is_empty() {
        "0"
    } else {
        trimmed_left
    };
    let normalized_right = if trimmed_right.is_empty() {
        "0"
    } else {
        trimmed_right
    };

    normalized_left
        .len()
        .cmp(&normalized_right.len())
        .then_with(|| normalized_left.cmp(normalized_right))
        .then_with(|| left.len().cmp(&right.len()))
}

fn build_loaded_media(path: &Path, dependency_status: &DependencyStatus) -> LoadedMedia {
    LoadedMedia {
        source: LoadedMediaSource::File(path.to_path_buf()),
        duration_seconds: dependency_status
            .ffprobe_path
            .as_deref()
            .and_then(|ffprobe_path| probe_media_duration_seconds(ffprobe_path, path)),
        audio_tracks: Vec::new(),
        subtitle_paths: discover_sidecar_subtitles(path),
        embedded_subtitle_tracks: Vec::new(),
    }
}

fn probe_media_duration_seconds(ffprobe_path: &Path, media_path: &Path) -> Option<f64> {
    let mut command = Command::new(ffprobe_path);
    command
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(media_path.as_os_str())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_child_process_window(&mut command);

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|duration| duration.is_finite() && *duration > 0.0)
}

fn build_live_capture_media(device: LiveCaptureDevice) -> LoadedMedia {
    LoadedMedia {
        source: LoadedMediaSource::LiveCapture(device),
        duration_seconds: None,
        audio_tracks: Vec::new(),
        subtitle_paths: Vec::new(),
        embedded_subtitle_tracks: Vec::new(),
    }
}

fn build_internet_media(media: InternetMedia) -> LoadedMedia {
    LoadedMedia {
        source: LoadedMediaSource::Internet(media),
        duration_seconds: None,
        audio_tracks: Vec::new(),
        subtitle_paths: Vec::new(),
        embedded_subtitle_tracks: Vec::new(),
    }
}

fn list_live_capture_devices(ffmpeg_path: &Path) -> Result<LiveCaptureDeviceScan, String> {
    #[cfg(target_os = "windows")]
    {
        list_windows_directshow_video_devices(ffmpeg_path)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = ffmpeg_path;
        Err("Live capture device listing is currently supported on Windows.".to_string())
    }
}

#[cfg(target_os = "windows")]
fn list_windows_directshow_video_devices(
    ffmpeg_path: &Path,
) -> Result<LiveCaptureDeviceScan, String> {
    let mut command = Command::new(ffmpeg_path);
    command
        .arg("-hide_banner")
        .arg("-list_devices")
        .arg("true")
        .arg("-f")
        .arg("dshow")
        .arg("-i")
        .arg("dummy")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_child_process_window(&mut command);

    let output = command
        .output()
        .map_err(|error| format!("Could not list live capture devices: {error}"))?;
    let mut device_output = String::from_utf8_lossy(&output.stderr).to_string();
    device_output.push_str(&String::from_utf8_lossy(&output.stdout));

    Ok(parse_windows_directshow_video_devices(&device_output))
}

#[cfg(target_os = "windows")]
fn parse_windows_directshow_video_devices(device_output: &str) -> LiveCaptureDeviceScan {
    let mut devices = Vec::new();
    let mut audio_devices = Vec::new();
    let mut is_reading_video_devices = false;
    let mut is_reading_audio_devices = false;
    let mut audio_backend_names = Vec::new();

    for line in device_output.lines() {
        if line.contains("DirectShow video devices") {
            is_reading_video_devices = true;
            is_reading_audio_devices = false;
            continue;
        }

        if line.contains("DirectShow audio devices") {
            is_reading_video_devices = false;
            is_reading_audio_devices = true;
            continue;
        }

        let Some(quoted_value) = first_quoted_value(line) else {
            continue;
        };

        if line.contains("Alternative name") {
            continue;
        }

        let is_inline_video_device = directshow_inline_device_has_video(line);
        let is_inline_audio_device = directshow_inline_device_has_audio(line);

        if is_reading_audio_devices || (is_inline_audio_device && !is_inline_video_device) {
            audio_backend_names.push(quoted_value.clone());
            audio_devices.push(LiveCaptureAudioDevice {
                display_name: quoted_value.clone(),
                backend_name: quoted_value,
            });
            continue;
        }

        if is_reading_video_devices || is_inline_video_device {
            let audio_backend_name = is_inline_audio_device.then(|| quoted_value.clone());
            let audio_pin_name =
                is_inline_audio_device.then(|| DIRECTSHOW_INTERNAL_AUDIO_PIN_NAME.to_string());
            devices.push(LiveCaptureDevice {
                display_name: quoted_value.clone(),
                backend_name: quoted_value.clone(),
                audio_backend_name,
                audio_pin_name,
                latency_mode: LiveCaptureLatencyMode::UltraLow,
            });
        }
    }

    for device in &mut devices {
        if device.audio_backend_name.is_none() {
            device.audio_backend_name =
                find_directshow_audio_device_for_video(&device.display_name, &audio_backend_names);
            device.audio_pin_name = None;
        }
    }

    LiveCaptureDeviceScan {
        video_devices: devices,
        audio_devices,
    }
}

#[cfg(target_os = "windows")]
fn find_directshow_audio_device_for_video(
    video_device_name: &str,
    audio_backend_names: &[String],
) -> Option<String> {
    audio_backend_names
        .iter()
        .find(|audio_backend_name| audio_backend_name.as_str() == video_device_name)
        .cloned()
        .or_else(|| {
            audio_backend_names
                .iter()
                .filter_map(|audio_backend_name| {
                    let compatibility_score = directshow_device_name_compatibility_score(
                        video_device_name,
                        audio_backend_name,
                    );
                    (compatibility_score > 0).then(|| (compatibility_score, audio_backend_name))
                })
                .max_by_key(|(compatibility_score, _)| *compatibility_score)
                .map(|(_, audio_backend_name)| audio_backend_name.clone())
        })
}

#[cfg(target_os = "windows")]
fn directshow_device_name_compatibility_score(
    video_device_name: &str,
    audio_device_name: &str,
) -> usize {
    let video_name_tokens = directshow_device_name_match_tokens(video_device_name);
    let audio_name_tokens = directshow_device_name_match_tokens(audio_device_name);

    video_name_tokens.intersection(&audio_name_tokens).count()
}

#[cfg(target_os = "windows")]
fn directshow_device_name_match_tokens(device_name: &str) -> HashSet<String> {
    device_name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|token| is_meaningful_directshow_device_token(token))
        .collect()
}

#[cfg(target_os = "windows")]
fn is_meaningful_directshow_device_token(token: &str) -> bool {
    const GENERIC_DIRECTSHOW_DEVICE_TOKENS: &[&str] = &[
        "audio",
        "camera",
        "capture",
        "default",
        "device",
        "digital",
        "directshow",
        "input",
        "microphone",
        "output",
        "video",
        "virtual",
    ];

    token.len() >= 4 && !GENERIC_DIRECTSHOW_DEVICE_TOKENS.contains(&token)
}

#[cfg(target_os = "windows")]
fn directshow_inline_device_has_video(line: &str) -> bool {
    line.contains("(video)") || line.contains("(audio, video)")
}

#[cfg(target_os = "windows")]
fn directshow_inline_device_has_audio(line: &str) -> bool {
    line.contains("(audio)") || line.contains("(audio, video)")
}

#[cfg(target_os = "windows")]
fn first_quoted_value(value: &str) -> Option<String> {
    let start_index = value.find('"')? + 1;
    let remaining_value = &value[start_index..];
    let end_index = remaining_value.find('"')?;

    Some(remaining_value[..end_index].to_string())
}

fn directshow_capture_url(video_device_name: &str, audio_device_name: Option<&str>) -> String {
    if let Some(audio_device_name) = audio_device_name {
        format!("av://dshow:video={video_device_name}:audio={audio_device_name}")
    } else {
        format!("av://dshow:video={video_device_name}")
    }
}

fn load_saved_watch_session() -> Option<SavedWatchSession> {
    let session_file_path = watch_session_file_path();
    let serialized_session = fs::read_to_string(&session_file_path).ok()?;
    let session_value = serde_json::from_str::<Value>(&serialized_session).ok()?;
    let current_queue_index = session_value
        .get("current_queue_index")
        .and_then(Value::as_u64)
        .map(|queue_index| queue_index as usize)
        .unwrap_or(0);
    let playback_position_seconds = session_value
        .get("playback_position_seconds")
        .and_then(Value::as_f64)
        .filter(|position_seconds| position_seconds.is_finite())
        .unwrap_or(0.0)
        .max(0.0);

    if playback_position_seconds < MINIMUM_RESUME_POSITION_SECONDS {
        clear_saved_watch_session();
        return None;
    }

    let mut media_paths = session_value
        .get("media_paths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .collect::<Vec<_>>();

    let current_media_path = session_value
        .get("current_media_path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| media_paths.get(current_queue_index).cloned())?;

    if !media_paths
        .iter()
        .any(|media_path| media_path == &current_media_path)
    {
        media_paths.push(current_media_path.clone());
    }

    let available_media_paths = media_paths
        .into_iter()
        .filter(|media_path| media_path.is_file() && is_video_path(media_path))
        .collect::<Vec<_>>();

    if available_media_paths.is_empty()
        || !current_media_path.is_file()
        || !is_video_path(&current_media_path)
    {
        clear_saved_watch_session();
        return None;
    }

    let volume_percent = session_value
        .get("volume_percent")
        .and_then(Value::as_u64)
        .map(|volume_percent| volume_percent.min(100) as u8)
        .unwrap_or(64);
    let is_muted = session_value
        .get("is_muted")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Some(SavedWatchSession {
        media_paths: available_media_paths,
        current_media_path,
        current_queue_index,
        playback_position_seconds,
        volume_percent,
        is_muted,
    })
}

fn save_watch_session(watch_session: &SavedWatchSession) {
    let session_file_path = watch_session_file_path();
    let Some(session_directory) = session_file_path.parent() else {
        return;
    };

    if fs::create_dir_all(session_directory).is_err() {
        return;
    }

    let session_value = json!({
        "media_paths": watch_session
            .media_paths
            .iter()
            .map(|media_path| media_path.display().to_string())
            .collect::<Vec<_>>(),
        "current_media_path": watch_session.current_media_path.display().to_string(),
        "current_queue_index": watch_session.current_queue_index,
        "playback_position_seconds": watch_session.playback_position_seconds,
        "volume_percent": watch_session.volume_percent,
        "is_muted": watch_session.is_muted,
    });
    let Ok(serialized_session) = serde_json::to_vec_pretty(&session_value) else {
        return;
    };

    let _ = write_json_file_atomic(&session_file_path, &serialized_session);
}

fn clear_saved_watch_session() {
    let _ = fs::remove_file(watch_session_file_path());
}

fn watch_session_file_path() -> PathBuf {
    watch_session_directory().join(WATCH_SESSION_FILE_NAME)
}

fn watch_settings_file_path() -> PathBuf {
    watch_session_directory().join(WATCH_SETTINGS_FILE_NAME)
}

fn watch_library_file_path() -> PathBuf {
    watch_session_directory().join(WATCH_LIBRARY_FILE_NAME)
}

fn load_player_settings() -> PlayerSettings {
    let settings_file_path = watch_settings_file_path();
    let Ok(serialized_settings) = fs::read_to_string(settings_file_path) else {
        return PlayerSettings::default();
    };
    serde_json::from_str::<PlayerSettings>(&serialized_settings).unwrap_or_default()
}

fn save_player_settings(settings: &PlayerSettings) {
    let settings_file_path = watch_settings_file_path();
    let Some(settings_directory) = settings_file_path.parent() else {
        return;
    };
    if fs::create_dir_all(settings_directory).is_err() {
        return;
    }

    let Ok(serialized_settings) = serde_json::to_vec_pretty(settings) else {
        return;
    };

    let _ = write_json_file_atomic(&settings_file_path, &serialized_settings);
}

fn load_player_library() -> PlayerLibrary {
    let library_file_path = watch_library_file_path();
    let Ok(serialized_library) = fs::read_to_string(library_file_path) else {
        return PlayerLibrary::default();
    };
    let Ok(library_value) = serde_json::from_str::<Value>(&serialized_library) else {
        return PlayerLibrary::default();
    };

    PlayerLibrary {
        recent_media_paths: json_paths(&library_value, "recent_media_paths")
            .into_iter()
            .filter(|path| path.is_file() && is_video_path(path))
            .take(MAX_RECENT_MEDIA)
            .collect(),
        recent_folder_paths: json_paths(&library_value, "recent_folder_paths")
            .into_iter()
            .filter(|path| path.is_dir())
            .take(MAX_RECENT_FOLDERS)
            .collect(),
        pinned_folder_paths: json_paths(&library_value, "pinned_folder_paths")
            .into_iter()
            .filter(|path| path.is_dir())
            .take(MAX_RECENT_FOLDERS)
            .collect(),
        media_history: library_value
            .get("media_history")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(media_history_entry_from_json)
            .collect(),
        recent_internet_media: library_value
            .get("recent_internet_media")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(internet_media_from_json)
            .take(MAX_RECENT_MEDIA)
            .collect(),
        internet_media_history: library_value
            .get("internet_media_history")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(internet_media_history_entry_from_json)
            .collect(),
    }
}

fn save_player_library_atomic(library: &PlayerLibrary) {
    let library_file_path = watch_library_file_path();
    let Some(library_directory) = library_file_path.parent() else {
        return;
    };
    if fs::create_dir_all(library_directory).is_err() {
        return;
    }

    let library_value = json!({
        "recent_media_paths": library
            .recent_media_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>(),
        "recent_folder_paths": library
            .recent_folder_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>(),
        "pinned_folder_paths": library
            .pinned_folder_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>(),
        "media_history": library
            .media_history
            .iter()
            .map(|entry| {
                json!({
                    "path": entry.path.display().to_string(),
                    "playback_position_seconds": entry.playback_position_seconds,
                    "duration_seconds": entry.duration_seconds,
                    "is_completed": entry.is_completed,
                    "updated_at_millis": entry.updated_at_millis,
                    "selected_audio_track_id": entry.selected_audio_track_id,
                    "selected_embedded_subtitle_track_id": entry.selected_embedded_subtitle_track_id,
                    "selected_subtitle_path": entry.selected_subtitle_path
                        .as_ref()
                        .map(|path| path.display().to_string()),
                })
            })
            .collect::<Vec<_>>(),
        "recent_internet_media": library
            .recent_internet_media
            .iter()
            .map(internet_media_to_json)
            .collect::<Vec<_>>(),
        "internet_media_history": library
            .internet_media_history
            .iter()
            .map(|entry| {
                json!({
                    "media": internet_media_to_json(&entry.media),
                    "playback_position_seconds": entry.playback_position_seconds,
                    "duration_seconds": entry.duration_seconds,
                    "is_completed": entry.is_completed,
                    "updated_at_millis": entry.updated_at_millis,
                })
            })
            .collect::<Vec<_>>(),
    });
    let Ok(serialized_library) = serde_json::to_vec_pretty(&library_value) else {
        return;
    };

    let _ = write_json_file_atomic(&library_file_path, &serialized_library);
}

fn write_json_file_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, bytes)?;
    fs::rename(temp_path, path)?;
    Ok(())
}

fn json_paths(parent_value: &Value, key: &str) -> Vec<PathBuf> {
    parent_value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .collect()
}

fn media_history_entry_from_json(history_value: &Value) -> Option<MediaHistoryEntry> {
    let path = history_value
        .get("path")
        .and_then(Value::as_str)
        .map(PathBuf::from)?;

    if !path.is_file() || !is_video_path(&path) {
        return None;
    }

    Some(MediaHistoryEntry {
        path,
        playback_position_seconds: history_value
            .get("playback_position_seconds")
            .and_then(Value::as_f64)
            .filter(|position_seconds| position_seconds.is_finite())
            .unwrap_or(0.0)
            .max(0.0),
        duration_seconds: history_value
            .get("duration_seconds")
            .and_then(Value::as_f64)
            .filter(|duration_seconds| duration_seconds.is_finite() && *duration_seconds > 0.0),
        is_completed: history_value
            .get("is_completed")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        updated_at_millis: history_value
            .get("updated_at_millis")
            .and_then(Value::as_u64)
            .map(u128::from)
            .unwrap_or_default(),
        selected_audio_track_id: history_value
            .get("selected_audio_track_id")
            .and_then(Value::as_i64),
        selected_embedded_subtitle_track_id: history_value
            .get("selected_embedded_subtitle_track_id")
            .and_then(Value::as_i64),
        selected_subtitle_path: history_value
            .get("selected_subtitle_path")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .filter(|path| path.is_file() && is_subtitle_path(path)),
    })
}

fn internet_media_from_json(media_value: &Value) -> Option<InternetMedia> {
    let provider_id = media_value
        .get("provider_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let provider_name = media_value
        .get("provider_name")
        .and_then(Value::as_str)
        .unwrap_or("Source")
        .trim()
        .to_string();
    let title = media_value
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())?
        .to_string();
    let stream_url = media_value
        .get("stream_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| is_http_url(url))?
        .to_string();

    Some(InternetMedia {
        provider_id,
        provider_name,
        title,
        series_title: media_value
            .get("series_title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToString::to_string),
        episode_title: media_value
            .get("episode_title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToString::to_string),
        episode_number: media_value
            .get("episode_number")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|number| !number.is_empty())
            .map(ToString::to_string),
        subtitle: media_value
            .get("subtitle")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|subtitle| !subtitle.is_empty())
            .map(ToString::to_string),
        stream_url,
        thumbnail_url: media_value
            .get("thumbnail_url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| is_http_url(url))
            .map(ToString::to_string),
        http_headers: media_value
            .get("http_headers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(internet_media_http_header_from_json)
            .collect(),
    })
}

fn internet_media_http_header_from_json(header_value: &Value) -> Option<InternetMediaHttpHeader> {
    let name = header_value
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())?
        .to_string();
    let value = header_value
        .get("value")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();

    Some(InternetMediaHttpHeader { name, value })
}

fn internet_media_history_entry_from_json(
    history_value: &Value,
) -> Option<InternetMediaHistoryEntry> {
    let media = history_value
        .get("media")
        .and_then(internet_media_from_json)?;

    Some(InternetMediaHistoryEntry {
        media,
        playback_position_seconds: history_value
            .get("playback_position_seconds")
            .and_then(Value::as_f64)
            .filter(|position_seconds| position_seconds.is_finite())
            .unwrap_or(0.0)
            .max(0.0),
        duration_seconds: history_value
            .get("duration_seconds")
            .and_then(Value::as_f64)
            .filter(|duration_seconds| duration_seconds.is_finite() && *duration_seconds > 0.0),
        is_completed: history_value
            .get("is_completed")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        updated_at_millis: history_value
            .get("updated_at_millis")
            .and_then(Value::as_u64)
            .map(u128::from)
            .unwrap_or_default(),
    })
}

fn internet_media_to_json(media: &InternetMedia) -> Value {
    json!({
        "provider_id": media.provider_id.clone(),
        "provider_name": media.provider_name.clone(),
        "title": media.title.clone(),
        "series_title": media.series_title.clone(),
        "episode_title": media.episode_title.clone(),
        "episode_number": media.episode_number.clone(),
        "subtitle": media.subtitle.clone(),
        "stream_url": media.stream_url.clone(),
        "thumbnail_url": media.thumbnail_url.clone(),
        "http_headers": media
            .http_headers
            .iter()
            .map(|header| {
                json!({
                    "name": header.name.clone(),
                    "value": header.value.clone(),
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn apply_source_media_metadata(
    media: &mut InternetMedia,
    series_title: &str,
    episode_title: Option<&str>,
    thumbnail_url: Option<String>,
) {
    if media.series_title.is_none() && !series_title.trim().is_empty() {
        media.series_title = Some(series_title.trim().to_string());
    }
    if media.episode_title.is_none() {
        media.episode_title = episode_title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToString::to_string);
    }
    if media.episode_number.is_none() {
        media.episode_number = media
            .episode_title
            .as_deref()
            .and_then(source_episode_number_from_text)
            .or_else(|| source_episode_number_from_text(&media.title));
    }
    if media.thumbnail_url.is_none() {
        media.thumbnail_url = thumbnail_url;
    }
}

fn current_time_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn promote_recent_path(recent_paths: &mut Vec<PathBuf>, path: PathBuf, max_len: usize) {
    let promoted_path_key = library_media_path_key(&path);
    recent_paths.retain(|recent_path| library_media_path_key(recent_path) != promoted_path_key);
    recent_paths.insert(0, path);
    recent_paths.truncate(max_len);
}

fn promote_recent_internet_media(
    recent_media: &mut Vec<InternetMedia>,
    media: InternetMedia,
    max_len: usize,
) {
    let promoted_media_key = internet_media_key(&media);
    recent_media.retain(|recent_media| internet_media_key(recent_media) != promoted_media_key);
    recent_media.insert(0, media);
    recent_media.truncate(max_len);
}

fn internet_media_key(media: &InternetMedia) -> String {
    format!(
        "{:016x}",
        stable_hash_bytes(format!("{}|{}", media.provider_id, media.stream_url).as_bytes())
    )
}

fn internet_media_library_path(media: &InternetMedia) -> PathBuf {
    PathBuf::from(format!("internet-{}", internet_media_key(media)))
}

fn library_media_path_key(path: &Path) -> String {
    let normalized_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_key = normalized_path.display().to_string();

    if cfg!(windows) {
        path_key.replace('/', "\\").to_ascii_lowercase()
    } else {
        path_key
    }
}

fn default_audio_output_options() -> Vec<AudioOutputDeviceOption> {
    vec![AudioOutputDeviceOption {
        label: "Auto".to_string(),
        device_id: AUDIO_OUTPUT_AUTO_DEVICE_ID.to_string(),
    }]
}

fn built_in_allanime_provider() -> SourceProvider {
    SourceProvider {
        id: ALLANIME_PROVIDER_ID.to_string(),
        name: ALLANIME_PROVIDER_NAME.to_string(),
        search_url_template: format!("{ALLANIME_SOURCE_URL_PREFIX}search"),
        episodes_url_template: None,
        streams_url_template: None,
    }
}

fn available_source_providers(custom_source_providers: &[SourceProvider]) -> Vec<SourceProvider> {
    let mut source_providers = Vec::with_capacity(custom_source_providers.len() + 1);
    source_providers.push(built_in_allanime_provider());
    source_providers.extend(custom_source_providers.iter().cloned());
    source_providers
}

fn available_source_provider_count(settings: &PlayerSettings) -> usize {
    available_source_providers(&settings.source_providers).len()
}

fn is_allanime_provider(provider: &SourceProvider) -> bool {
    provider.id == ALLANIME_PROVIDER_ID
}

fn allanime_episodes_source_url(show_id: &str) -> String {
    format!("{ALLANIME_SOURCE_URL_PREFIX}episodes/{show_id}")
}

fn allanime_streams_source_url(show_id: &str, episode_number: &str) -> String {
    format!("{ALLANIME_SOURCE_URL_PREFIX}streams/{show_id}/{episode_number}")
}

fn allanime_show_id_from_episodes_url(episodes_url: &str) -> Option<String> {
    episodes_url
        .strip_prefix(&format!("{ALLANIME_SOURCE_URL_PREFIX}episodes/"))
        .map(str::trim)
        .filter(|show_id| !show_id.is_empty())
        .map(ToString::to_string)
}

fn allanime_stream_parts_from_streams_url(streams_url: &str) -> Option<(String, String)> {
    let remaining_url =
        streams_url.strip_prefix(&format!("{ALLANIME_SOURCE_URL_PREFIX}streams/"))?;
    let (show_id, episode_number) = remaining_url.split_once('/')?;
    let show_id = show_id.trim();
    let episode_number = episode_number.trim();

    if show_id.is_empty() || episode_number.is_empty() {
        None
    } else {
        Some((show_id.to_string(), episode_number.to_string()))
    }
}

fn source_provider_from_input(input: &str) -> Option<SourceProvider> {
    let trimmed_input = input.trim();
    if trimmed_input.is_empty() {
        return None;
    }

    let provider_parts = trimmed_input.split('|').map(str::trim).collect::<Vec<_>>();
    if provider_parts.len() > 4 {
        return None;
    }

    let (name_hint, raw_search_url_template) = if provider_parts.len() == 1 {
        ("", provider_parts[0])
    } else {
        (provider_parts[0], provider_parts[1])
    };
    if !is_http_url(raw_search_url_template) {
        return None;
    }

    let episodes_url_template = provider_parts
        .get(2)
        .map(|template| template.trim())
        .filter(|template| !template.is_empty())
        .map(ToString::to_string);
    if episodes_url_template
        .as_deref()
        .is_some_and(|template| !is_http_url(template))
    {
        return None;
    }

    let streams_url_template = provider_parts
        .get(3)
        .map(|template| template.trim())
        .filter(|template| !template.is_empty())
        .map(ToString::to_string);
    if streams_url_template
        .as_deref()
        .is_some_and(|template| !is_http_url(template))
    {
        return None;
    }

    let search_url_template = normalize_source_provider_template(raw_search_url_template);
    let name = if name_hint.is_empty() {
        source_provider_name_from_url(&search_url_template)
    } else {
        collapse_whitespace(name_hint)
    };
    let id = format!(
        "{:016x}",
        stable_hash_bytes(
            format!(
                "{name}|{search_url_template}|{}|{}",
                episodes_url_template.as_deref().unwrap_or_default(),
                streams_url_template.as_deref().unwrap_or_default()
            )
            .as_bytes()
        )
    );

    Some(SourceProvider {
        id,
        name,
        search_url_template,
        episodes_url_template,
        streams_url_template,
    })
}

fn normalize_source_provider_template(raw_url_template: &str) -> String {
    let trimmed_template = raw_url_template.trim().to_string();
    if trimmed_template.contains("{query}") {
        return trimmed_template;
    }

    if trimmed_template.contains('?') {
        format!("{trimmed_template}&q={{query}}")
    } else {
        format!("{trimmed_template}?q={{query}}")
    }
}

fn source_provider_name_from_url(url_template: &str) -> String {
    let without_scheme = url_template
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("Source")
        .trim_start_matches("www.");

    if host.is_empty() {
        "Source".to_string()
    } else {
        host.to_string()
    }
}

fn upsert_source_provider(source_providers: &mut Vec<SourceProvider>, provider: SourceProvider) {
    source_providers.retain(|existing_provider| {
        existing_provider.id != provider.id
            && existing_provider.search_url_template != provider.search_url_template
    });
    source_providers.insert(0, provider);
}

fn search_configured_source_providers(
    source_providers: Vec<SourceProvider>,
    search_query: String,
) -> Result<Vec<SourceSearchResult>, String> {
    let mut search_results = Vec::new();
    let mut error_messages = Vec::new();

    for provider in source_providers {
        match fetch_source_provider_results(&provider, &search_query) {
            Ok(mut provider_results) => search_results.append(&mut provider_results),
            Err(error_message) => error_messages.push(error_message),
        }
    }

    search_results.truncate(MAX_LIBRARY_ITEMS_PER_SHELF);
    if !search_results.is_empty() {
        Ok(search_results)
    } else if let Some(error_message) = error_messages.into_iter().next() {
        Err(error_message)
    } else {
        Ok(Vec::new())
    }
}

fn fetch_source_provider_results(
    provider: &SourceProvider,
    search_query: &str,
) -> Result<Vec<SourceSearchResult>, String> {
    if is_allanime_provider(provider) {
        return fetch_allanime_source_provider_results(provider, search_query);
    }

    let search_url = provider_search_url(provider, search_query);
    let response = ureq::get(&search_url)
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|error| format!("{} search failed: {error}", provider.name))?;
    let response_json = response
        .into_json::<Value>()
        .map_err(|error| format!("{} returned invalid JSON: {error}", provider.name))?;

    Ok(parse_source_provider_results(provider, &response_json))
}

fn fetch_allanime_source_provider_results(
    provider: &SourceProvider,
    search_query: &str,
) -> Result<Vec<SourceSearchResult>, String> {
    Ok(allanime::search_anime(search_query)?
        .into_iter()
        .map(|anime_result| SourceSearchResult {
            provider: provider.clone(),
            item_id: Some(anime_result.show_id.clone()),
            title: anime_result.title,
            subtitle: anime_result.subtitle,
            episodes_url: Some(allanime_episodes_source_url(&anime_result.show_id)),
            streams_url: None,
            direct_media: None,
            thumbnail_url: anime_result.thumbnail_url,
        })
        .collect())
}

fn provider_search_url(provider: &SourceProvider, search_query: &str) -> String {
    provider
        .search_url_template
        .replace("{query}", &percent_encode_query_component(search_query))
}

fn parse_source_provider_results(
    provider: &SourceProvider,
    response_json: &Value,
) -> Vec<SourceSearchResult> {
    source_result_values(
        response_json,
        &["results", "items", "series", "anime", "data"],
    )
    .into_iter()
    .filter_map(|result_value| parse_source_provider_result(provider, result_value))
    .collect()
}

fn parse_source_provider_result(
    provider: &SourceProvider,
    result_value: &Value,
) -> Option<SourceSearchResult> {
    let title = text_json_field(result_value, &["title", "name", "series", "label"])?;
    let subtitle = string_json_field(
        result_value,
        &[
            "subtitle",
            "description",
            "releaseDate",
            "type",
            "subOrDub",
            "episode",
            "season",
            "year",
        ],
    );
    let thumbnail_url = string_json_field(
        result_value,
        &[
            "thumbnail_url",
            "thumbnailUrl",
            "thumbnail",
            "poster",
            "image",
        ],
    )
    .filter(|url| is_http_url(url));
    let stream_url = playable_stream_url(result_value, false);
    let direct_media = stream_url.map(|stream_url| InternetMedia {
        provider_id: provider.id.clone(),
        provider_name: provider.name.clone(),
        title: title.clone(),
        series_title: Some(title.clone()),
        episode_title: None,
        episode_number: None,
        subtitle: subtitle.clone(),
        stream_url,
        thumbnail_url: thumbnail_url.clone(),
        http_headers: Vec::new(),
    });

    Some(SourceSearchResult {
        provider: provider.clone(),
        item_id: source_series_item_id(result_value),
        title,
        subtitle,
        episodes_url: source_url_json_field(
            result_value,
            &[
                "episodes_url",
                "episodesUrl",
                "episode_list_url",
                "episodeListUrl",
            ],
        ),
        streams_url: source_url_json_field(
            result_value,
            &["streams_url", "streamsUrl", "sources_url", "sourcesUrl"],
        ),
        direct_media,
        thumbnail_url,
    })
}

fn fetch_source_episode_results(
    provider: &SourceProvider,
    series_title: &str,
    episodes_url: &str,
) -> Result<Vec<SourceEpisodeResult>, String> {
    if is_allanime_provider(provider) {
        return fetch_allanime_source_episode_results(provider, series_title, episodes_url);
    }

    let response_json = fetch_source_json(provider, episodes_url, "episodes")?;
    Ok(
        source_result_values(&response_json, &["episodes", "results", "items", "data"])
            .into_iter()
            .filter_map(|episode_value| {
                parse_source_episode_result(provider, series_title, episode_value)
            })
            .collect(),
    )
}

fn fetch_allanime_source_episode_results(
    provider: &SourceProvider,
    series_title: &str,
    episodes_url: &str,
) -> Result<Vec<SourceEpisodeResult>, String> {
    let show_id = allanime_show_id_from_episodes_url(episodes_url)
        .ok_or_else(|| "AllAnime episode URL was invalid.".to_string())?;

    Ok(allanime::fetch_episodes(&show_id)?
        .into_iter()
        .map(|episode_result| SourceEpisodeResult {
            provider: provider.clone(),
            series_title: series_title.to_string(),
            item_id: Some(episode_result.episode_number.clone()),
            title: episode_result.title,
            subtitle: episode_result.subtitle,
            streams_url: Some(allanime_streams_source_url(
                &show_id,
                &episode_result.episode_number,
            )),
            direct_media: None,
        })
        .collect())
}

fn parse_source_episode_result(
    provider: &SourceProvider,
    series_title: &str,
    episode_value: &Value,
) -> Option<SourceEpisodeResult> {
    let title = text_json_field(
        episode_value,
        &["title", "name", "episodeTitle", "episode_title", "label"],
    )
    .or_else(|| {
        text_json_field(
            episode_value,
            &["number", "episode", "episodeNumber", "episode_number"],
        )
        .map(|episode_number| format!("Episode {episode_number}"))
    })?;
    let subtitle = string_json_field(
        episode_value,
        &["subtitle", "description", "releaseDate", "type", "subOrDub"],
    );
    let episode_number = source_episode_item_id(episode_value);
    let stream_url = playable_stream_url(episode_value, false);
    let direct_media = stream_url.map(|stream_url| InternetMedia {
        provider_id: provider.id.clone(),
        provider_name: provider.name.clone(),
        title: format!("{series_title} - {title}"),
        series_title: Some(series_title.to_string()),
        episode_title: Some(title.clone()),
        episode_number: episode_number.clone(),
        subtitle: subtitle.clone(),
        stream_url,
        thumbnail_url: None,
        http_headers: Vec::new(),
    });

    Some(SourceEpisodeResult {
        provider: provider.clone(),
        series_title: series_title.to_string(),
        item_id: episode_number,
        title,
        subtitle,
        streams_url: source_url_json_field(
            episode_value,
            &["streams_url", "streamsUrl", "sources_url", "sourcesUrl"],
        ),
        direct_media,
    })
}

fn fetch_source_stream_results(
    provider: &SourceProvider,
    series_title: &str,
    episode_title: Option<&str>,
    streams_url: &str,
) -> Result<Vec<SourceStreamResult>, String> {
    if is_allanime_provider(provider) {
        return fetch_allanime_source_stream_results(
            provider,
            series_title,
            episode_title,
            streams_url,
        );
    }

    let response_json = fetch_source_json(provider, streams_url, "streams")?;
    Ok(source_result_values(
        &response_json,
        &["sources", "streams", "links", "results", "items", "data"],
    )
    .into_iter()
    .filter_map(|stream_value| {
        parse_source_stream_result(provider, series_title, episode_title, stream_value)
    })
    .collect())
}

fn fetch_allanime_source_stream_results(
    provider: &SourceProvider,
    series_title: &str,
    episode_title: Option<&str>,
    streams_url: &str,
) -> Result<Vec<SourceStreamResult>, String> {
    let (show_id, episode_number) = allanime_stream_parts_from_streams_url(streams_url)
        .ok_or_else(|| "AllAnime stream URL was invalid.".to_string())?;
    let media_title = episode_title
        .map(|title| format!("{series_title} - {title}"))
        .unwrap_or_else(|| series_title.to_string());

    Ok(allanime::fetch_streams(&show_id, &episode_number)?
        .into_iter()
        .map(|stream_result| {
            let http_headers = stream_result
                .http_headers
                .into_iter()
                .map(|(name, value)| InternetMediaHttpHeader { name, value })
                .collect::<Vec<_>>();

            SourceStreamResult {
                quality: stream_result.quality.clone(),
                media: InternetMedia {
                    provider_id: provider.id.clone(),
                    provider_name: provider.name.clone(),
                    title: media_title.clone(),
                    series_title: Some(series_title.to_string()),
                    episode_title: episode_title.map(ToString::to_string),
                    episode_number: Some(episode_number.clone()),
                    subtitle: stream_result.subtitle,
                    stream_url: stream_result.stream_url,
                    thumbnail_url: None,
                    http_headers,
                },
            }
        })
        .collect())
}

fn parse_source_stream_result(
    provider: &SourceProvider,
    series_title: &str,
    episode_title: Option<&str>,
    stream_value: &Value,
) -> Option<SourceStreamResult> {
    let stream_url = playable_stream_url(stream_value, true)?;
    let quality = text_json_field(stream_value, &["quality", "label", "resolution", "name"]);
    let media_title = episode_title
        .map(|title| format!("{series_title} - {title}"))
        .unwrap_or_else(|| series_title.to_string());
    let subtitle = quality
        .as_ref()
        .map(|quality| format!("{} / {quality}", provider.name))
        .or_else(|| Some(provider.name.clone()));

    Some(SourceStreamResult {
        media: InternetMedia {
            provider_id: provider.id.clone(),
            provider_name: provider.name.clone(),
            title: media_title,
            series_title: Some(series_title.to_string()),
            episode_title: episode_title.map(ToString::to_string),
            episode_number: episode_title.and_then(source_episode_number_from_text),
            subtitle,
            stream_url,
            thumbnail_url: None,
            http_headers: Vec::new(),
        },
        quality,
    })
}

fn fetch_source_json(
    provider: &SourceProvider,
    source_url: &str,
    action: &str,
) -> Result<Value, String> {
    let response = ureq::get(source_url)
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|error| format!("{} {action} request failed: {error}", provider.name))?;
    response.into_json::<Value>().map_err(|error| {
        format!(
            "{} returned invalid JSON for {action}: {error}",
            provider.name
        )
    })
}

fn source_result_values<'a>(response_json: &'a Value, candidate_keys: &[&str]) -> Vec<&'a Value> {
    if let Some(results) = response_json.as_array() {
        return results.iter().collect();
    }

    for key in candidate_keys {
        if let Some(results) = response_json.get(*key).and_then(Value::as_array) {
            return results.iter().collect();
        }
    }

    for wrapper_key in ["data", "payload", "response"] {
        if let Some(wrapper_value) = response_json.get(wrapper_key) {
            let nested_results = source_result_values(wrapper_value, candidate_keys);
            if !nested_results.is_empty() {
                return nested_results;
            }
        }
    }

    if response_json.is_object() {
        vec![response_json]
    } else {
        Vec::new()
    }
}

fn source_series_item_id(value: &Value) -> Option<String> {
    text_json_field(
        value,
        &[
            "id",
            "slug",
            "anime_id",
            "animeId",
            "series_id",
            "seriesId",
            "malId",
            "anilistId",
        ],
    )
}

fn source_episode_item_id(value: &Value) -> Option<String> {
    text_json_field(
        value,
        &[
            "episode_id",
            "episodeId",
            "id",
            "slug",
            "number",
            "episodeNumber",
            "episode_number",
        ],
    )
}

fn source_search_result_has_episode_step(search_result: &SourceSearchResult) -> bool {
    source_search_result_episodes_url(search_result).is_some()
}

fn source_search_result_has_stream_step(search_result: &SourceSearchResult) -> bool {
    source_search_result_streams_url(search_result).is_some()
}

fn source_episode_result_has_stream_step(episode_result: &SourceEpisodeResult) -> bool {
    source_episode_result_streams_url(episode_result).is_some()
}

fn source_search_result_episodes_url(search_result: &SourceSearchResult) -> Option<String> {
    if is_allanime_provider(&search_result.provider) {
        return search_result.episodes_url.clone().or_else(|| {
            search_result
                .item_id
                .as_deref()
                .map(allanime_episodes_source_url)
        });
    }

    search_result
        .episodes_url
        .clone()
        .filter(|url| is_http_url(url))
        .or_else(|| {
            search_result
                .provider
                .episodes_url_template
                .as_deref()
                .and_then(|template| {
                    fill_source_url_template(
                        template,
                        &[
                            ("id", search_result.item_id.as_deref()),
                            ("series_id", search_result.item_id.as_deref()),
                            ("title", Some(search_result.title.as_str())),
                            ("query", Some(search_result.title.as_str())),
                        ],
                    )
                })
        })
}

fn source_search_result_streams_url(search_result: &SourceSearchResult) -> Option<String> {
    if is_allanime_provider(&search_result.provider) {
        return search_result.streams_url.clone();
    }

    search_result
        .streams_url
        .clone()
        .filter(|url| is_http_url(url))
        .or_else(|| {
            search_result
                .provider
                .streams_url_template
                .as_deref()
                .and_then(|template| {
                    fill_source_url_template(
                        template,
                        &[
                            ("id", search_result.item_id.as_deref()),
                            ("series_id", search_result.item_id.as_deref()),
                            ("title", Some(search_result.title.as_str())),
                            ("query", Some(search_result.title.as_str())),
                        ],
                    )
                })
        })
}

fn source_episode_result_streams_url(episode_result: &SourceEpisodeResult) -> Option<String> {
    if is_allanime_provider(&episode_result.provider) {
        return episode_result.streams_url.clone();
    }

    episode_result
        .streams_url
        .clone()
        .filter(|url| is_http_url(url))
        .or_else(|| {
            episode_result
                .provider
                .streams_url_template
                .as_deref()
                .and_then(|template| {
                    fill_source_url_template(
                        template,
                        &[
                            ("id", episode_result.item_id.as_deref()),
                            ("episode_id", episode_result.item_id.as_deref()),
                            ("episode_title", Some(episode_result.title.as_str())),
                            ("series_title", Some(episode_result.series_title.as_str())),
                            ("title", Some(episode_result.title.as_str())),
                        ],
                    )
                })
        })
}

fn fill_source_url_template(
    raw_url_template: &str,
    replacements: &[(&str, Option<&str>)],
) -> Option<String> {
    let mut filled_url = raw_url_template.trim().to_string();
    for (placeholder_name, replacement_value) in replacements {
        let Some(replacement_value) = replacement_value else {
            continue;
        };
        filled_url = filled_url.replace(
            &format!("{{{placeholder_name}}}"),
            &percent_encode_query_component(replacement_value),
        );
    }

    if filled_url.contains('{') || filled_url.contains('}') || !is_http_url(&filled_url) {
        return None;
    }

    Some(filled_url)
}

fn source_search_result_key(search_result: &SourceSearchResult) -> String {
    let direct_stream_url = search_result
        .direct_media
        .as_ref()
        .map(|media| media.stream_url.as_str())
        .unwrap_or_default();
    format!(
        "{:016x}",
        stable_hash_bytes(
            format!(
                "{}|{}|{}|{}|{}|{}|{}",
                search_result.provider.id,
                search_result.item_id.as_deref().unwrap_or_default(),
                search_result.title,
                search_result.episodes_url.as_deref().unwrap_or_default(),
                search_result.streams_url.as_deref().unwrap_or_default(),
                search_result.thumbnail_url.as_deref().unwrap_or_default(),
                direct_stream_url
            )
            .as_bytes()
        )
    )
}

fn source_episode_result_key(episode_result: &SourceEpisodeResult) -> String {
    let direct_stream_url = episode_result
        .direct_media
        .as_ref()
        .map(|media| media.stream_url.as_str())
        .unwrap_or_default();
    format!(
        "{:016x}",
        stable_hash_bytes(
            format!(
                "{}|{}|{}|{}|{}",
                episode_result.provider.id,
                episode_result.series_title,
                episode_result.item_id.as_deref().unwrap_or_default(),
                episode_result.title,
                direct_stream_url
            )
            .as_bytes()
        )
    )
}

fn source_search_result_kind_label(search_result: &SourceSearchResult) -> &'static str {
    if search_result.direct_media.is_some() {
        "Direct"
    } else if source_search_result_has_episode_step(search_result) {
        "Episodes"
    } else if source_search_result_has_stream_step(search_result) {
        "Streams"
    } else {
        "Source"
    }
}

fn source_episode_result_kind_label(episode_result: &SourceEpisodeResult) -> &'static str {
    if episode_result.direct_media.is_some() {
        "Direct"
    } else if source_episode_result_has_stream_step(episode_result) {
        "Streams"
    } else {
        "Episode"
    }
}

fn source_url_json_field(value: &Value, candidate_keys: &[&str]) -> Option<String> {
    string_json_field(value, candidate_keys).filter(|url| is_http_url(url))
}

fn playable_stream_url(value: &Value, include_generic_url: bool) -> Option<String> {
    let explicit_url = source_url_json_field(
        value,
        &[
            "stream_url",
            "streamUrl",
            "play_url",
            "playUrl",
            "video_url",
            "videoUrl",
            "file_url",
            "fileUrl",
        ],
    );
    if explicit_url.is_some() || !include_generic_url {
        return explicit_url;
    }

    source_url_json_field(value, &["url", "file", "src"])
}

fn text_json_field(value: &Value, candidate_keys: &[&str]) -> Option<String> {
    candidate_keys.iter().find_map(|key| {
        value.get(*key).and_then(|field_value| match field_value {
            Value::String(text) => {
                let trimmed_text = text.trim();
                (!trimmed_text.is_empty()).then(|| trimmed_text.to_string())
            }
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(boolean) => Some(boolean.to_string()),
            _ => None,
        })
    })
}

fn string_json_field(value: &Value, candidate_keys: &[&str]) -> Option<String> {
    candidate_keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|field_value| !field_value.is_empty())
            .map(ToString::to_string)
    })
}

fn is_http_url(url: &str) -> bool {
    let lowercase_url = url.to_ascii_lowercase();
    lowercase_url.starts_with("https://") || lowercase_url.starts_with("http://")
}

fn percent_encode_query_component(value: &str) -> String {
    let mut encoded_value = String::new();

    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.' | b'~') {
            encoded_value.push(*byte as char);
        } else {
            encoded_value.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded_value
}

fn list_audio_output_devices(mpv_path: &Path) -> Vec<AudioOutputDeviceOption> {
    let mut command = Command::new(mpv_path);
    command
        .arg("--no-config")
        .arg("--audio-device=help")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_child_process_window(&mut command);

    let Ok(output) = command.output() else {
        return default_audio_output_options();
    };

    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));

    parse_mpv_audio_device_help(&text)
}

fn parse_mpv_audio_device_help(output: &str) -> Vec<AudioOutputDeviceOption> {
    let mut devices = default_audio_output_options();
    let mut seen_device_ids = HashSet::from([AUDIO_OUTPUT_AUTO_DEVICE_ID.to_string()]);

    for line in output.lines() {
        let line = line.trim();
        let Some((device_id, label)) = line.split_once(' ') else {
            continue;
        };
        let device_id = device_id.trim();
        if (!device_id.contains('/') && device_id != AUDIO_OUTPUT_AUTO_DEVICE_ID)
            || !seen_device_ids.insert(device_id.to_string())
        {
            continue;
        }

        devices.push(AudioOutputDeviceOption {
            label: label.trim().to_string(),
            device_id: device_id.to_string(),
        });
    }

    devices
}

fn on_off_label(is_enabled: bool) -> &'static str {
    if is_enabled {
        "On"
    } else {
        "Off"
    }
}

fn on_off_message_word(is_enabled: bool) -> &'static str {
    if is_enabled {
        "on"
    } else {
        "off"
    }
}

fn live_capture_audio_source_label(
    live_capture_audio_source: &str,
    live_capture_audio_devices: &[LiveCaptureAudioDevice],
) -> String {
    match live_capture_audio_source {
        LIVE_CAPTURE_AUDIO_SOURCE_AUTO => "Auto".to_string(),
        LIVE_CAPTURE_AUDIO_SOURCE_NONE => "Video only".to_string(),
        audio_backend_name => live_capture_audio_devices
            .iter()
            .find(|audio_device| audio_device.backend_name == audio_backend_name)
            .map(|audio_device| audio_device.display_name.clone())
            .unwrap_or_else(|| "Custom audio source".to_string()),
    }
}

fn preferred_audio_track_id(audio_tracks: &[AudioTrack], preferred_language: &str) -> Option<i64> {
    audio_tracks
        .iter()
        .find(|track| language_matches(track.language.as_deref(), preferred_language))
        .map(|track| track.track_id)
}

fn preferred_embedded_subtitle_track_id(
    subtitle_tracks: &[EmbeddedSubtitleTrack],
    preferred_language: &str,
) -> Option<i64> {
    subtitle_tracks
        .iter()
        .find(|track| language_matches(track.language.as_deref(), preferred_language))
        .map(|track| track.track_id)
}

fn preferred_sidecar_subtitle_path(
    subtitle_paths: &[PathBuf],
    preferred_language: &str,
) -> Option<PathBuf> {
    subtitle_paths
        .iter()
        .find(|path| {
            path.file_stem()
                .and_then(OsStr::to_str)
                .map(|stem| language_token_matches(stem, preferred_language))
                .unwrap_or(false)
        })
        .cloned()
}

fn language_matches(language: Option<&str>, preferred_language: &str) -> bool {
    let Some(language) = language else {
        return preferred_language == "any";
    };
    preferred_language == "any" || normalize_language_code(language) == preferred_language
}

fn language_token_matches(text: &str, preferred_language: &str) -> bool {
    if preferred_language == "any" {
        return true;
    }
    let normalized_text = text.to_ascii_lowercase();
    let preferred_aliases = language_aliases(preferred_language);

    preferred_aliases
        .iter()
        .any(|alias| normalized_text.contains(alias))
}

fn normalize_language_code(language: &str) -> String {
    match language.to_ascii_lowercase().as_str() {
        "en" | "eng" | "english" => "eng".to_string(),
        "ja" | "jpn" | "jp" | "japanese" => "jpn".to_string(),
        "es" | "spa" | "spanish" => "spa".to_string(),
        "fr" | "fre" | "fra" | "french" => "fra".to_string(),
        value => value.to_string(),
    }
}

fn language_aliases(language: &str) -> Vec<&'static str> {
    match language {
        "eng" => vec!["eng", "en", "english"],
        "jpn" => vec!["jpn", "jp", "ja", "japanese"],
        "spa" => vec!["spa", "es", "spanish"],
        "fra" => vec!["fra", "fre", "fr", "french"],
        _ => vec![""],
    }
}

fn compare_episode_paths(left: &PathBuf, right: &PathBuf) -> std::cmp::Ordering {
    let left_episode = parse_episode_identity(left);
    let right_episode = parse_episode_identity(right);

    left_episode
        .normalized_series_key
        .cmp(&right_episode.normalized_series_key)
        .then_with(|| {
            left_episode
                .season_number
                .unwrap_or(0)
                .cmp(&right_episode.season_number.unwrap_or(0))
        })
        .then_with(|| {
            left_episode
                .episode_number
                .unwrap_or(0)
                .cmp(&right_episode.episode_number.unwrap_or(0))
        })
        .then_with(|| compare_natural_paths(left, right))
}

#[derive(Clone)]
struct EpisodeIdentity {
    display_series_title: String,
    normalized_series_key: String,
    season_number: Option<u32>,
    episode_number: Option<u32>,
    episode_title: Option<String>,
}

struct EpisodeMarker {
    season_number: Option<u32>,
    episode_number: u32,
    marker_start_index: usize,
    marker_end_index: usize,
}

fn parse_episode_identity(path: &Path) -> EpisodeIdentity {
    let file_stem = path.file_stem().and_then(OsStr::to_str).unwrap_or_default();
    let cleaned_stem = clean_media_file_stem(file_stem);
    let lowercase_stem = cleaned_stem.to_ascii_lowercase();

    if let Some(marker) = parse_episode_marker(&lowercase_stem) {
        let series_title = format_media_title_segment(&cleaned_stem[..marker.marker_start_index]);
        let fallback_series_title = if series_title.is_empty() {
            display_name(path)
        } else {
            series_title
        };

        return EpisodeIdentity {
            normalized_series_key: normalized_series_key(&fallback_series_title),
            display_series_title: fallback_series_title,
            season_number: marker.season_number,
            episode_number: Some(marker.episode_number),
            episode_title: episode_title_after_marker(&cleaned_stem, marker.marker_end_index),
        };
    }

    let display_series_title = format_media_title_segment(&cleaned_stem);
    let display_series_title = if display_series_title.is_empty() {
        display_name(path)
    } else {
        display_series_title
    };

    EpisodeIdentity {
        normalized_series_key: normalized_series_key(&display_series_title),
        display_series_title,
        season_number: None,
        episode_number: None,
        episode_title: None,
    }
}

fn parse_episode_marker(text: &str) -> Option<EpisodeMarker> {
    parse_season_episode_marker(text)
        .or_else(|| parse_season_word_episode_marker(text))
        .or_else(|| parse_short_season_dash_episode_marker(text))
        .or_else(|| parse_dash_episode_marker(text))
}

fn parse_season_episode_marker(text: &str) -> Option<EpisodeMarker> {
    let bytes = text.as_bytes();

    for marker_index in 0..bytes.len().saturating_sub(3) {
        if bytes[marker_index] != b's' || !is_token_boundary(bytes, marker_index) {
            continue;
        }

        let Some((season_number, season_end)) = parse_number_at(text, marker_index + 1, 2) else {
            continue;
        };
        if bytes.get(season_end).copied() != Some(b'e') {
            continue;
        }
        let Some((episode_number, episode_end)) = parse_number_at(text, season_end + 1, 3) else {
            continue;
        };

        return Some(EpisodeMarker {
            season_number: Some(season_number),
            episode_number,
            marker_start_index: marker_index,
            marker_end_index: episode_end,
        });
    }

    None
}

fn parse_season_word_episode_marker(text: &str) -> Option<EpisodeMarker> {
    let season_index = text.find("season")?;
    let season_number_start = skip_ascii_separators(text, season_index + "season".len());
    let (season_number, season_number_end) = parse_number_at(text, season_number_start, 2)?;
    let episode_relative_index = text[season_number_end..].find("episode")?;
    let episode_index = season_number_end + episode_relative_index;
    let episode_number_start = skip_ascii_separators(text, episode_index + "episode".len());
    let (episode_number, episode_number_end) = parse_number_at(text, episode_number_start, 3)?;

    Some(EpisodeMarker {
        season_number: Some(season_number),
        episode_number,
        marker_start_index: season_index,
        marker_end_index: episode_number_end,
    })
}

fn parse_short_season_dash_episode_marker(text: &str) -> Option<EpisodeMarker> {
    let bytes = text.as_bytes();

    for marker_index in 0..bytes.len().saturating_sub(2) {
        if bytes[marker_index] != b's' || !is_token_boundary(bytes, marker_index) {
            continue;
        }
        let Some((season_number, season_end)) = parse_number_at(text, marker_index + 1, 2) else {
            continue;
        };
        let episode_start = skip_ascii_separators(text, season_end);
        let Some((episode_number, episode_end)) = parse_number_at(text, episode_start, 3) else {
            continue;
        };

        return Some(EpisodeMarker {
            season_number: Some(season_number),
            episode_number,
            marker_start_index: marker_index,
            marker_end_index: episode_end,
        });
    }

    None
}

fn parse_dash_episode_marker(text: &str) -> Option<EpisodeMarker> {
    for (dash_index, character) in text.char_indices().rev() {
        if character != '-' {
            continue;
        }

        let episode_start = skip_ascii_separators(text, dash_index + character.len_utf8());
        let Some((episode_number, episode_end)) = parse_number_at(text, episode_start, 3) else {
            continue;
        };
        if text[..dash_index].trim().is_empty() {
            continue;
        }

        return Some(EpisodeMarker {
            season_number: None,
            episode_number,
            marker_start_index: dash_index,
            marker_end_index: episode_end,
        });
    }

    None
}

fn parse_number_at(text: &str, start_index: usize, max_digits: usize) -> Option<(u32, usize)> {
    let bytes = text.as_bytes();
    let mut end_index = start_index;

    while end_index < bytes.len()
        && end_index - start_index < max_digits
        && bytes[end_index].is_ascii_digit()
    {
        end_index += 1;
    }

    if end_index == start_index {
        return None;
    }

    text[start_index..end_index]
        .parse::<u32>()
        .ok()
        .map(|number| (number, end_index))
}

fn skip_ascii_separators(text: &str, start_index: usize) -> usize {
    let mut index = start_index;
    let bytes = text.as_bytes();

    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace() || matches!(byte, b'.' | b'_' | b'-'))
    {
        index += 1;
    }

    index
}

fn is_token_boundary(bytes: &[u8], index: usize) -> bool {
    index == 0 || !bytes[index - 1].is_ascii_alphanumeric()
}

fn clean_media_file_stem(file_stem: &str) -> String {
    collapse_whitespace(
        &remove_bracketed_segments(file_stem)
            .replace(['.', '_'], " ")
            .trim_matches([' ', '-', '.', '_'])
            .to_string(),
    )
}

fn remove_bracketed_segments(text: &str) -> String {
    let mut cleaned = String::new();
    let mut bracket_depth = 0usize;

    for character in text.chars() {
        match character {
            '[' | '(' => {
                bracket_depth += 1;
                cleaned.push(' ');
            }
            ']' | ')' => {
                bracket_depth = bracket_depth.saturating_sub(1);
                cleaned.push(' ');
            }
            _ if bracket_depth == 0 => cleaned.push(character),
            _ => {}
        }
    }

    cleaned
}

fn episode_title_after_marker(text: &str, marker_end_index: usize) -> Option<String> {
    let episode_title = format_media_title_segment(&text[marker_end_index..]);
    (!episode_title.is_empty()).then_some(episode_title)
}

fn format_media_title_segment(segment: &str) -> String {
    let cleaned_title = collapse_whitespace(
        &segment
            .replace(['.', '_'], " ")
            .trim_matches([' ', '-', '.', '_'])
            .to_string(),
    );
    let title_without_release_tokens = remove_release_tokens(&cleaned_title);

    format_title_case_if_needed(&title_without_release_tokens)
}

fn remove_release_tokens(title: &str) -> String {
    let release_tokens = [
        "480p", "720p", "1080p", "2160p", "4k", "8bit", "10bit", "x264", "x265", "h264", "h265",
        "hevc", "aac", "flac", "web", "web-dl", "webrip", "bluray", "bdrip", "hdr", "proper",
    ];
    let words = title
        .split_whitespace()
        .filter(|word| {
            let normalized_word = word
                .trim_matches(|character: char| !character.is_ascii_alphanumeric())
                .to_ascii_lowercase();
            !release_tokens.contains(&normalized_word.as_str())
        })
        .collect::<Vec<_>>();

    words.join(" ")
}

fn format_title_case_if_needed(title: &str) -> String {
    let alphabetic_characters = title
        .chars()
        .filter(|character| character.is_ascii_alphabetic())
        .collect::<Vec<_>>();
    let has_lowercase_character = alphabetic_characters
        .iter()
        .any(|character| character.is_ascii_lowercase());
    let has_uppercase_character = alphabetic_characters
        .iter()
        .any(|character| character.is_ascii_uppercase());
    let should_title_case =
        !alphabetic_characters.is_empty() && (has_lowercase_character != has_uppercase_character);

    if !should_title_case {
        return title.to_string();
    }

    let lower_title = title.to_ascii_lowercase();
    let small_words = [
        "a", "an", "and", "as", "at", "but", "by", "for", "from", "in", "into", "no", "of", "on",
        "or", "the", "to", "with",
    ];

    lower_title
        .split_whitespace()
        .enumerate()
        .map(|(word_index, word)| {
            if word_index > 0 && small_words.contains(&word) {
                word.to_string()
            } else {
                capitalize_ascii_word(word)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize_ascii_word(word: &str) -> String {
    let mut characters = word.chars();
    let Some(first_character) = characters.next() else {
        return String::new();
    };

    format!(
        "{}{}",
        first_character.to_ascii_uppercase(),
        characters.as_str()
    )
}

fn normalized_series_key(title: &str) -> String {
    collapse_whitespace(&title.to_ascii_lowercase())
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn library_item_for_media_path(path: &Path, prefer_episode_label: bool) -> LibraryGridItem {
    let episode_identity = parse_episode_identity(path);
    let title = if prefer_episode_label {
        episode_identity.episode_title.clone().unwrap_or_default()
    } else {
        episode_identity.display_series_title.clone()
    };
    let episode_badge = episode_identity
        .episode_number
        .map(|episode_number| episode_number.to_string());

    LibraryGridItem {
        path: path.to_path_buf(),
        title,
        subtitle: None,
        episode_badge,
        thumbnail_media_path: Some(path.to_path_buf()),
        thumbnail_url: None,
        internet_media: None,
        resume_history_entry: None,
        is_watched: false,
        is_internet_media: false,
        can_remove_from_continue_watching: false,
    }
}

fn build_library_shelves(library: &PlayerLibrary, show_unwatched_only: bool) -> Vec<LibraryShelf> {
    let watched_media_path_keys = library
        .media_history
        .iter()
        .filter(|entry| entry.is_completed)
        .map(|entry| library_media_path_key(&entry.path))
        .collect::<HashSet<_>>();
    let mut series_shelves =
        series_library_shelves(library, &watched_media_path_keys, show_unwatched_only);
    series_shelves.extend(source_media_series_library_shelves(
        library,
        show_unwatched_only,
    ));
    series_shelves.sort_by(|left, right| compare_natural_text(&left.title, &right.title));
    let grouped_media_paths = series_shelves
        .iter()
        .flat_map(|shelf| shelf.items.iter().map(|item| item.path.clone()))
        .collect::<HashSet<_>>();

    let mut shelves = Vec::new();
    shelves.push(LibraryShelf {
        key: "pinned-folders".to_string(),
        title: "Pinned Folders".to_string(),
        subtitle: None,
        empty_message: "No pinned folders yet.",
        items: pinned_folder_library_items(library),
    });
    shelves.push(LibraryShelf {
        key: "continue-watching".to_string(),
        title: "Continue Watching".to_string(),
        subtitle: None,
        empty_message: "No in-progress media yet.",
        items: continue_watching_library_items(library),
    });
    shelves.push(LibraryShelf {
        key: "recent-media".to_string(),
        title: "Recent Media".to_string(),
        subtitle: None,
        empty_message: "No recent media yet.",
        items: recent_media_library_items(library, &grouped_media_paths, &watched_media_path_keys),
    });
    shelves.extend(series_shelves);

    shelves
        .into_iter()
        .filter(|shelf| !shelf.items.is_empty())
        .collect()
}

#[derive(Clone)]
struct SourceMediaIdentity {
    display_series_title: String,
    normalized_series_key: String,
    episode_number: Option<String>,
    episode_sort_number: Option<f64>,
    episode_title: Option<String>,
}

fn source_media_series_library_shelves(
    library: &PlayerLibrary,
    show_unwatched_only: bool,
) -> Vec<LibraryShelf> {
    let completed_internet_media_keys = library
        .internet_media_history
        .iter()
        .filter(|entry| entry.is_completed)
        .map(|entry| internet_media_key(&entry.media))
        .collect::<HashSet<_>>();
    let mut media_by_series_key: HashMap<String, Vec<(SourceMediaIdentity, InternetMedia)>> =
        HashMap::new();
    let mut display_title_by_series_key: HashMap<String, String> = HashMap::new();

    for media in &library.recent_internet_media {
        if show_unwatched_only && completed_internet_media_keys.contains(&internet_media_key(media))
        {
            continue;
        }

        let identity = source_media_identity(media);
        display_title_by_series_key
            .entry(identity.normalized_series_key.clone())
            .or_insert_with(|| identity.display_series_title.clone());
        media_by_series_key
            .entry(identity.normalized_series_key.clone())
            .or_default()
            .push((identity, media.clone()));
    }

    let mut shelves = media_by_series_key
        .into_iter()
        .filter_map(|(series_key, mut entries)| {
            entries.sort_by(compare_source_media_entries);
            entries.dedup_by(|left, right| {
                internet_media_key(&left.1) == internet_media_key(&right.1)
            });

            let title = display_title_by_series_key
                .get(&series_key)
                .cloned()
                .unwrap_or_else(|| series_key.clone());
            let subtitle = source_media_series_subtitle(&entries);
            let items = entries
                .into_iter()
                .take(MAX_LIBRARY_ITEMS_PER_SHELF)
                .map(|(identity, media)| {
                    let mut item = source_media_library_item(&media, &identity);
                    item.is_watched =
                        completed_internet_media_keys.contains(&internet_media_key(&media));
                    item
                })
                .collect::<Vec<_>>();

            (!items.is_empty()).then(|| LibraryShelf {
                key: format!("source-series-{series_key}"),
                title,
                subtitle,
                empty_message: "No episodes found.",
                items,
            })
        })
        .collect::<Vec<_>>();

    shelves.sort_by(|left, right| compare_natural_text(&left.title, &right.title));
    shelves
}

fn source_media_library_item(
    media: &InternetMedia,
    identity: &SourceMediaIdentity,
) -> LibraryGridItem {
    LibraryGridItem {
        path: internet_media_library_path(media),
        title: identity
            .episode_title
            .clone()
            .unwrap_or_else(|| media.title.clone()),
        subtitle: media
            .subtitle
            .clone()
            .or_else(|| Some(media.provider_name.clone())),
        episode_badge: identity.episode_number.clone(),
        thumbnail_media_path: None,
        thumbnail_url: media.thumbnail_url.clone(),
        internet_media: Some(media.clone()),
        resume_history_entry: None,
        is_watched: false,
        is_internet_media: true,
        can_remove_from_continue_watching: false,
    }
}

fn source_media_identity(media: &InternetMedia) -> SourceMediaIdentity {
    let (series_title_from_title, episode_title_from_title) =
        split_source_media_title(&media.title);
    let display_series_title = media
        .series_title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(ToString::to_string)
        .or(series_title_from_title)
        .unwrap_or_else(|| media.title.clone());
    let episode_title = media
        .episode_title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(ToString::to_string)
        .or(episode_title_from_title);
    let episode_number = media
        .episode_number
        .as_deref()
        .map(str::trim)
        .filter(|number| !number.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            episode_title
                .as_deref()
                .and_then(source_episode_number_from_text)
        })
        .or_else(|| source_episode_number_from_text(&media.title));
    let episode_sort_number = episode_number
        .as_deref()
        .and_then(|number| number.parse::<f64>().ok());

    SourceMediaIdentity {
        normalized_series_key: normalized_series_key(&display_series_title),
        display_series_title,
        episode_number,
        episode_sort_number,
        episode_title,
    }
}

fn split_source_media_title(title: &str) -> (Option<String>, Option<String>) {
    title
        .split_once(" - ")
        .map(|(series_title, episode_title)| {
            (
                Some(series_title.trim().to_string()).filter(|title| !title.is_empty()),
                Some(episode_title.trim().to_string()).filter(|title| !title.is_empty()),
            )
        })
        .unwrap_or((None, None))
}

fn source_episode_number_from_text(text: &str) -> Option<String> {
    let lowercase_text = text.to_ascii_lowercase();
    let episode_index = lowercase_text.find("episode")?;
    let number_start = skip_ascii_separators(&lowercase_text, episode_index + "episode".len());
    let bytes = lowercase_text.as_bytes();
    let mut number_end = number_start;

    while number_end < bytes.len()
        && (bytes[number_end].is_ascii_digit() || bytes[number_end] == b'.')
    {
        number_end += 1;
    }

    (number_end > number_start).then(|| lowercase_text[number_start..number_end].to_string())
}

fn source_media_series_subtitle(
    entries: &[(SourceMediaIdentity, InternetMedia)],
) -> Option<String> {
    let mut provider_names = entries
        .iter()
        .map(|(_, media)| media.provider_name.clone())
        .filter(|provider_name| !provider_name.trim().is_empty())
        .collect::<Vec<_>>();

    provider_names.sort_by(|left, right| compare_natural_text(left, right));
    provider_names.dedup_by(|left, right| left.eq_ignore_ascii_case(right));

    if provider_names.is_empty() {
        None
    } else {
        Some(provider_names.join(" / "))
    }
}

fn compare_source_media_entries(
    left: &(SourceMediaIdentity, InternetMedia),
    right: &(SourceMediaIdentity, InternetMedia),
) -> std::cmp::Ordering {
    left.0
        .episode_sort_number
        .partial_cmp(&right.0.episode_sort_number)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| compare_natural_text(&left.1.title, &right.1.title))
}

fn continue_watching_library_items(library: &PlayerLibrary) -> Vec<LibraryGridItem> {
    library
        .media_history
        .iter()
        .filter(|entry| {
            !entry.is_completed
                && entry.playback_position_seconds >= MINIMUM_RESUME_POSITION_SECONDS
                && entry.path.is_file()
        })
        .take(MAX_LIBRARY_ITEMS_PER_SHELF)
        .map(|history_entry| {
            let mut item = library_item_for_media_path(&history_entry.path, false);
            item.subtitle = Some(format!(
                "{} watched",
                format_timestamp(history_entry.playback_position_seconds)
            ));
            item.resume_history_entry = Some(history_entry.clone());
            item.can_remove_from_continue_watching = true;
            item
        })
        .collect()
}

fn recent_media_library_items(
    library: &PlayerLibrary,
    grouped_media_paths: &HashSet<PathBuf>,
    watched_media_path_keys: &HashSet<String>,
) -> Vec<LibraryGridItem> {
    library
        .recent_media_paths
        .iter()
        .filter(|path| path.is_file() && is_video_path(path))
        .filter(|path| !grouped_media_paths.contains(*path))
        .take(MAX_LIBRARY_ITEMS_PER_SHELF)
        .map(|path| {
            let mut item = library_item_for_media_path(path, false);
            item.is_watched = watched_media_path_keys.contains(&library_media_path_key(path));
            item
        })
        .collect()
}

fn pinned_folder_library_items(library: &PlayerLibrary) -> Vec<LibraryGridItem> {
    library
        .pinned_folder_paths
        .iter()
        .filter(|path| path.is_dir())
        .map(|path| LibraryGridItem {
            path: path.clone(),
            title: display_name(path),
            subtitle: Some("Pinned folder".to_string()),
            episode_badge: None,
            thumbnail_media_path: None,
            thumbnail_url: None,
            internet_media: None,
            resume_history_entry: None,
            is_watched: false,
            is_internet_media: false,
            can_remove_from_continue_watching: false,
        })
        .collect()
}

fn series_library_shelves(
    library: &PlayerLibrary,
    watched_media_path_keys: &HashSet<String>,
    show_unwatched_only: bool,
) -> Vec<LibraryShelf> {
    let mut media_paths_by_series_key: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut display_title_by_series_key: HashMap<String, String> = HashMap::new();

    for media_path in library_series_media_paths(library) {
        let episode_identity = parse_episode_identity(&media_path);
        if episode_identity.episode_number.is_none() {
            continue;
        }

        display_title_by_series_key
            .entry(episode_identity.normalized_series_key.clone())
            .or_insert_with(|| episode_identity.display_series_title.clone());
        media_paths_by_series_key
            .entry(episode_identity.normalized_series_key)
            .or_default()
            .push(media_path);
    }

    let mut shelves = media_paths_by_series_key
        .into_iter()
        .filter_map(|(series_key, mut media_paths)| {
            media_paths.sort_by(compare_episode_paths);
            media_paths.dedup();

            if media_paths.len() < 2 {
                return None;
            }

            let title = display_title_by_series_key
                .get(&series_key)
                .cloned()
                .unwrap_or_else(|| series_key.clone());
            let subtitle = season_subtitle_for_media_paths(&media_paths);
            let items = media_paths
                .into_iter()
                .filter(|media_path| {
                    !show_unwatched_only
                        || !watched_media_path_keys.contains(&library_media_path_key(media_path))
                })
                .take(MAX_LIBRARY_ITEMS_PER_SHELF)
                .map(|media_path| {
                    let mut item = library_item_for_media_path(&media_path, true);
                    item.is_watched =
                        watched_media_path_keys.contains(&library_media_path_key(&media_path));
                    item
                })
                .collect::<Vec<_>>();

            Some(LibraryShelf {
                key: format!("series-{series_key}"),
                title,
                subtitle,
                empty_message: "No episodes found.",
                items,
            })
        })
        .collect::<Vec<_>>();

    shelves.sort_by(|left, right| compare_natural_text(&left.title, &right.title));
    shelves
}

fn season_subtitle_for_media_paths(media_paths: &[PathBuf]) -> Option<String> {
    let mut season_numbers = media_paths
        .iter()
        .filter_map(|media_path| parse_episode_identity(media_path).season_number)
        .collect::<Vec<_>>();

    season_numbers.sort_unstable();
    season_numbers.dedup();

    if season_numbers.is_empty() {
        return None;
    }

    Some(
        season_numbers
            .into_iter()
            .map(|season_number| format!("Season {season_number}"))
            .collect::<Vec<_>>()
            .join(" / "),
    )
}

fn library_series_media_paths(library: &PlayerLibrary) -> Vec<PathBuf> {
    let mut seen_media_paths = HashSet::new();
    let mut media_paths = Vec::new();

    for media_path in library
        .media_history
        .iter()
        .map(|entry| entry.path.clone())
        .chain(library.recent_media_paths.iter().cloned())
    {
        let media_path_key = library_media_path_key(&media_path);
        if media_path.is_file()
            && is_video_path(&media_path)
            && seen_media_paths.insert(media_path_key)
        {
            media_paths.push(media_path);
        }
    }

    for folder_path in library
        .recent_folder_paths
        .iter()
        .chain(library.pinned_folder_paths.iter())
    {
        for media_path in media_paths_in_folder(folder_path) {
            let media_path_key = library_media_path_key(&media_path);
            if seen_media_paths.insert(media_path_key) {
                media_paths.push(media_path);
            }
        }
    }

    media_paths
}

fn library_ui_scale(viewport_width: f32, viewport_height: f32) -> f32 {
    let width_scale = viewport_width / LIBRARY_BASE_VIEWPORT_WIDTH_PX;
    let height_scale = viewport_height / LIBRARY_BASE_VIEWPORT_HEIGHT_PX;

    (width_scale * height_scale)
        .sqrt()
        .clamp(LIBRARY_MIN_UI_SCALE, LIBRARY_MAX_UI_SCALE)
}

fn library_content_width(viewport_width: f32, library_scale: f32) -> f32 {
    (viewport_width - (LIBRARY_HORIZONTAL_MARGIN_PX * 2.0 * library_scale)).clamp(
        320.0 * library_scale,
        LIBRARY_MAX_CONTENT_WIDTH_PX * library_scale,
    )
}

fn library_card_width(viewport_width: f32, library_scale: f32) -> f32 {
    let content_width = library_content_width(viewport_width, library_scale);
    let target_card_count = library_target_card_count(content_width, library_scale);
    let gap_width = LIBRARY_CARD_GAP_PX * library_scale * (target_card_count - 1.0);

    ((content_width - gap_width) / target_card_count).clamp(
        LIBRARY_MIN_CARD_WIDTH_PX * library_scale,
        LIBRARY_MAX_CARD_WIDTH_PX * library_scale,
    )
}

fn library_visible_card_count(viewport_width: f32, library_scale: f32) -> usize {
    let content_width = library_content_width(viewport_width, library_scale);
    let card_width = library_card_width(viewport_width, library_scale);
    let card_gap = LIBRARY_CARD_GAP_PX * library_scale;

    ((content_width + card_gap) / (card_width + card_gap))
        .floor()
        .max(1.0) as usize
}

fn library_target_card_count(content_width: f32, library_scale: f32) -> f32 {
    (content_width / (LIBRARY_PREFERRED_CARD_WIDTH_PX * library_scale))
        .round()
        .clamp(
            LIBRARY_MIN_VISIBLE_CARD_COUNT,
            LIBRARY_MAX_VISIBLE_CARD_COUNT,
        )
}

fn generate_timeline_thumbnail(
    ffmpeg_path: &Path,
    media_path: &Path,
    position_seconds: f64,
) -> Option<PathBuf> {
    let thumbnail_path = timeline_thumbnail_path(media_path, position_seconds);
    let thumbnail_second = thumbnail_second_for_position(position_seconds);
    if thumbnail_path.is_file() {
        return Some(thumbnail_path);
    }
    if let Some(parent_directory) = thumbnail_path.parent() {
        fs::create_dir_all(parent_directory).ok()?;
    }

    let mut command = Command::new(ffmpeg_path);
    command
        .arg("-y")
        .arg("-ss")
        .arg(thumbnail_second.to_string())
        .arg("-i")
        .arg(media_path.as_os_str())
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg("scale=360:-1")
        .arg("-q:v")
        .arg("6")
        .arg(thumbnail_path.as_os_str())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_child_process_window(&mut command);

    let output = command.status().ok()?;

    output.success().then_some(thumbnail_path)
}

fn library_thumbnail_media_paths(library: &PlayerLibrary) -> Vec<PathBuf> {
    let mut seen_media_paths = HashSet::new();
    let mut media_paths = Vec::new();

    for media_path in library
        .media_history
        .iter()
        .map(|entry| entry.path.clone())
        .chain(library.recent_media_paths.iter().cloned())
        .chain(library_series_media_paths(library))
    {
        if media_path.is_file()
            && is_video_path(&media_path)
            && seen_media_paths.insert(media_path.clone())
        {
            media_paths.push(media_path);
        }
    }

    media_paths
}

fn internet_library_thumbnail_urls(library: &PlayerLibrary) -> Vec<String> {
    let mut seen_thumbnail_urls = HashSet::new();
    let mut thumbnail_urls = Vec::new();

    for thumbnail_url in library
        .recent_internet_media
        .iter()
        .chain(
            library
                .internet_media_history
                .iter()
                .map(|history_entry| &history_entry.media),
        )
        .filter_map(|media| media.thumbnail_url.clone())
        .filter(|thumbnail_url| is_http_url(thumbnail_url))
    {
        if seen_thumbnail_urls.insert(thumbnail_url.clone()) {
            thumbnail_urls.push(thumbnail_url);
        }
    }

    thumbnail_urls
}

fn existing_library_thumbnail_path(media_path: &Path) -> Option<PathBuf> {
    let preferred_thumbnail_path = library_thumbnail_path(media_path);
    if preferred_thumbnail_path.is_file() {
        return Some(preferred_thumbnail_path);
    }

    first_existing_timeline_thumbnail_path(media_path)
}

fn library_thumbnail_path(media_path: &Path) -> PathBuf {
    timeline_thumbnail_path(media_path, LIBRARY_THUMBNAIL_POSITION_SECONDS)
}

fn existing_remote_thumbnail_path(thumbnail_url: &str) -> Option<PathBuf> {
    let thumbnail_path = remote_thumbnail_path(thumbnail_url);

    thumbnail_path.is_file().then_some(thumbnail_path)
}

fn remote_thumbnail_path(thumbnail_url: &str) -> PathBuf {
    watch_session_directory()
        .join(THUMBNAIL_CACHE_DIRECTORY_NAME)
        .join(format!(
            "remote-{:016x}.img",
            stable_hash_bytes(thumbnail_url.as_bytes())
        ))
}

fn cache_remote_thumbnail(thumbnail_url: &str) -> Option<PathBuf> {
    if !is_http_url(thumbnail_url) {
        return None;
    }

    let thumbnail_path = remote_thumbnail_path(thumbnail_url);
    if thumbnail_path.is_file() {
        return Some(thumbnail_path);
    }

    if let Some(parent_directory) = thumbnail_path.parent() {
        let _ = fs::create_dir_all(parent_directory);
    }

    let response = ureq::get(thumbnail_url)
        .set("User-Agent", REMOTE_THUMBNAIL_USER_AGENT)
        .set(
            "Accept",
            "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8",
        )
        .timeout(Duration::from_secs(
            REMOTE_THUMBNAIL_DOWNLOAD_TIMEOUT_SECONDS,
        ))
        .call()
        .ok()?;
    let mut response_reader = response.into_reader().take(10 * 1024 * 1024);
    let mut thumbnail_bytes = Vec::new();
    response_reader.read_to_end(&mut thumbnail_bytes).ok()?;
    if thumbnail_bytes.is_empty() {
        return None;
    }

    let temporary_path = thumbnail_path.with_extension("tmp");
    fs::write(&temporary_path, thumbnail_bytes).ok()?;
    match fs::rename(&temporary_path, &thumbnail_path) {
        Ok(()) => Some(thumbnail_path),
        Err(_) => {
            let _ = fs::remove_file(&temporary_path);
            None
        }
    }
}

fn first_existing_timeline_thumbnail_path(media_path: &Path) -> Option<PathBuf> {
    let thumbnail_directory = watch_session_directory().join(THUMBNAIL_CACHE_DIRECTORY_NAME);
    let media_hash_prefix = format!("{}-", media_cache_key(media_path));
    let Ok(entries) = fs::read_dir(thumbnail_directory) else {
        return None;
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|thumbnail_path| {
            thumbnail_path
                .file_name()
                .and_then(OsStr::to_str)
                .map(|file_name| {
                    file_name.starts_with(&media_hash_prefix) && file_name.ends_with(".jpg")
                })
                .unwrap_or(false)
        })
}

fn timeline_thumbnail_path(media_path: &Path, position_seconds: f64) -> PathBuf {
    let media_key = media_cache_key(media_path);
    let thumbnail_second = thumbnail_second_for_position(position_seconds);

    watch_session_directory()
        .join(THUMBNAIL_CACHE_DIRECTORY_NAME)
        .join(format!("{media_key}-{thumbnail_second}.jpg"))
}

fn stable_hash_bytes(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    hash
}

fn media_cache_key(media_path: &Path) -> String {
    let path_key = library_media_path_key(media_path);
    let metadata = fs::metadata(media_path).ok();
    let file_len = metadata
        .as_ref()
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    let modified_millis = metadata
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let full_key = format!("{path_key}|{file_len}|{modified_millis}");

    format!("{:016x}", stable_hash_bytes(full_key.as_bytes()))
}

fn stable_ui_id(prefix: &str, value: &str) -> String {
    format!("{prefix}-{:016x}", stable_hash_bytes(value.as_bytes()))
}

fn stable_path_ui_id(prefix: &str, path: &Path) -> String {
    let key = library_media_path_key(path);
    stable_ui_id(prefix, &key)
}

fn prune_thumbnail_cache() {
    let cache_dir = watch_session_directory().join(THUMBNAIL_CACHE_DIRECTORY_NAME);
    let Ok(entries) = fs::read_dir(&cache_dir) else {
        return;
    };

    let mut files = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            let modified = metadata.modified().ok()?;
            Some((path, metadata.len(), modified))
        })
        .collect::<Vec<_>>();
    let mut total_size = files.iter().map(|(_, size, _)| *size).sum::<u64>();

    if total_size <= THUMBNAIL_CACHE_MAX_BYTES {
        return;
    }

    files.sort_by_key(|(_, _, modified)| *modified);
    for (path, size, _) in files {
        if total_size <= THUMBNAIL_CACHE_MAX_BYTES {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total_size = total_size.saturating_sub(size);
        }
    }
}

fn generate_library_thumbnails_bounded(ffmpeg_path: PathBuf, media_paths: Vec<PathBuf>) {
    let queue = Arc::new(Mutex::new(VecDeque::from(media_paths)));
    let workers = (0..THUMBNAIL_WORKER_COUNT)
        .map(|_| {
            let queue = Arc::clone(&queue);
            let ffmpeg_path = ffmpeg_path.clone();

            std::thread::spawn(move || loop {
                let media_path = {
                    let Ok(mut queue) = queue.lock() else {
                        return;
                    };
                    queue.pop_front()
                };

                let Some(media_path) = media_path else {
                    break;
                };
                let _ = generate_timeline_thumbnail(
                    &ffmpeg_path,
                    &media_path,
                    LIBRARY_THUMBNAIL_POSITION_SECONDS,
                );
            })
        })
        .collect::<Vec<_>>();

    for worker in workers {
        let _ = worker.join();
    }
}

fn thumbnail_second_for_position(position_seconds: f64) -> u64 {
    let rounded_second = position_seconds.max(0.0).round() as u64;

    if TIMELINE_THUMBNAIL_INTERVAL_SECONDS == 0 {
        rounded_second
    } else {
        (rounded_second / TIMELINE_THUMBNAIL_INTERVAL_SECONDS) * TIMELINE_THUMBNAIL_INTERVAL_SECONDS
    }
}

fn searchable_media_title(media_path: &Path) -> String {
    media_path
        .file_stem()
        .and_then(OsStr::to_str)
        .map(|stem| stem.replace(['.', '_', '-'], " "))
        .unwrap_or_else(|| display_name(media_path))
}

fn url_encode(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() {
            encoded.push(byte as char);
        } else if byte == b' ' {
            encoded.push('+');
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded
}

fn watch_session_directory() -> PathBuf {
    if let Some(app_data_directory) = std::env::var_os("APPDATA") {
        return PathBuf::from(app_data_directory).join(WATCH_SESSION_DIRECTORY_NAME);
    }

    if let Some(config_directory) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config_directory).join(WATCH_SESSION_DIRECTORY_NAME);
    }

    if let Some(home_directory) = std::env::var_os("HOME") {
        return PathBuf::from(home_directory)
            .join(".config")
            .join(WATCH_SESSION_DIRECTORY_NAME);
    }

    std::env::current_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join(WATCH_SESSION_DIRECTORY_NAME)
}

fn detect_dependency_status() -> DependencyStatus {
    DependencyStatus {
        mpv_path: locate_dependency("mpv"),
        ffprobe_path: locate_dependency("ffprobe"),
        ffmpeg_path: locate_dependency("ffmpeg"),
    }
}

fn platform_support_message() -> Option<&'static str> {
    if cfg!(target_os = "windows") {
        None
    } else {
        Some("Embedded playback is currently Windows-only.")
    }
}

fn locate_dependency(binary_name: &str) -> Option<PathBuf> {
    bundled_dependency_candidates(binary_name)
        .into_iter()
        .chain(path_dependency_candidates(binary_name))
        .find(|candidate_path| candidate_path.is_file())
}

fn bundled_dependency_candidates(binary_name: &str) -> Vec<PathBuf> {
    let executable_name = platform_executable_name(binary_name);
    let mut base_directories = Vec::new();

    if let Ok(current_executable_path) = std::env::current_exe() {
        if let Some(executable_directory) = current_executable_path.parent() {
            base_directories.push(executable_directory.to_path_buf());
        }
    }
    base_directories.push(Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf());

    base_directories
        .into_iter()
        .flat_map(|base_directory| {
            [
                base_directory.join(&executable_name),
                base_directory
                    .join("tools")
                    .join(binary_name)
                    .join(&executable_name),
                base_directory
                    .join("tools")
                    .join("ffmpeg")
                    .join("bin")
                    .join(&executable_name),
                base_directory.join("bin").join(&executable_name),
            ]
        })
        .collect()
}

fn path_dependency_candidates(binary_name: &str) -> Vec<PathBuf> {
    let executable_name = platform_executable_name(binary_name);
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path_value| std::env::split_paths(&path_value).collect::<Vec<_>>())
        .map(|path_directory| path_directory.join(&executable_name))
        .collect()
}

fn platform_executable_name(binary_name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{binary_name}.exe")
    } else {
        binary_name.to_string()
    }
}

fn discover_sidecar_subtitles(media_path: &Path) -> Vec<PathBuf> {
    let Some(parent_directory) = media_path.parent() else {
        return Vec::new();
    };
    let media_stem = media_path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    let Ok(entries) = std::fs::read_dir(parent_directory) else {
        return Vec::new();
    };

    let mut subtitle_paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_subtitle_path(path))
        .filter(|path| {
            path.file_stem()
                .and_then(OsStr::to_str)
                .map(|stem| stem.to_ascii_lowercase().starts_with(&media_stem))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    subtitle_paths.sort();
    subtitle_paths
}

#[cfg(target_os = "windows")]
fn hide_child_process_window(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW_FLAG);
}

#[cfg(not(target_os = "windows"))]
fn hide_child_process_window(_command: &mut Command) {}

fn embedded_subtitle_detail(subtitle_track: &EmbeddedSubtitleTrack) -> String {
    let mut details = Vec::new();

    if let Some(language) = subtitle_track.language.as_ref() {
        details.push(language.clone());
    }
    if let Some(codec) = subtitle_track.codec.as_ref() {
        details.push(codec.clone());
    }
    details.push(format!("track {}", subtitle_track.track_id));

    details.join(" / ")
}

fn audio_track_detail(audio_track: &AudioTrack) -> String {
    let mut details = Vec::new();

    if let Some(language) = audio_track.language.as_ref() {
        details.push(language.clone());
    }
    if let Some(codec) = audio_track.codec.as_ref() {
        details.push(codec.clone());
    }
    details.push(format!("track {}", audio_track.track_id));

    details.join(" / ")
}

fn clamp_menu_axis(value: f32, min_value: f32, max_value: f32) -> f32 {
    if max_value < min_value {
        min_value
    } else {
        value.clamp(min_value, max_value)
    }
}

fn timeline_fraction_from_window_position(window: &Window, window_position: Point<Pixels>) -> f64 {
    let (timeline_left_px, timeline_width_px) = timeline_geometry_from_window(window);

    if timeline_width_px <= 0.0 {
        return 0.0;
    }

    ((window_position.x.as_f32() - timeline_left_px) / timeline_width_px).clamp(0.0, 1.0) as f64
}

fn timeline_width_from_window(window: &Window) -> f32 {
    timeline_geometry_from_window(window).1
}

fn timeline_geometry_from_window(window: &Window) -> (f32, f32) {
    let viewport_width_px = window.viewport_size().width.as_f32();
    let timeline_left_px =
        TIMELINE_HORIZONTAL_PADDING_PX + TIMELINE_TIME_LABEL_WIDTH_PX + TIMELINE_LABEL_GAP_PX;
    let timeline_right_padding_px =
        TIMELINE_HORIZONTAL_PADDING_PX + TIMELINE_TIME_LABEL_WIDTH_PX + TIMELINE_LABEL_GAP_PX;
    let timeline_width_px =
        (viewport_width_px - timeline_left_px - timeline_right_padding_px).max(1.0);

    (timeline_left_px, timeline_width_px)
}

fn is_position_inside_video_double_click_region(
    viewport_height: Pixels,
    window_position: Point<Pixels>,
) -> bool {
    let local_y_px = window_position.y.as_f32();
    let safe_bottom_px = (viewport_height.as_f32() - VIDEO_DOUBLE_CLICK_BOTTOM_GUARD_PX)
        .max(VIDEO_DOUBLE_CLICK_TOP_GUARD_PX);

    local_y_px >= VIDEO_DOUBLE_CLICK_TOP_GUARD_PX && local_y_px <= safe_bottom_px
}

fn rgb_alpha(hex: u32, alpha: f32) -> gpui::Rgba {
    let mut color = rgb(hex);
    color.a = alpha;
    color
}

fn icon_path(icon_file_name: &str) -> SharedString {
    let installed_icon_path = std::env::current_exe()
        .ok()
        .and_then(|executable_path| executable_path.parent().map(Path::to_path_buf))
        .map(|executable_directory| executable_directory.join("icons").join(icon_file_name));

    if let Some(installed_icon_path) = installed_icon_path {
        if installed_icon_path.is_file() {
            return installed_icon_path.display().to_string().into();
        }
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("icons")
        .join(icon_file_name)
        .display()
        .to_string()
        .into()
}

fn initial_window_bounds(settings: &PlayerSettings, cx: &mut App) -> Bounds<Pixels> {
    if let Some(saved_bounds) = settings.window_bounds.as_ref() {
        return Bounds {
            origin: point(
                px(saved_bounds.x.unwrap_or(80.0)),
                px(saved_bounds.y.unwrap_or(80.0)),
            ),
            size: size(
                px(saved_bounds.width.max(640.0)),
                px(saved_bounds.height.max(360.0)),
            ),
        };
    }

    Bounds::centered(None, size(px(1500.0), px(920.0)), cx)
}

fn run_application() {
    let startup_options = startup_options_from_args();
    let initial_media_paths = startup_options.media_paths;
    let mut initial_player_settings = load_player_settings();

    if let Some(start_fullscreen) = startup_options.start_fullscreen {
        initial_player_settings.start_fullscreen = start_fullscreen;
    }
    if let Some(resume_behavior) = startup_options.resume_behavior {
        initial_player_settings.resume_behavior = resume_behavior;
    }

    application().run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("space", TogglePlayback, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("k", TogglePlayback, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("m", ToggleMute, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("f", ToggleFullscreen, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("n", NextQueueItem, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("p", PreviousQueueItem, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new(".", FrameStepForward, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new(",", FrameStepBackward, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("shift-a", SetABLoopA, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("shift-b", SetABLoopB, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("shift-c", ClearABLoop, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new(
                "shift-right",
                SeekNextChapter,
                Some(WATCH_PLAYER_KEY_CONTEXT),
            ),
            KeyBinding::new(
                "shift-left",
                SeekPreviousChapter,
                Some(WATCH_PLAYER_KEY_CONTEXT),
            ),
            KeyBinding::new("0", JumpToPercent0, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("1", JumpToPercent1, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("2", JumpToPercent2, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("3", JumpToPercent3, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("4", JumpToPercent4, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("5", JumpToPercent5, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("6", JumpToPercent6, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("7", JumpToPercent7, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("8", JumpToPercent8, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("9", JumpToPercent9, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("left", SeekBackward, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("right", SeekForward, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("j", SeekBackward, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("l", SeekForward, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("up", VolumeUp, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("down", VolumeDown, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("h", IncreaseSubtitleDelay, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("g", DecreaseSubtitleDelay, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new(
                "shift-g",
                ResetSubtitleDelay,
                Some(WATCH_PLAYER_KEY_CONTEXT),
            ),
            KeyBinding::new("=", IncreasePlaybackSpeed, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("-", DecreasePlaybackSpeed, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("s", ToggleShuffle, Some(WATCH_PLAYER_KEY_CONTEXT)),
            KeyBinding::new("r", CycleRepeatMode, Some(WATCH_PLAYER_KEY_CONTEXT)),
        ]);
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("delete", Delete, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("left", Left, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("right", Right, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new(
                "enter",
                SubmitSourceSearch,
                Some(SOURCE_TEXT_INPUT_KEY_CONTEXT),
            ),
            KeyBinding::new(
                "return",
                SubmitSourceSearch,
                Some(SOURCE_TEXT_INPUT_KEY_CONTEXT),
            ),
            KeyBinding::new(
                "up",
                SelectPreviousSourceResult,
                Some(SOURCE_TEXT_INPUT_KEY_CONTEXT),
            ),
            KeyBinding::new(
                "down",
                SelectNextSourceResult,
                Some(SOURCE_TEXT_INPUT_KEY_CONTEXT),
            ),
            KeyBinding::new(
                "shift-left",
                SelectLeft,
                Some(SOURCE_TEXT_INPUT_KEY_CONTEXT),
            ),
            KeyBinding::new(
                "shift-right",
                SelectRight,
                Some(SOURCE_TEXT_INPUT_KEY_CONTEXT),
            ),
            KeyBinding::new("cmd-a", SelectAll, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("ctrl-a", SelectAll, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("home", Home, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("end", End, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("cmd-v", Paste, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("ctrl-v", Paste, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("cmd-c", Copy, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("ctrl-c", Copy, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("cmd-x", Cut, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
            KeyBinding::new("ctrl-x", Cut, Some(SOURCE_TEXT_INPUT_KEY_CONTEXT)),
        ]);
        cx.bind_keys([
            KeyBinding::new("enter", SubmitSourceSearch, Some(WATCH_DIALOG_KEY_CONTEXT)),
            KeyBinding::new("return", SubmitSourceSearch, Some(WATCH_DIALOG_KEY_CONTEXT)),
            KeyBinding::new(
                "up",
                SelectPreviousSourceResult,
                Some(WATCH_DIALOG_KEY_CONTEXT),
            ),
            KeyBinding::new(
                "down",
                SelectNextSourceResult,
                Some(WATCH_DIALOG_KEY_CONTEXT),
            ),
        ]);

        let bounds = initial_window_bounds(&initial_player_settings, cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(APP_NAME.into()),
                    ..Default::default()
                }),
                window_background: initial_player_settings.window_background_appearance(),
                app_id: Some(APP_ID.to_string()),
                focus: true,
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title(APP_NAME);
                window.set_app_id(APP_ID);
                let player = cx.new(|cx| WatchPlayer::new(initial_media_paths, window, cx));
                player.focus_handle(cx).focus(window, cx);
                player
            },
        )
        .unwrap();

        cx.activate(true);
    });
}

fn main() {
    run_application();
}
