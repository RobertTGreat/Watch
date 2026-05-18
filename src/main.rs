#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    ffi::OsStr,
    fs::{self, OpenOptions},
    hash::{Hash, Hasher},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use gpui::{
    actions, canvas, div, img, linear_color_stop, linear_gradient, point, prelude::*, px, relative,
    rgb, size, svg, Animation, AnimationExt, AnyElement, App, Bounds, Context, Div, ExternalPaths,
    FocusHandle, Focusable, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ObjectFit, PathPromptOptions, Pixels, Point, Render, ScrollDelta, ScrollWheelEvent,
    SharedString, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowOptions,
};
use gpui_platform::application;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
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
const PLAYER_BLACK: u32 = 0x030303;
const MENU_BLACK: u32 = 0x080808;
const SOFT_WHITE: u32 = 0xf5f5f5;
const MUTED_TEXT: u32 = 0x9a9a9a;
const FINE_BORDER: u32 = 0x242424;
const BRIGHT_BORDER: u32 = 0x6a6a6a;
const VLC_ORANGE: u32 = 0xff8a2a;
const CONTROLS_HIDE_DELAY_MS: u64 = 500;
const PLAYBACK_STATE_POLL_MS: u64 = 250;
const OSD_HIDE_DELAY_MS: u64 = 1200;
const TIMELINE_THUMBNAIL_INTERVAL_SECONDS: u64 = 5;
const TIMELINE_SCRUB_MIN_DELTA_SECONDS: f64 = 0.35;
const TIMELINE_HORIZONTAL_PADDING_PX: f32 = 16.0;
const TIMELINE_TIME_LABEL_WIDTH_PX: f32 = 64.0;
const TIMELINE_LABEL_GAP_PX: f32 = 16.0;
const VIDEO_DOUBLE_CLICK_TOP_GUARD_PX: f32 = 112.0;
const VIDEO_DOUBLE_CLICK_BOTTOM_GUARD_PX: f32 = 154.0;
const LIBRARY_THUMBNAIL_POSITION_SECONDS: f64 = 45.0;
const MAX_LIBRARY_THUMBNAILS_TO_GENERATE: usize = 12;
const LIBRARY_TITLE_LINE_HEIGHT_PX: f32 = 18.0;
const LIBRARY_TITLE_AVERAGE_CHARACTER_WIDTH_PX: f32 = 7.2;
const LIBRARY_TITLE_SCROLL_END_PADDING_PX: f32 = 24.0;
const VOLUME_SLIDER_SEGMENT_COUNT: usize = 101;
const VOLUME_SLIDER_WIDTH: f32 = 142.0;
const MAIN_MENU_WIDTH: f32 = 320.0;
const CONTEXT_MENU_WIDTH: f32 = 300.0;
const LIBRARY_CONTEXT_MENU_WIDTH: f32 = 220.0;
const LIBRARY_CONTEXT_MENU_ESTIMATED_HEIGHT: f32 = 48.0;
const SETTINGS_MODAL_WIDTH: f32 = 480.0;
const MENU_RIGHT_MARGIN: f32 = 16.0;
const MAIN_MENU_CLICKOFF_SAFE_COLUMN_WIDTH: f32 = MAIN_MENU_WIDTH + (MENU_RIGHT_MARGIN * 2.0);
const CONTEXT_MENU_OFFSET: f32 = 8.0;
const CONTEXT_MENU_ESTIMATED_HEIGHT: f32 = 390.0;
const QUEUE_LIST_MAX_HEIGHT: f32 = 260.0;
const CONTINUE_WATCHING_PROMPT_WIDTH: f32 = 340.0;
const MINIMUM_RESUME_POSITION_SECONDS: f64 = 1.0;
const COMPLETED_MEDIA_REMAINING_SECONDS: f64 = 90.0;
const COMPLETED_MEDIA_FRACTION: f64 = 0.92;
const SEEK_STEP_SECONDS: f64 = 5.0;
const VOLUME_STEP_PERCENT: i16 = 5;
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
const ICON_CHEVRON_LEFT: &str = "chevron-left.svg";
const ICON_CHEVRON_RIGHT: &str = "chevron-right.svg";
const ICON_CHEVRON_UP: &str = "chevron-up.svg";
const ICON_CHEVRON_DOWN: &str = "chevron-down.svg";
const ICON_EYE: &str = "eye.svg";
const ICON_X: &str = "x.svg";

const VIDEO_EXTENSIONS: [&str; 20] = [
    "mkv", "mp4", "mov", "m4v", "3gp", "avi", "wmv", "asf", "ogm", "ogg", "flv", "webm", "mxf",
    "mpeg", "mpg", "m2ts", "ts", "vob", "divx", "dv",
];
const SUBTITLE_EXTENSIONS: [&str; 13] = [
    "srt", "ass", "ssa", "sub", "idx", "vtt", "smi", "sami", "txt", "usf", "mpl", "mpsub", "jss",
];

actions!(
    oled_vlc_player,
    [
        TogglePlayback,
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

#[derive(Clone)]
struct LoadedMedia {
    path: PathBuf,
    duration_seconds: Option<f64>,
    audio_tracks: Vec<AudioTrack>,
    subtitle_paths: Vec<PathBuf>,
    embedded_subtitle_tracks: Vec<EmbeddedSubtitleTrack>,
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

#[derive(Clone, Copy, PartialEq, Eq)]
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

    fn next(self) -> Self {
        match self {
            Self::Ask => Self::Always,
            Self::Always => Self::Never,
            Self::Never => Self::Ask,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Always => "always",
            Self::Never => "never",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "always" => Self::Always,
            "never" => Self::Never,
            _ => Self::Ask,
        }
    }
}

#[derive(Clone)]
struct PlayerSettings {
    default_volume_percent: u8,
    resume_behavior: ResumeBehavior,
    preferred_audio_language: String,
    preferred_subtitle_language: String,
    prefer_embedded_subtitles: bool,
    hardware_decoding_mode: String,
    start_fullscreen: bool,
    subtitle_font_size: u8,
    subtitle_color: String,
    subtitle_position_percent: u8,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            default_volume_percent: DEFAULT_VOLUME_PERCENT,
            resume_behavior: ResumeBehavior::Ask,
            preferred_audio_language: "eng".to_string(),
            preferred_subtitle_language: "eng".to_string(),
            prefer_embedded_subtitles: true,
            hardware_decoding_mode: "auto-safe".to_string(),
            start_fullscreen: false,
            subtitle_font_size: 48,
            subtitle_color: "#FFFFFF".to_string(),
            subtitle_position_percent: 95,
        }
    }
}

#[derive(Clone)]
struct MediaHistoryEntry {
    path: PathBuf,
    playback_position_seconds: f64,
    duration_seconds: Option<f64>,
    is_completed: bool,
    updated_at_millis: u128,
}

#[derive(Clone, Default)]
struct PlayerLibrary {
    recent_media_paths: Vec<PathBuf>,
    recent_folder_paths: Vec<PathBuf>,
    media_history: Vec<MediaHistoryEntry>,
}

