use crate::models::AppSettings;
use crate::settings_store;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.settings.lock().clone())
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: AppSettings) -> Result<(), String> {
    settings_store::save_settings(&settings)?;
    *state.settings.lock() = settings;
    Ok(())
}
