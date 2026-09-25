use crate::audio::microphone;
use crate::models::AudioDeviceInfo;

#[tauri::command]
pub fn get_audio_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    let devices = microphone::enumerate_microphones()?;
    Ok(devices
        .into_iter()
        .map(|(id, name, is_default)| AudioDeviceInfo {
            id,
            name,
            is_default,
        })
        .collect())
}