#[derive(Clone)]
struct LibraryGridItem {
    path: PathBuf,
    title: String,
    subtitle: Option<String>,
    episode_badge: Option<String>,
    thumbnail_media_path: Option<PathBuf>,
    resume_history_entry: Option<MediaHistoryEntry>,
    is_watched: bool,
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

#[derive(Clone)]
struct SavedWatchSession {
    media_paths: Vec<PathBuf>,
    current_media_path: PathBuf,
    current_queue_index: usize,
    playback_position_seconds: f64,
    volume_percent: u8,
    is_muted: bool,
}

#[cfg(target_os = "windows")]
struct EmbeddedVideoHost {
    window_id: isize,
    parent_window_id: isize,
}

#[cfg(not(target_os = "windows"))]
struct EmbeddedVideoHost;

struct OledVlcPlayer {
    focus_handle: FocusHandle,
    playback_queue: Vec<LoadedMedia>,
    current_queue_index: Option<usize>,
    selected_audio_track_id: Option<i64>,
    selected_subtitle_path: Option<PathBuf>,
    selected_embedded_subtitle_track_id: Option<i64>,
    is_playing: bool,
    is_eof_reached: bool,
    playback_position_seconds: f64,
    playback_progress_generation: u64,
    playback_speed: f64,
    volume_percent: u8,
    is_muted: bool,
    subtitle_delay_ms: i32,
    is_shuffle_enabled: bool,
    repeat_mode: PlaybackRepeatMode,
    are_controls_visible: bool,
    controls_visibility_generation: u64,
    osd_message: Option<SharedString>,
    osd_generation: u64,
    is_main_menu_open: bool,
    is_subtitle_menu_open: bool,
    is_settings_modal_open: bool,
    is_library_open: bool,
    subtitle_menu_anchor: Option<Point<Pixels>>,
    library_context_menu_anchor: Option<Point<Pixels>>,
    library_context_menu_media_path: Option<PathBuf>,
    open_context_menu_section: Option<ContextMenuSection>,
    pending_watch_session: Option<SavedWatchSession>,
    settings: PlayerSettings,
    library: PlayerLibrary,
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
}

impl OledVlcPlayer {
    fn new(initial_media_paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = load_player_settings();
        let dependency_status = detect_dependency_status();
        let pending_watch_session = match settings.resume_behavior {
            ResumeBehavior::Ask | ResumeBehavior::Always => load_saved_watch_session(),
            ResumeBehavior::Never => {
                clear_saved_watch_session();
                None
            }
        };
        let status_message = dependency_status.setup_message().unwrap_or_else(|| {
            "Open a real media file or folder to populate the player.".to_string()
        });
        let mut player = Self {
            focus_handle: cx.focus_handle(),
            playback_queue: Vec::new(),
            current_queue_index: None,
            selected_audio_track_id: None,
            selected_subtitle_path: None,
            selected_embedded_subtitle_track_id: None,
            is_playing: false,
            is_eof_reached: false,
            playback_position_seconds: 0.0,
            playback_progress_generation: 0,
            playback_speed: 1.0,
            volume_percent: settings.default_volume_percent.min(100),
            is_muted: false,
            subtitle_delay_ms: 0,
            is_shuffle_enabled: false,
            repeat_mode: PlaybackRepeatMode::Off,
            are_controls_visible: true,
            controls_visibility_generation: 0,
            osd_message: None,
            osd_generation: 0,
            is_main_menu_open: false,
            is_subtitle_menu_open: false,
            is_settings_modal_open: false,
            is_library_open: initial_media_paths.is_empty(),
            subtitle_menu_anchor: None,
            library_context_menu_anchor: None,
            library_context_menu_media_path: None,
            open_context_menu_section: None,
            pending_watch_session,
            settings,
            library: load_player_library(),
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
        player.schedule_controls_hide(window, cx);
        player
    }

    fn current_media(&self) -> Option<&LoadedMedia> {
        self.current_queue_index
            .and_then(|queue_index| self.playback_queue.get(queue_index))
    }

    fn current_media_title(&self) -> String {
        self.current_media()
            .map(|media| display_name(&media.path))
            .unwrap_or_else(|| "No media loaded".to_string())
    }

    fn current_media_detail(&self) -> String {
        self.current_media()
            .map(|media| media.path.display().to_string())
            .unwrap_or_else(|| "Use Menu > Play file, Play folder, or Play queue.".to_string())
    }

    fn current_media_path(&self) -> Option<PathBuf> {
        self.current_media().map(|media| media.path.clone())
    }

    fn current_media_duration_seconds(&self) -> Option<f64> {
        self.current_media()
            .and_then(|media| media.duration_seconds)
            .filter(|duration_seconds| duration_seconds.is_finite() && *duration_seconds > 0.0)
    }

    fn current_watch_session(&self) -> Option<SavedWatchSession> {
        let current_queue_index = self.current_queue_index?;
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
            .map(|media| media.path.clone())
            .collect::<Vec<_>>();

        if media_paths.is_empty() {
            return None;
        }

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

    fn record_current_media_progress(&mut self) {
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

        self.library
            .media_history
            .retain(|entry| entry.path != current_media_path);
        self.library.media_history.insert(
            0,
            MediaHistoryEntry {
                path: current_media_path.clone(),
                playback_position_seconds,
                duration_seconds,
                is_completed,
                updated_at_millis,
            },
        );
        self.library.media_history.truncate(MAX_RECENT_MEDIA * 2);
        promote_recent_path(
            &mut self.library.recent_media_paths,
            current_media_path,
            MAX_RECENT_MEDIA,
        );
        save_player_library(&self.library);
    }

    fn remember_current_queue_in_library(&mut self) {
        let media_paths = self
            .playback_queue
            .iter()
            .map(|media| media.path.clone())
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

        save_player_library(&self.library);
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

    fn set_playback_position_seconds(&mut self, position_seconds: f64) {
        let max_position_seconds = self.current_media_duration_seconds().unwrap_or(0.0);
        self.playback_position_seconds = position_seconds.clamp(0.0, max_position_seconds);
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
                } else if player.playback_process_has_exited() {
                    player.is_playing = false;
                    player.playback_progress_generation += 1;
                    player.playback_process = None;
                    player.playback_ipc_path = None;
                    player.status_message = Some("Playback backend exited.".into());
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
        false
    }

    fn playback_process_has_exited(&mut self) -> bool {
        self.playback_process
            .as_mut()
            .and_then(|process| process.try_wait().ok())
            .flatten()
            .is_some()
    }

    fn handle_end_of_current_media(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_playing = false;
        self.record_current_media_progress();

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
        self.are_controls_visible = true;
        self.controls_visibility_generation += 1;
        self.schedule_controls_hide(window, cx);
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
                {
                    player.are_controls_visible = false;
                    cx.notify();
                }
            });
        })
        .detach();
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
        if self.current_media_path().is_some() {
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
            self.status_message = Some("Load a real media file before playback.".into());
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

    fn seek_to_timeline_fraction(
        &mut self,
        timeline_fraction: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.apply_timeline_seek(timeline_fraction, true, true, window, cx);
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
        let last_backend_seek_position = self
            .last_timeline_seek_position_seconds
            .unwrap_or(self.playback_position_seconds);
        let should_send_backend_seek = should_force_backend_seek
            || (position_seconds - last_backend_seek_position).abs()
                >= TIMELINE_SCRUB_MIN_DELTA_SECONDS;

        self.set_playback_position_seconds(position_seconds);
        self.is_eof_reached = false;

        if should_send_backend_seek {
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
        if self.current_media_path().is_none() {
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
        self.seek_relative_seconds(-SEEK_STEP_SECONDS, window, cx);
    }

    fn seek_forward(&mut self, _: &SeekForward, window: &mut Window, cx: &mut Context<Self>) {
        self.seek_relative_seconds(SEEK_STEP_SECONDS, window, cx);
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
        window.toggle_fullscreen();
        self.reveal_controls(window, cx);
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

    fn change_volume_by(&mut self, volume_delta: i16, window: &mut Window, cx: &mut Context<Self>) {
        let current_volume = i16::from(self.volume_percent);
        let next_volume = (current_volume + volume_delta).clamp(0, 100) as u8;
        self.set_volume_percent(next_volume, window, cx);
    }

    fn increase_volume(&mut self, _: &VolumeUp, window: &mut Window, cx: &mut Context<Self>) {
        self.change_volume_by(VOLUME_STEP_PERCENT, window, cx);
    }

    fn decrease_volume(&mut self, _: &VolumeDown, window: &mut Window, cx: &mut Context<Self>) {
        self.change_volume_by(-VOLUME_STEP_PERCENT, window, cx);
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
        self.is_settings_modal_open = false;
        self.subtitle_menu_anchor = None;
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
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
        self.is_settings_modal_open = false;
        self.subtitle_menu_anchor = Some(anchor);
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.open_context_menu_section = None;
        self.reveal_controls(window, cx);
    }

    fn close_open_menus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.is_settings_modal_open = false;
        self.subtitle_menu_anchor = None;
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
        self.open_context_menu_section = None;
        self.reveal_controls(window, cx);
    }

    fn open_settings_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_settings_modal_open = true;
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.subtitle_menu_anchor = None;
        self.open_context_menu_section = None;
        self.reveal_controls(window, cx);
    }

    fn close_settings_modal(&mut self, cx: &mut Context<Self>) {
        self.is_settings_modal_open = false;
        cx.notify();
    }

    fn show_library_context_menu(
        &mut self,
        media_path: PathBuf,
        anchor: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.library_context_menu_anchor = Some(anchor);
        self.library_context_menu_media_path = Some(media_path);
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.is_settings_modal_open = false;
        self.subtitle_menu_anchor = None;
        self.open_context_menu_section = None;
        cx.notify();
    }

    fn close_library_context_menu(&mut self, cx: &mut Context<Self>) {
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
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
        self.timeline_hover_preview = None;
        self.schedule_library_thumbnail_generation(window, cx);
        cx.notify();
    }

    fn reveal_library_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.playback_progress_generation += 1;
        self.record_current_media_progress();
        self.save_current_watch_session();
        self.stop_playback_process();
        self.is_playing = false;
        self.is_eof_reached = false;
        self.is_library_open = true;
        self.are_controls_visible = false;
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.is_settings_modal_open = false;
        self.library_context_menu_anchor = None;
        self.library_context_menu_media_path = None;
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

        cx.spawn_in(window, async move |this, cx| {
            for media_path in media_paths {
                let _ = generate_timeline_thumbnail(
                    &ffmpeg_path,
                    &media_path,
                    LIBRARY_THUMBNAIL_POSITION_SECONDS,
                );
            }

            let _ = this.update_in(cx, |_player, _window, cx| {
                cx.notify();
            });
        })
        .detach();
    }

    fn open_file_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
        self.open_context_menu_section = None;
        self.reveal_controls(window, cx);
        let Some(media_path) = video_file_dialog("Play file").pick_file() else {
            return;
        };

        if is_video_path(&media_path) {
            self.load_media_paths(vec![media_path], window, cx);
        }
        self.reveal_controls(window, cx);
    }

