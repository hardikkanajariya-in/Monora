use crate::logging;
use crate::models::AppSettings;
use std::fs;
use std::path::PathBuf;

pub fn settings_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.join("config").join("settings.json");
        }
    }
    std::env::temp_dir().join("simple-recorder-settings.json")
}

pub fn load_settings() -> AppSettings {
    let path = settings_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<AppSettings>(&text) {
                Ok(s) => return s,
                Err(e) => logging::warn(format!("Failed to parse settings: {e}")),
            },
            Err(e) => logging::warn(format!("Failed to read settings: {e}")),
        }
    }
    AppSettings::default()
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    logging::info(format!("Settings saved to {}", path.display()));
    Ok(())
}
