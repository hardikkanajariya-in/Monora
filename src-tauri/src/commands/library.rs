use crate::AppState;
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct RecordingEntry {
    pub file_name: String,
    pub path: String,
    pub size_bytes: u64,
    pub modified_unix_ms: i64,
}

#[tauri::command]
pub fn list_recordings(state: State<'_, AppState>) -> Result<Vec<RecordingEntry>, String> {
    let dir = state.settings.lock().output_directory.clone();
    list_recordings_in_dir(&dir)
}

pub fn list_recordings_in_dir(dir: &str) -> Result<Vec<RecordingEntry>, String> {
    let path = Path::new(dir);
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let read_dir = fs::read_dir(path).map_err(|e| e.to_string())?;
    for item in read_dir {
        let item = item.map_err(|e| e.to_string())?;
        let file_path = item.path();
        if !file_path.is_file() {
            continue;
        }
        let name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if !is_app_recording_name(&name) {
            continue;
        }
        let meta = fs::metadata(&file_path).map_err(|e| e.to_string())?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        entries.push(RecordingEntry {
            file_name: name,
            path: file_path.to_string_lossy().into_owned(),
            size_bytes: meta.len(),
            modified_unix_ms: modified,
        });
    }

    entries.sort_by(|a, b| b.modified_unix_ms.cmp(&a.modified_unix_ms));
    Ok(entries)
}

fn is_app_recording_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("simplerecorder_") && lower.ends_with(".mp4")
}