    fn open_folder_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_main_menu_open = false;
        self.is_subtitle_menu_open = false;
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
        self.playback_queue = media_paths
            .into_iter()
            .map(|path| build_loaded_media(&path))
            .collect();
        self.current_queue_index = Some(0);
        self.select_initial_tracks_for_queue_index(0);
        self.is_library_open = false;
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
        self.playback_queue = media_paths
            .into_iter()
            .map(|path| build_loaded_media(&path))
            .collect();

        let fallback_queue_index = watch_session
            .current_queue_index
            .min(self.playback_queue.len().saturating_sub(1));
        let resume_queue_index = self
            .playback_queue
            .iter()
            .position(|media| media.path == watch_session.current_media_path)
            .unwrap_or(fallback_queue_index);

        self.current_queue_index = Some(resume_queue_index);
        self.volume_percent = watch_session.volume_percent.min(100);
        self.is_muted = watch_session.is_muted;
        self.select_initial_tracks_for_queue_index(resume_queue_index);
        self.is_library_open = false;
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

    fn next_queue_index_for_playback(&self, is_from_eof: bool) -> Option<usize> {
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
            return Some(shuffled_queue_index(current_queue_index, queue_len));
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
            self.current_queue_index = Some(queue_index);
            self.select_initial_tracks_for_queue_index(queue_index);
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

        let Some(media_path) = self.current_media_path() else {
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

        let clamped_start_position_seconds = self
            .current_media_duration_seconds()
            .map(|duration_seconds| start_position_seconds.clamp(0.0, duration_seconds))
            .unwrap_or_else(|| start_position_seconds.max(0.0));
        self.playback_position_seconds = clamped_start_position_seconds;
        self.is_eof_reached = false;

        let Some(video_host_window_id) = self.ensure_video_host_window(window) else {
            self.is_playing = false;
            self.status_message =
                Some("Could not create the embedded video surface for this window.".into());
            return;
        };

        let ipc_path = create_playback_ipc_path();
        self.position_video_host_for_current_window(window, true);

        let mut command = Command::new(mpv_path);
        command
            .arg(format!("--wid={video_host_window_id}"))
            .arg(format!("--input-ipc-server={ipc_path}"))
            .arg("--force-window=yes")
            .arg("--no-border")
            .arg("--no-osc")
            .arg("--keep-open=yes")
            .arg("--no-terminal")
            .arg("--really-quiet")
            .arg(format!("--hwdec={}", self.settings.hardware_decoding_mode))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        hide_child_process_window(&mut command);

        if clamped_start_position_seconds > 0.0 {
            command.arg(format!("--start={clamped_start_position_seconds:.3}"));
        }
        command.arg(media_path.as_os_str());

        match command.spawn() {
            Ok(child) => {
                self.playback_process = Some(child);
                self.playback_ipc_path = Some(ipc_path);
                self.is_playing = true;
                self.status_message = None;
                if !self.send_mpv_command(json!(["set_property", "volume", self.volume_percent])) {
                    self.status_message =
                        Some("Playback started, but mpv controls are not connected yet.".into());
                }
                self.send_mpv_command(json!(["set_property", "mute", self.is_muted]));
                if let Some(audio_track_id) = self.selected_audio_track_id {
                    self.send_mpv_command(json!(["set_property", "aid", audio_track_id]));
                }
                self.load_selected_subtitle_in_backend();
                self.apply_subtitle_style_in_backend();
                self.send_mpv_command(json!([
                    "set_property",
                    "sub-delay",
                    self.subtitle_delay_ms as f64 / 1000.0
                ]));
                self.send_mpv_command(json!(["set_property", "speed", self.playback_speed]));
                if clamped_start_position_seconds > 0.0 {
                    self.send_mpv_command(json!([
                        "seek",
                        clamped_start_position_seconds,
                        "absolute+exact"
                    ]));
                }
                self.schedule_playback_state_poll(window, cx);
            }
            Err(error) => {
                self.playback_process = None;
                self.playback_ipc_path = None;
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

    fn load_library_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.load_media_paths(media_paths_from_path(path), window, cx);
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
            .map(media_paths_in_folder)
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

    fn set_default_volume_percent(&mut self, default_volume_percent: u8, cx: &mut Context<Self>) {
        self.settings.default_volume_percent = default_volume_percent.min(100);
        save_player_settings(&self.settings);
        self.status_message =
            Some(format!("Default volume {}%", self.settings.default_volume_percent).into());
        cx.notify();
    }

    fn cycle_resume_behavior_setting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.resume_behavior = self.settings.resume_behavior.next();
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

    fn cycle_audio_language_setting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.preferred_audio_language =
            next_language_preference(&self.settings.preferred_audio_language);
        self.save_settings_and_show(
            format!("Audio language {}", self.settings.preferred_audio_language),
            window,
            cx,
        );
    }

    fn cycle_subtitle_language_setting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.preferred_subtitle_language =
            next_language_preference(&self.settings.preferred_subtitle_language);
        self.save_settings_and_show(
            format!(
                "Subtitle language {}",
                self.settings.preferred_subtitle_language
            ),
            window,
            cx,
        );
    }

    fn cycle_hardware_decoding_setting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.hardware_decoding_mode = match self.settings.hardware_decoding_mode.as_str() {
            "auto-safe" => "d3d11va".to_string(),
            "d3d11va" => "no".to_string(),
            _ => "auto-safe".to_string(),
        };
        self.save_settings_and_show(
            format!("Hardware decode {}", self.settings.hardware_decoding_mode),
            window,
            cx,
        );
    }

    fn toggle_start_fullscreen_setting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.start_fullscreen = !self.settings.start_fullscreen;
        self.save_settings_and_show(
            if self.settings.start_fullscreen {
                "Start fullscreen on"
            } else {
                "Start fullscreen off"
            }
            .to_string(),
            window,
            cx,
        );
    }

    fn render_video_surface(
        &self,
        is_window_fullscreen: bool,
        viewport_width: f32,
        viewport_height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_video_surface_active = self.is_video_surface_active();
        let should_show_player_overlay = !self.is_library_open;

        div()
            .id("player-surface")
            .relative()
            .overflow_hidden()
            .size_full()
            .bg(if is_video_surface_active {
                rgb_alpha(PLAYER_BLACK, 0.0)
            } else {
                rgb(PLAYER_BLACK)
            })
            .border_1()
            .border_color(rgb(FINE_BORDER))
            .when(!is_video_surface_active, |surface| {
                surface.child(self.render_empty_video_plane(viewport_width, viewport_height, cx))
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
        &self,
        viewport_width: f32,
        viewport_height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.is_library_open || self.current_media().is_none() {
            return self
                .render_library_mode(viewport_width, viewport_height, cx)
                .into_any_element();
        }

        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x010101))
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
        &self,
        viewport_width: f32,
        viewport_height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let content_width = (viewport_width - 64.0).clamp(320.0, 1360.0);
        let shelves = self.library_shelves();
        let has_library_shelves = !shelves.is_empty();

        div()
            .id("library-plane")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .p_8()
            .bg(rgb(0x010101))
            .overflow_y_scroll()
            .scrollbar_width(px(6.0))
            .text_color(rgb(SOFT_WHITE))
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .w(px(content_width))
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .justify_between()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_lg().child("Library"))
                                    .child(div().text_xs().text_color(rgb(MUTED_TEXT)).child(
                                        self.status_message.clone().unwrap_or_else(|| {
                                            "Recent media, quick resume, and series.".into()
                                        }),
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(prompt_action_button(
                                        "library-open-file",
                                        "Open File",
                                        false,
                                        cx,
                                        |player, window, cx| {
                                            player.open_file_picker(window, cx);
                                        },
                                    ))
                                    .child(prompt_action_button(
                                        "library-open-folder",
                                        "Open Folder",
                                        true,
                                        cx,
                                        |player, window, cx| {
                                            player.open_folder_picker(window, cx);
                                        },
                                    )),
                            ),
                    )
                    .children(
                        shelves.into_iter().map(|shelf| {
                            self.render_library_shelf_section(shelf, viewport_width, cx)
                        }),
                    )
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
            .child(self.render_library_settings_button(cx))
    }

