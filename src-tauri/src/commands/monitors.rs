use crate::capture;
use crate::models::MonitorInfo;

#[tauri::command]
pub fn get_monitors() -> Result<Vec<MonitorInfo>, String> {
    Ok(capture::enumerate_monitors())
}
