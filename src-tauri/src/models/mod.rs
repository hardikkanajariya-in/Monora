use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingMode {
    Combined,
    Separate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Balanced,
    High,
    VeryHigh,
}

impl Quality {
    pub fn video_bitrate_bps(self, width: u32, height: u32) -> u32 {
        let pixels = width as u64 * height as u64;
        let base = pixels / 4;
        match self {
            Quality::Balanced => base as u32,
            Quality::High => (base * 3 / 2) as u32,
            Quality::VeryHigh => (base * 2) as u32,
        }
    }

    pub fn audio_bitrate_bps(self) -> u32 {
        match self {
            Quality::Balanced => 128_000,
            Quality::High => 160_000,
            Quality::VeryHigh => 192_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingState {
    Idle,
    Starting,
    Recording,
    Stopping,
    Completed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub id: String,
    pub name: String,
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub refresh_rate_hz: Option<u32>,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub selected_monitor_ids: Vec<String>,
    pub recording_mode: RecordingMode,
    pub fps: u32,
    pub quality: Quality,
    pub system_audio_enabled: bool,
    pub microphone_enabled: bool,
    pub selected_microphone_id: Option<String>,
    pub output_directory: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            selected_monitor_ids: Vec::new(),
            recording_mode: RecordingMode::Combined,
            fps: 30,
            quality: Quality::High,
            system_audio_enabled: true,
            microphone_enabled: true,
            selected_microphone_id: None,
            output_directory: default_output_directory(),
        }
    }
}

pub fn default_output_directory() -> String {
    if let Some(videos) = dirs_videos() {
        let path = videos.join("SimpleRecorder");
        return path.to_string_lossy().into_owned();
    }
    std::env::temp_dir()
        .join("SimpleRecorder")
        .to_string_lossy()
        .into_owned()
}

fn dirs_videos() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        use std::path::PathBuf;
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let p = PathBuf::from(profile).join("Videos");
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingProgress {
    pub elapsed_secs: u64,
    pub state: RecordingState,
    pub display_count: usize,
    pub output_paths: Vec<String>,
    pub file_size_bytes: Option<u64>,
    pub system_audio: bool,
    pub microphone: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingStatus {
    pub state: RecordingState,
    pub progress: Option<RecordingProgress>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MonitorCaptureTarget {
    pub info: MonitorInfo,
    pub hmonitor: isize,
}