    fn render_library_settings_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("library-settings-button")
            .absolute()
            .right(px(28.0))
            .bottom(px(28.0))
            .w(px(48.0))
            .h(px(48.0))
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb_alpha(OLED_BLACK, 0.82))
            .border_1()
            .border_color(rgb(BRIGHT_BORDER))
            .shadow_lg()
            .text_color(rgb(SOFT_WHITE))
            .cursor_pointer()
            .hover(|button| button.bg(rgb_alpha(MENU_BLACK, 0.96)))
            .active(|button| button.opacity(0.78))
            .on_click(cx.listener(|player, _, window, cx| {
                player.open_settings_modal(window, cx);
                cx.stop_propagation();
            }))
            .child(
                svg()
                    .external_path(crate::icon_path(ICON_SETTINGS))
                    .w(px(21.0))
                    .h(px(21.0))
                    .text_color(rgb(SOFT_WHITE)),
            )
            .tooltip(move |_window, cx| {
                cx.new(|_| TooltipText {
                    text: "Settings".into(),
                })
                .into()
            })
    }

    fn library_shelves(&self) -> Vec<LibraryShelf> {
        let watched_media_paths = self.watched_media_paths();
        let series_shelves = series_library_shelves(&self.library, &watched_media_paths);
        let grouped_media_paths = series_shelves
            .iter()
            .flat_map(|shelf| shelf.items.iter().map(|item| item.path.clone()))
            .collect::<HashSet<_>>();

        let mut shelves = Vec::new();
        shelves.push(LibraryShelf {
            key: "continue-watching".to_string(),
            title: "Continue Watching".to_string(),
            subtitle: None,
            empty_message: "No in-progress media yet.",
            items: self.continue_watching_library_items(),
        });
        shelves.push(LibraryShelf {
            key: "recent-media".to_string(),
            title: "Recent Media".to_string(),
            subtitle: None,
            empty_message: "No recent media yet.",
            items: self.recent_media_library_items(&grouped_media_paths, &watched_media_paths),
        });
        shelves.extend(series_shelves);
        shelves
            .into_iter()
            .filter(|shelf| !shelf.items.is_empty())
            .collect()
    }

    fn watched_media_paths(&self) -> HashSet<PathBuf> {
        self.library
            .media_history
            .iter()
            .filter(|entry| entry.is_completed)
            .map(|entry| entry.path.clone())
            .collect()
    }

    fn continue_watching_library_items(&self) -> Vec<LibraryGridItem> {
        self.library
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
        &self,
        grouped_media_paths: &HashSet<PathBuf>,
        watched_media_paths: &HashSet<PathBuf>,
    ) -> Vec<LibraryGridItem> {
        self.library
            .recent_media_paths
            .iter()
            .filter(|path| path.is_file() && is_video_path(path))
            .filter(|path| !grouped_media_paths.contains(*path))
            .take(MAX_LIBRARY_ITEMS_PER_SHELF)
            .map(|path| {
                let mut item = library_item_for_media_path(path, false);
                item.is_watched = watched_media_paths.contains(path);
                item
            })
            .collect()
    }

    fn render_library_shelf_section(
        &self,
        shelf: LibraryShelf,
        viewport_width: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let visible_card_count = library_visible_card_count(viewport_width);
        let card_width = library_card_width(viewport_width);
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
            .gap_3()
            .child(
                div()
                    .id(format!("library-shelf-header-{shelf_key}"))
                    .h(px(34.0))
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .on_click(cx.listener(move |player, _, _window, cx| {
                        player.toggle_library_shelf_collapse(collapse_shelf_key.clone(), cx);
                        cx.stop_propagation();
                    }))
                    .child(
                        svg()
                            .external_path(crate::icon_path(collapse_icon_path))
                            .w(px(16.0))
                            .h(px(16.0))
                            .text_color(rgb(MUTED_TEXT)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .line_height(px(16.0))
                                    .text_color(rgb(MUTED_TEXT))
                                    .child(shelf_title.clone()),
                            )
                            .when_some(shelf_subtitle, |label, subtitle| {
                                label.child(
                                    div()
                                        .text_xs()
                                        .line_height(px(12.0))
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
                        .child(div().flex().gap_3().children(visible_items.into_iter().map(
                            |item| {
                                self.render_library_grid_card(
                                    shelf_title.clone(),
                                    card_width,
                                    item,
                                    cx,
                                )
                            },
                        )))
                        .when(can_page_left, |row| {
                            row.child(self.render_library_shelf_arrow(
                                shelf_key.clone(),
                                item_count,
                                -1,
                                viewport_width,
                                cx,
                            ))
                        })
                        .when(can_page_right, |row| {
                            row.child(self.render_library_shelf_arrow(
                                shelf_key.clone(),
                                item_count,
                                1,
                                viewport_width,
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
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_left_arrow = direction < 0;
        let icon_path = if is_left_arrow {
            ICON_CHEVRON_LEFT
        } else {
            ICON_CHEVRON_RIGHT
        };
        let mut arrow = div()
            .id(format!(
                "library-shelf-arrow-{}-{}",
                shelf_key,
                if is_left_arrow { "left" } else { "right" }
            ))
            .absolute()
            .top(px(34.0))
            .w(px(40.0))
            .h(px(72.0))
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb_alpha(OLED_BLACK, 0.76))
            .text_color(rgb(SOFT_WHITE))
            .cursor_pointer()
            .hover(|button| button.bg(rgb_alpha(OLED_BLACK, 0.92)))
            .on_click(cx.listener(move |player, _, _window, cx| {
                player.shift_library_shelf(
                    shelf_key.clone(),
                    item_count,
                    direction,
                    viewport_width,
                );
                cx.stop_propagation();
                cx.notify();
            }))
            .child(
                svg()
                    .external_path(crate::icon_path(icon_path))
                    .w(px(22.0))
                    .h(px(22.0))
                    .text_color(rgb(SOFT_WHITE)),
            );

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
    ) {
        let visible_card_count = library_visible_card_count(viewport_width);
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

    fn dismiss_continue_watching_entry(&mut self, media_path: PathBuf, cx: &mut Context<Self>) {
        self.library
            .media_history
            .retain(|entry| entry.path != media_path);
        save_player_library(&self.library);
        self.status_message = Some("Removed from Continue Watching.".into());
        cx.notify();
    }

    fn mark_library_media_watched(&mut self, media_path: PathBuf, cx: &mut Context<Self>) {
        let duration_seconds = self
            .current_media()
            .filter(|media| media.path == media_path)
            .and_then(|media| media.duration_seconds)
            .or_else(|| {
                self.library
                    .media_history
                    .iter()
                    .find(|entry| entry.path == media_path)
                    .and_then(|entry| entry.duration_seconds)
            });
        let playback_position_seconds = duration_seconds.unwrap_or(0.0).max(0.0);

        self.library
            .media_history
            .retain(|entry| entry.path != media_path);
        self.library.media_history.insert(
            0,
            MediaHistoryEntry {
                path: media_path.clone(),
                playback_position_seconds,
                duration_seconds,
                is_completed: true,
                updated_at_millis: current_time_millis(),
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
        self.status_message = Some("Marked watched.".into());
        save_player_library(&self.library);
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
        let (menu_left, menu_top) =
            self.library_context_menu_origin(viewport_width, viewport_height);

        div()
            .id("library-context-menu")
            .absolute()
            .left(px(menu_left))
            .top(px(menu_top))
            .w(px(LIBRARY_CONTEXT_MENU_WIDTH))
            .p_2()
            .bg(rgb(MENU_BLACK))
            .border_1()
            .border_color(rgb(BRIGHT_BORDER))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_1()
            .text_color(rgb(SOFT_WHITE))
            .child(simple_menu_action(
                "Mark Watched",
                cx,
                move |player, _window, cx| {
                    player.mark_library_media_watched(media_path.clone(), cx);
                },
            ))
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
        item: LibraryGridItem,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let item_path = item.path.clone();
        let item_path_for_click = item.path.clone();
        let item_path_for_remove = item.path.clone();
        let item_path_for_context_menu = item.path.clone();
        let resume_history_entry = item.resume_history_entry.clone();
        let can_remove_from_continue_watching = item.can_remove_from_continue_watching;
        let is_watched = item.is_watched;
        let episode_badge = item.episode_badge.clone();
        let has_episode_badge = episode_badge.is_some();
        let item_title = item.title.clone();
        let has_item_title = !item_title.is_empty();
        let title_overlay_width = if has_episode_badge {
            (card_width - 66.0).max(1.0)
        } else {
            (card_width - 16.0).max(1.0)
        };
        let thumbnail_path = item
            .thumbnail_media_path
            .as_ref()
            .and_then(|media_path| existing_library_thumbnail_path(media_path));
        let has_thumbnail = thumbnail_path.is_some();
        let placeholder_label = item
            .title
            .chars()
            .find(|character| character.is_ascii_alphanumeric())
            .map(|character| character.to_ascii_uppercase().to_string())
            .unwrap_or_else(|| "W".to_string());

        div()
            .id(format!(
                "library-card-{section_title}-{}",
                item_path.display()
            ))
            .flex()
            .flex_col()
            .gap_2()
            .w(px(card_width))
            .flex_none()
            .min_w_0()
            .cursor_pointer()
            .on_click(cx.listener(move |player, _, window, cx| {
                if let Some(resume_history_entry) = resume_history_entry.clone() {
                    player.continue_library_entry(resume_history_entry, window, cx);
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
                    .bg(rgb(0x101010))
                    .border_1()
                    .border_color(rgb(FINE_BORDER))
                    .hover(|thumbnail| thumbnail.border_color(rgb(BRIGHT_BORDER)))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |player, event: &MouseDownEvent, _window, cx| {
                            player.show_library_context_menu(
                                item_path_for_context_menu.clone(),
                                event.position,
                                cx,
                            );
                            cx.stop_propagation();
                        }),
                    )
                    .when_some(thumbnail_path, |frame, thumbnail_path| {
                        frame.child(
                            img(thumbnail_path)
                                .w_full()
                                .h_full()
                                .object_fit(ObjectFit::Cover),
                        )
                    })
                    .when(item.thumbnail_media_path.is_none(), |frame| {
                        frame
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(MUTED_TEXT))
                            .text_xs()
                            .child("Folder")
                    })
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
                                    .text_lg()
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
                                .px_2()
                                .py_2()
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
                                    ),
                                )),
                        )
                    })
                    .when(can_remove_from_continue_watching, |frame| {
                        frame.child(
                            div()
                                .id(format!(
                                    "library-continue-remove-{}",
                                    item_path_for_remove.display()
                                ))
                                .absolute()
                                .top(px(8.0))
                                .right(px(8.0))
                                .w(px(28.0))
                                .h(px(28.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(rgb_alpha(OLED_BLACK, 0.78))
                                .text_color(rgb(SOFT_WHITE))
                                .cursor_pointer()
                                .hover(|button| button.bg(rgb_alpha(0xbb2222, 0.82)))
                                .on_click(cx.listener(move |player, _, _window, cx| {
                                    player.dismiss_continue_watching_entry(
                                        item_path_for_remove.clone(),
                                        cx,
                                    );
                                    cx.stop_propagation();
                                }))
                                .child(
                                    svg()
                                        .external_path(crate::icon_path(ICON_X))
                                        .w(px(16.0))
                                        .h(px(16.0))
                                        .text_color(rgb(SOFT_WHITE)),
                                ),
                        )
                    })
                    .when_some(episode_badge, |frame, episode_badge| {
                        frame.child(
                            div()
                                .absolute()
                                .right(px(8.0))
                                .bottom(px(8.0))
                                .px_2()
                                .py_1()
                                .text_xs()
                                .line_height(px(14.0))
                                .text_color(rgb(SOFT_WHITE))
                                .bg(rgb_alpha(OLED_BLACK, 0.78))
                                .child(episode_badge),
                        )
                    })
                    .when(is_watched, |frame| {
                        frame.child(
                            div()
                                .id(format!("library-watched-badge-{}", item_path.display()))
                                .absolute()
                                .top(px(8.0))
                                .left(px(8.0))
                                .w(px(30.0))
                                .h(px(30.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(rgb_alpha(OLED_BLACK, 0.88))
                                .border_1()
                                .border_color(rgb_alpha(SOFT_WHITE, 0.46))
                                .text_color(rgb(SOFT_WHITE))
                                .shadow_lg()
                                .child(
                                    svg()
                                        .external_path(crate::icon_path(ICON_EYE))
                                        .w(px(17.0))
                                        .h(px(17.0))
                                        .text_color(rgb(SOFT_WHITE)),
                                ),
                        )
                    }),
            )
            .when_some(item.subtitle, |card, subtitle| {
                card.child(
                    div()
                        .min_w_0()
                        .text_xs()
                        .text_color(rgb(MUTED_TEXT))
                        .truncate()
                        .child(subtitle),
                )
            })
    }

    fn render_top_overlay(&self, is_window_fullscreen: bool, cx: &mut Context<Self>) -> Div {
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
            .child(
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
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
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

    fn render_bottom_controls(&self, cx: &mut Context<Self>) -> Div {
        div()
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
            .child(self.render_progress_bar(cx))
            .child(self.render_transport_controls(cx))
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
                    player.seek_to_timeline_fraction(timeline_fraction, window, cx);
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
            .bg(rgb(MENU_BLACK))
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
            .bg(rgb(MENU_BLACK))
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
                    .justify_end()
                    .gap_2()
                    .px_2()
                    .pb_1()
                    .child(self.render_playback_mode_toggles("main-menu", cx)),
            )
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
            .child(simple_menu_action("Library", cx, |player, window, cx| {
                player.reveal_library_mode(window, cx);
            }))
            .child(simple_menu_action("Settings", cx, |player, window, cx| {
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

        div()
            .absolute()
            .left(px(menu_left))
            .top(px(menu_top))
            .w(px(CONTEXT_MENU_WIDTH))
            .p_2()
            .bg(rgb(MENU_BLACK))
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
                    .gap_2()
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
                    ))
                    .child(div().flex_1())
                    .child(self.render_playback_mode_toggles("context-menu", cx)),
            )
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
            .bg(rgb_alpha(OLED_BLACK, 0.78))
            .child(
                div()
                    .id("continue-watching-dialog")
                    .w(px(CONTINUE_WATCHING_PROMPT_WIDTH))
                    .p_4()
                    .bg(rgb(MENU_BLACK))
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
                                cx,
                                |player, window, cx| {
                                    player.decline_saved_watch_session(window, cx);
                                },
                            ))
                            .child(prompt_action_button(
                                "continue-watching-yes",
                                "Yes",
                                true,
                                cx,
                                |player, window, cx| {
                                    player.continue_saved_watch_session(window, cx);
                                },
                            )),
                    ),
            )
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
            .bg(rgb_alpha(OLED_BLACK, 0.78))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|player, _event, _window, cx| {
                    player.close_settings_modal(cx);
                }),
            )
            .child(
                div()
                    .id("settings-modal")
                    .w(px(SETTINGS_MODAL_WIDTH))
                    .p_4()
                    .bg(rgb(MENU_BLACK))
                    .border_1()
                    .border_color(rgb(BRIGHT_BORDER))
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .text_color(rgb(SOFT_WHITE))
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
                            .gap_2()
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
                            .items_center()
                            .gap_2()
                            .child(context_icon_button(
                                "settings-previous",
                                ICON_PREVIOUS,
                                "Previous queue item",
                                cx,
                                |player, window, cx| {
                                    player.play_previous_queue_item(window, cx);
                                },
                            ))
                            .child(context_icon_button(
                                "settings-play",
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
                                "settings-next",
                                ICON_NEXT,
                                "Next queue item",
                                cx,
                                |player, window, cx| {
                                    player.play_next_queue_item(window, cx);
                                },
                            )),
                    )
                    .child(self.render_settings_menu_section(cx)),
            )
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

    fn render_settings_menu_section(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(self.render_default_volume_setting_row(cx))
            .child(self.render_settings_action_row(
                "Resume",
                self.settings.resume_behavior.label().to_string(),
                cx,
                |player, window, cx| {
                    player.cycle_resume_behavior_setting(window, cx);
                },
            ))
            .child(self.render_settings_action_row(
                "Audio Language",
                self.settings.preferred_audio_language.clone(),
                cx,
                |player, window, cx| {
                    player.cycle_audio_language_setting(window, cx);
                },
            ))
            .child(self.render_settings_action_row(
                "Subtitle Language",
                self.settings.preferred_subtitle_language.clone(),
                cx,
                |player, window, cx| {
                    player.cycle_subtitle_language_setting(window, cx);
                },
            ))
            .child(self.render_settings_action_row(
                "Hardware Decode",
                self.settings.hardware_decoding_mode.clone(),
                cx,
                |player, window, cx| {
                    player.cycle_hardware_decoding_setting(window, cx);
                },
            ))
            .child(
                self.render_settings_action_row(
                    "Start Fullscreen",
                    if self.settings.start_fullscreen {
                        "On"
                    } else {
                        "Off"
                    }
                    .to_string(),
                    cx,
                    |player, window, cx| {
                        player.toggle_start_fullscreen_setting(window, cx);
                    },
                ),
            )
    }

    fn render_default_volume_setting_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-default-volume")
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .py_1()
            .child(div().text_sm().w(px(122.0)).child("Default Volume"))
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
    }

    fn render_settings_action_row(
        &self,
        label: &'static str,
        detail: String,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut OledVlcPlayer, &mut Window, &mut Context<OledVlcPlayer>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(format!("settings-row-{label}"))
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
        let media_title = queue_display_name(&media.path);
        let media_path = media.path.display().to_string();

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
                            .child(media_path),
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
            .id(format!("subtitle-{}", subtitle_path.display()))
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

impl Focusable for OledVlcPlayer {
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

impl Render for OledVlcPlayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_window_fullscreen = window.is_fullscreen();
        let viewport_size = window.viewport_size();
        let viewport_width = viewport_size.width.as_f32();
        let viewport_height = viewport_size.height.as_f32();
        let is_video_surface_active = self.is_video_surface_active();

        div()
            .id("oled-vlc-player")
            .key_context("OledVlcPlayer")
            .on_action(cx.listener(Self::toggle_playback))
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
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(if is_video_surface_active {
                rgb_alpha(OLED_BLACK, 0.0)
            } else {
                rgb(OLED_BLACK)
            })
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
                    || player.open_context_menu_section.is_some()
                {
                    return;
                }

                let scroll_delta = match event.delta {
                    ScrollDelta::Pixels(delta) => delta.y.as_f32(),
                    ScrollDelta::Lines(delta) => delta.y,
                };
                if scroll_delta < 0.0 {
                    player.change_volume_by(VOLUME_STEP_PERCENT, window, cx);
                } else if scroll_delta > 0.0 {
                    player.change_volume_by(-VOLUME_STEP_PERCENT, window, cx);
                }
            }))
            .on_drop(
                cx.listener(|player, external_paths: &ExternalPaths, window, cx| {
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

impl Drop for OledVlcPlayer {
    fn drop(&mut self) {
        self.record_current_media_progress();
        self.save_current_watch_session();
        self.stop_playback_process();
    }
}

fn render_autoscrolling_library_title(
    title: String,
    card_width: f32,
    section_title: &str,
    media_path: &Path,
) -> AnyElement {
    let scroll_distance = library_title_scroll_distance(&title, card_width);

    if scroll_distance <= 0.0 {
        return div()
            .min_w_0()
            .h(px(LIBRARY_TITLE_LINE_HEIGHT_PX))
            .text_sm()
            .line_height(px(LIBRARY_TITLE_LINE_HEIGHT_PX))
            .text_color(rgb(SOFT_WHITE))
            .truncate()
            .child(title)
            .into_any_element();
    }

    let animation_id = format!(
        "library-title-scroll-{}-{:016x}",
        library_safe_element_key(section_title),
        media_path_hash(media_path)
    );
    let animation_duration = library_title_scroll_duration(scroll_distance);

    div()
        .relative()
        .w_full()
        .h(px(LIBRARY_TITLE_LINE_HEIGHT_PX))
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .text_sm()
                .line_height(px(LIBRARY_TITLE_LINE_HEIGHT_PX))
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

fn library_title_scroll_distance(title: &str, card_width: f32) -> f32 {
    let estimated_title_width =
        title.chars().count() as f32 * LIBRARY_TITLE_AVERAGE_CHARACTER_WIDTH_PX;
    let available_title_width = card_width.max(1.0);

    (estimated_title_width - available_title_width + LIBRARY_TITLE_SCROLL_END_PADDING_PX).max(0.0)
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

fn simple_menu_action(
    label: &'static str,
    cx: &mut Context<OledVlcPlayer>,
    on_click: impl Fn(&mut OledVlcPlayer, &mut Window, &mut Context<OledVlcPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(format!("simple-menu-action-{label}"))
        .px_2()
        .py_1()
        .text_sm()
        .cursor_pointer()
        .hover(|menu_item| menu_item.bg(rgb(0x121212)))
        .on_click(cx.listener(move |player, _, window, cx| {
            on_click(player, window, cx);
            cx.stop_propagation();
        }))
        .child(label)
}

fn context_section_button(
    label: &'static str,
    detail: String,
    is_open: bool,
    cx: &mut Context<OledVlcPlayer>,
    on_click: impl Fn(&mut OledVlcPlayer, &mut Window, &mut Context<OledVlcPlayer>) + 'static,
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
            div()
                .w(px(12.0))
                .text_xs()
                .text_color(rgb(MUTED_TEXT))
                .child(if is_open { "v" } else { ">" }),
        )
}

fn playback_mode_icon_button(
    id: String,
    icon_path: &'static str,
    tooltip: String,
    is_active: bool,
    cx: &mut Context<OledVlcPlayer>,
    on_click: impl Fn(&mut OledVlcPlayer, &mut Window, &mut Context<OledVlcPlayer>) + 'static,
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
    cx: &mut Context<OledVlcPlayer>,
    on_click: impl Fn(&mut OledVlcPlayer, &mut Window, &mut Context<OledVlcPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(34.0))
        .h(px(30.0))
        .text_color(rgb(SOFT_WHITE))
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
    cx: &mut Context<OledVlcPlayer>,
    on_click: impl Fn(&mut OledVlcPlayer, &mut Window, &mut Context<OledVlcPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(24.0))
        .h(px(24.0))
        .text_color(rgb(SOFT_WHITE))
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
    cx: &mut Context<OledVlcPlayer>,
    on_click: impl Fn(&mut OledVlcPlayer, &mut Window, &mut Context<OledVlcPlayer>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_4()
        .py_2()
        .text_sm()
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
        .cursor_pointer()
        .hover(|button| button.opacity(0.82))
        .active(|button| button.opacity(0.72))
        .on_click(cx.listener(move |player, _, window, cx| {
            on_click(player, window, cx);
        }))
        .child(label)
}

fn square_button(
    id: &'static str,
    label: &'static str,
    tooltip: &'static str,
    cx: &mut Context<OledVlcPlayer>,
    on_click: impl Fn(&mut OledVlcPlayer, &mut Window, &mut Context<OledVlcPlayer>) + 'static,
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
    cx: &mut Context<OledVlcPlayer>,
    on_click: impl Fn(&mut OledVlcPlayer, &mut Window, &mut Context<OledVlcPlayer>) + 'static,
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
    let time_pos_seconds = read_mpv_property(ipc_path, "time-pos").and_then(json_f64);
    let duration_seconds = read_mpv_property(ipc_path, "duration").and_then(json_f64);
    let is_paused = read_mpv_property(ipc_path, "pause").and_then(|value| value.as_bool());
    let is_eof_reached =
        read_mpv_property(ipc_path, "eof-reached").and_then(|value| value.as_bool());
    let volume_percent = read_mpv_property(ipc_path, "volume")
        .and_then(json_f64)
        .map(|volume| volume.round().clamp(0.0, 100.0) as u8);
    let is_muted = read_mpv_property(ipc_path, "mute").and_then(|value| value.as_bool());
    let playback_speed = read_mpv_property(ipc_path, "speed").and_then(json_f64);
    let audio_track_id = read_mpv_property(ipc_path, "aid").and_then(json_i64);
    let subtitle_track_id = read_mpv_property(ipc_path, "sid").and_then(json_i64);
    let track_list = read_mpv_property(ipc_path, "track-list");
    let (audio_tracks, embedded_subtitle_tracks) = track_list
        .as_ref()
        .map(mpv_tracks_from_track_list)
        .unwrap_or_default();

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
        audio_tracks,
        embedded_subtitle_tracks,
    })
}

fn read_mpv_property(ipc_path: &str, property_name: &str) -> Option<Value> {
    let response = request_mpv_ipc_response(
        ipc_path,
        json!({
            "command": ["get_property", property_name],
            "request_id": property_name,
        }),
    )?;

    if response.get("error").and_then(Value::as_str) == Some("success") {
        response.get("data").cloned()
    } else {
        None
    }
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
fn request_mpv_ipc_response(ipc_path: &str, payload: Value) -> Option<Value> {
    let mut ipc_stream = OpenOptions::new()
        .read(true)
        .write(true)
        .open(ipc_path)
        .ok()?;
    let payload = payload.to_string();
    ipc_stream.write_all(payload.as_bytes()).ok()?;
    ipc_stream.write_all(b"\n").ok()?;
    ipc_stream.flush().ok()?;

    read_mpv_response_line(BufReader::new(ipc_stream))
}

#[cfg(not(target_os = "windows"))]
fn request_mpv_ipc_response(ipc_path: &str, payload: Value) -> Option<Value> {
    use std::os::unix::net::UnixStream;

    let mut ipc_stream = UnixStream::connect(ipc_path).ok()?;
    let payload = payload.to_string();
    ipc_stream.write_all(payload.as_bytes()).ok()?;
    ipc_stream.write_all(b"\n").ok()?;
    ipc_stream.flush().ok()?;

    read_mpv_response_line(BufReader::new(ipc_stream))
}

fn read_mpv_response_line<R: BufRead>(mut reader: R) -> Option<Value> {
    for _ in 0..8 {
        let mut response_line = String::new();
        if reader.read_line(&mut response_line).ok()? == 0 {
            return None;
        }

        let response = serde_json::from_str::<Value>(&response_line).ok()?;
        if response.get("error").is_some() {
            return Some(response);
        }
    }

    None
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

fn volume_slider(volume_percent: u8, cx: &mut Context<OledVlcPlayer>) -> impl IntoElement {
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
                .absolute()
                .left_0()
                .right_0()
                .top_0()
                .bottom_0()
                .flex()
                .cursor_pointer()
                .children((0..VOLUME_SLIDER_SEGMENT_COUNT).map(|segment_index| {
                    let volume_percent = segment_index as u8;

                    div()
                        .id(format!("volume-segment-{segment_index}"))
                        .flex_1()
                        .h(px(42.0))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |player, _event, window, cx| {
                                player.set_volume_percent(volume_percent, window, cx);
                            }),
                        )
                        .on_mouse_move(cx.listener(
                            move |player, event: &gpui::MouseMoveEvent, window, cx| {
                                if event.dragging() {
                                    player.set_volume_percent(volume_percent, window, cx);
                                }
                            },
                        ))
                })),
        )
}

fn default_volume_slider(
    default_volume_percent: u8,
    cx: &mut Context<OledVlcPlayer>,
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
                .absolute()
                .left_0()
                .right_0()
                .top_0()
                .bottom_0()
                .flex()
                .cursor_pointer()
                .children((0..VOLUME_SLIDER_SEGMENT_COUNT).map(|segment_index| {
                    let default_volume_percent = segment_index as u8;

                    div()
                        .id(format!("default-volume-segment-{segment_index}"))
                        .flex_1()
                        .h(px(34.0))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |player, _event, _window, cx| {
                                player.set_default_volume_percent(default_volume_percent, cx);
                                cx.stop_propagation();
                            }),
                        )
                        .on_mouse_move(cx.listener(
                            move |player, event: &gpui::MouseMoveEvent, _window, cx| {
                                if event.dragging() {
                                    player.set_default_volume_percent(default_volume_percent, cx);
                                    cx.stop_propagation();
                                }
                            },
                        ))
                })),
        )
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

fn initial_media_paths_from_args() -> Vec<PathBuf> {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .flat_map(|path| {
            if path.is_dir() {
                media_paths_in_folder(&path)
            } else if path.is_file() && is_video_path(&path) {
                vec![path]
            } else {
                Vec::new()
            }
        })
        .collect()
}

fn queue_display_name(path: &Path) -> String {
    let file_name = display_name(path);
    strip_leading_bracket_tags(&file_name)
        .filter(|cleaned_file_name| !cleaned_file_name.is_empty())
        .map(ToString::to_string)
        .unwrap_or(file_name)
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

fn media_paths_from_path(path: PathBuf) -> Vec<PathBuf> {
    if path.is_dir() {
        media_paths_in_folder(&path)
    } else if path.is_file() && is_video_path(&path) {
        vec![path]
    } else {
        Vec::new()
    }
}

fn video_file_dialog(title: &str) -> rfd::FileDialog {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter("Video files", &VIDEO_EXTENSIONS)
}

fn media_paths_in_folder(folder_path: &Path) -> Vec<PathBuf> {
    let mut media_paths = Vec::new();
    collect_media_paths_in_folder(folder_path, &mut media_paths);
    media_paths.sort_by(|left, right| compare_natural_paths(left, right));
    media_paths.dedup();
    media_paths
}

fn collect_media_paths_in_folder(folder_path: &Path, media_paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder_path) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_media_paths_in_folder(&path, media_paths);
        } else if path.is_file() && is_video_path(&path) {
            media_paths.push(path);
        }
    }
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

fn build_loaded_media(path: &Path) -> LoadedMedia {
    LoadedMedia {
        duration_seconds: discover_media_duration_seconds(path),
        audio_tracks: discover_audio_tracks(path),
        subtitle_paths: discover_sidecar_subtitles(path),
        embedded_subtitle_tracks: discover_embedded_subtitle_tracks(path),
        path: path.to_path_buf(),
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

    let _ = fs::write(session_file_path, serialized_session);
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
    let Ok(settings_value) = serde_json::from_str::<Value>(&serialized_settings) else {
        return PlayerSettings::default();
    };
    let default_settings = PlayerSettings::default();

    PlayerSettings {
        default_volume_percent: settings_value
            .get("default_volume_percent")
            .and_then(Value::as_u64)
            .map(|volume_percent| volume_percent.min(100) as u8)
            .unwrap_or(default_settings.default_volume_percent),
        resume_behavior: settings_value
            .get("resume_behavior")
            .and_then(Value::as_str)
            .map(ResumeBehavior::from_str)
            .unwrap_or(default_settings.resume_behavior),
        preferred_audio_language: settings_value
            .get("preferred_audio_language")
            .and_then(Value::as_str)
            .unwrap_or(&default_settings.preferred_audio_language)
            .to_string(),
        preferred_subtitle_language: settings_value
            .get("preferred_subtitle_language")
            .and_then(Value::as_str)
            .unwrap_or(&default_settings.preferred_subtitle_language)
            .to_string(),
        prefer_embedded_subtitles: settings_value
            .get("prefer_embedded_subtitles")
            .and_then(Value::as_bool)
            .unwrap_or(default_settings.prefer_embedded_subtitles),
        hardware_decoding_mode: settings_value
            .get("hardware_decoding_mode")
            .and_then(Value::as_str)
            .unwrap_or(&default_settings.hardware_decoding_mode)
            .to_string(),
        start_fullscreen: settings_value
            .get("start_fullscreen")
            .and_then(Value::as_bool)
            .unwrap_or(default_settings.start_fullscreen),
        subtitle_font_size: settings_value
            .get("subtitle_font_size")
            .and_then(Value::as_u64)
            .map(|font_size| font_size.clamp(24, 96) as u8)
            .unwrap_or(default_settings.subtitle_font_size),
        subtitle_color: settings_value
            .get("subtitle_color")
            .and_then(Value::as_str)
            .unwrap_or(&default_settings.subtitle_color)
            .to_string(),
        subtitle_position_percent: settings_value
            .get("subtitle_position_percent")
            .and_then(Value::as_u64)
            .map(|position_percent| position_percent.min(100) as u8)
            .unwrap_or(default_settings.subtitle_position_percent),
    }
}

fn save_player_settings(settings: &PlayerSettings) {
    let settings_file_path = watch_settings_file_path();
    let Some(settings_directory) = settings_file_path.parent() else {
        return;
    };
    if fs::create_dir_all(settings_directory).is_err() {
        return;
    }

    let settings_value = json!({
        "default_volume_percent": settings.default_volume_percent,
        "resume_behavior": settings.resume_behavior.as_str(),
        "preferred_audio_language": settings.preferred_audio_language,
        "preferred_subtitle_language": settings.preferred_subtitle_language,
        "prefer_embedded_subtitles": settings.prefer_embedded_subtitles,
        "hardware_decoding_mode": settings.hardware_decoding_mode,
        "start_fullscreen": settings.start_fullscreen,
        "subtitle_font_size": settings.subtitle_font_size,
        "subtitle_color": settings.subtitle_color,
        "subtitle_position_percent": settings.subtitle_position_percent,
    });
    let Ok(serialized_settings) = serde_json::to_vec_pretty(&settings_value) else {
        return;
    };

    let _ = fs::write(settings_file_path, serialized_settings);
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
        media_history: library_value
            .get("media_history")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(media_history_entry_from_json)
            .collect(),
    }
}

fn save_player_library(library: &PlayerLibrary) {
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
                })
            })
            .collect::<Vec<_>>(),
    });
    let Ok(serialized_library) = serde_json::to_vec_pretty(&library_value) else {
        return;
    };

    let _ = fs::write(library_file_path, serialized_library);
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
    })
}

fn current_time_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn promote_recent_path(recent_paths: &mut Vec<PathBuf>, path: PathBuf, max_len: usize) {
    recent_paths.retain(|recent_path| recent_path != &path);
    recent_paths.insert(0, path);
    recent_paths.truncate(max_len);
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

fn next_language_preference(current_language: &str) -> String {
    match current_language {
        "eng" => "jpn",
        "jpn" => "spa",
        "spa" => "fra",
        "fra" => "any",
        _ => "eng",
    }
    .to_string()
}

fn shuffled_queue_index(current_queue_index: usize, queue_len: usize) -> usize {
    if queue_len <= 1 {
        return current_queue_index;
    }

    let random_slot = (current_time_millis() as usize) % (queue_len - 1);
    if random_slot >= current_queue_index {
        random_slot + 1
    } else {
        random_slot
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
        resume_history_entry: None,
        is_watched: false,
        can_remove_from_continue_watching: false,
    }
}

fn series_library_shelves(
    library: &PlayerLibrary,
    watched_media_paths: &HashSet<PathBuf>,
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
                .take(MAX_LIBRARY_ITEMS_PER_SHELF)
                .map(|media_path| {
                    let mut item = library_item_for_media_path(&media_path, true);
                    item.is_watched = watched_media_paths.contains(&media_path);
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
        if media_path.is_file()
            && is_video_path(&media_path)
            && seen_media_paths.insert(media_path.clone())
        {
            media_paths.push(media_path);
        }
    }

    media_paths
}

fn library_content_width(viewport_width: f32) -> f32 {
    (viewport_width - 64.0).clamp(320.0, 1360.0)
}

fn library_card_width(viewport_width: f32) -> f32 {
    let content_width = library_content_width(viewport_width);
    let target_card_count = if content_width >= 1220.0 {
        6.0
    } else if content_width >= 960.0 {
        5.0
    } else if content_width >= 720.0 {
        4.0
    } else if content_width >= 500.0 {
        3.0
    } else {
        2.0
    };
    let gap_width = 12.0 * (target_card_count - 1.0);

    ((content_width - gap_width) / target_card_count).clamp(136.0, 220.0)
}

fn library_visible_card_count(viewport_width: f32) -> usize {
    let content_width = library_content_width(viewport_width);
    let card_width = library_card_width(viewport_width);
    let gap_width = 12.0;

    ((content_width + gap_width) / (card_width + gap_width))
        .floor()
        .max(1.0) as usize
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

fn first_existing_timeline_thumbnail_path(media_path: &Path) -> Option<PathBuf> {
    let thumbnail_directory = watch_session_directory().join(THUMBNAIL_CACHE_DIRECTORY_NAME);
    let media_hash_prefix = format!("{:016x}-", media_path_hash(media_path));
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
    let media_hash = media_path_hash(media_path);
    let thumbnail_second = thumbnail_second_for_position(position_seconds);

    watch_session_directory()
        .join(THUMBNAIL_CACHE_DIRECTORY_NAME)
        .join(format!("{media_hash:016x}-{thumbnail_second}.jpg"))
}

fn media_path_hash(media_path: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    media_path.hash(&mut hasher);
    hasher.finish()
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

fn discover_media_duration_seconds(media_path: &Path) -> Option<f64> {
    let ffprobe_path = locate_dependency("ffprobe")?;
    let mut command = Command::new(ffprobe_path);
    command
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(media_path.as_os_str())
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
        .filter(|duration_seconds| duration_seconds.is_finite() && *duration_seconds > 0.0)
}

fn discover_audio_tracks(media_path: &Path) -> Vec<AudioTrack> {
    let Some(ffprobe_path) = locate_dependency("ffprobe") else {
        return Vec::new();
    };
    let mut command = Command::new(ffprobe_path);
    command
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("a")
        .arg("-show_entries")
        .arg("stream=index,codec_name:stream_tags=language,title")
        .arg("-of")
        .arg("json")
        .arg(media_path.as_os_str())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_child_process_window(&mut command);

    let Ok(output) = command.output() else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let Ok(ffprobe_output) = serde_json::from_slice::<Value>(&output.stdout) else {
        return Vec::new();
    };

    ffprobe_output
        .get("streams")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(audio_index, stream)| {
            let track_id = (audio_index + 1) as i64;
            let tags = stream.get("tags");
            let title = tags
                .and_then(|tags| tags.get("title"))
                .and_then(Value::as_str)
                .filter(|title| !title.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("Audio track {track_id}"));
            let language = tags
                .and_then(|tags| tags.get("language"))
                .and_then(Value::as_str)
                .filter(|language| !language.is_empty())
                .map(ToString::to_string);
            let codec = stream
                .get("codec_name")
                .and_then(Value::as_str)
                .filter(|codec| !codec.is_empty())
                .map(ToString::to_string);

            AudioTrack {
                track_id,
                title,
                language,
                codec,
            }
        })
        .collect()
}

fn discover_embedded_subtitle_tracks(media_path: &Path) -> Vec<EmbeddedSubtitleTrack> {
    let Some(ffprobe_path) = locate_dependency("ffprobe") else {
        return Vec::new();
    };
    let mut command = Command::new(ffprobe_path);
    command
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("s")
        .arg("-show_entries")
        .arg("stream=index,codec_name:stream_tags=language,title")
        .arg("-of")
        .arg("json")
        .arg(media_path.as_os_str())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_child_process_window(&mut command);

    let Ok(output) = command.output() else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let Ok(ffprobe_output) = serde_json::from_slice::<Value>(&output.stdout) else {
        return Vec::new();
    };

    ffprobe_output
        .get("streams")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(subtitle_index, stream)| {
            let track_id = (subtitle_index + 1) as i64;
            let tags = stream.get("tags");
            let title = tags
                .and_then(|tags| tags.get("title"))
                .and_then(Value::as_str)
                .filter(|title| !title.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("Subtitle track {track_id}"));
            let language = tags
                .and_then(|tags| tags.get("language"))
                .and_then(Value::as_str)
                .filter(|language| !language.is_empty())
                .map(ToString::to_string);
            let codec = stream
                .get("codec_name")
                .and_then(Value::as_str)
                .filter(|codec| !codec.is_empty())
                .map(ToString::to_string);

            EmbeddedSubtitleTrack {
                track_id,
                title,
                language,
                codec,
                is_selected: false,
            }
        })
        .collect()
}

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

fn run_application() {
    let initial_media_paths = initial_media_paths_from_args();

    application().run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("space", TogglePlayback, Some("OledVlcPlayer")),
            KeyBinding::new("k", TogglePlayback, Some("OledVlcPlayer")),
            KeyBinding::new("left", SeekBackward, Some("OledVlcPlayer")),
            KeyBinding::new("right", SeekForward, Some("OledVlcPlayer")),
            KeyBinding::new("j", SeekBackward, Some("OledVlcPlayer")),
            KeyBinding::new("l", SeekForward, Some("OledVlcPlayer")),
            KeyBinding::new("up", VolumeUp, Some("OledVlcPlayer")),
            KeyBinding::new("down", VolumeDown, Some("OledVlcPlayer")),
            KeyBinding::new("h", IncreaseSubtitleDelay, Some("OledVlcPlayer")),
            KeyBinding::new("g", DecreaseSubtitleDelay, Some("OledVlcPlayer")),
            KeyBinding::new("shift-g", ResetSubtitleDelay, Some("OledVlcPlayer")),
            KeyBinding::new("=", IncreasePlaybackSpeed, Some("OledVlcPlayer")),
            KeyBinding::new("-", DecreasePlaybackSpeed, Some("OledVlcPlayer")),
            KeyBinding::new("s", ToggleShuffle, Some("OledVlcPlayer")),
            KeyBinding::new("r", CycleRepeatMode, Some("OledVlcPlayer")),
        ]);

        let bounds = Bounds::centered(None, size(px(1500.0), px(920.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(APP_NAME.into()),
                    ..Default::default()
                }),
                window_background: WindowBackgroundAppearance::Transparent,
                app_id: Some(APP_ID.to_string()),
                focus: true,
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title(APP_NAME);
                window.set_app_id(APP_ID);
                let player = cx.new(|cx| OledVlcPlayer::new(initial_media_paths, window, cx));
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
