use crate::capture;
use crate::models::{RecordingProgress, RecordingState, RecordingStatus};
use crate::AppState;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub fn get_recording_status(state: State<'_, AppState>) -> Result<RecordingStatus, String> {
    let session = state.session.lock();
    Ok(RecordingStatus {
        state: session.state(),
        progress: session.progress(),
        last_error: session.last_error(),
    })
}

#[tauri::command]
pub async fn start_recording(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.settings.lock().clone();
    if settings.selected_monitor_ids.is_empty() {
        return Err("Select at least one display before recording.".into());
    }

    let targets = tauri::async_runtime::spawn_blocking({
        let ids = settings.selected_monitor_ids.clone();
        move || capture::resolve_capture_targets(&ids)
    })
    .await
    .map_err(|e| e.to_string())??;

    {
        let mut session = state.session.lock();
        session.start(app.clone(), settings.clone(), targets)?;
    }

    let _ = app.emit("recording_started", ());
    spawn_progress_emitter(app.clone());
    Ok(())
}

#[tauri::command]
pub fn stop_recording(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.session.lock().stop();
    let _ = app.emit("recording_progress", progress_payload(state.inner()));
    Ok(())
}

fn spawn_progress_emitter(app: AppHandle) {
    std::thread::spawn(move || {
        while {
            let state = app.state::<AppState>();
            let s = state.session.lock().state();
            s == RecordingState::Recording || s == RecordingState::Starting
        } {
            let state = app.state::<AppState>();
            let _ = app.emit("recording_progress", progress_payload(&state));
            std::thread::sleep(std::time::Duration::from_millis(750));
        }
        let state = app.state::<AppState>();
        let final_state = state.session.lock().state();
        if final_state == RecordingState::Completed {
            let _ = app.emit("recording_stopped", ());
        } else if final_state == RecordingState::Error {
            let err = state
                .session
                .lock()
                .last_error()
                .unwrap_or_else(|| "Recording failed".into());
            let _ = app.emit("recording_error", err);
        }
    });
}

fn progress_payload(state: &AppState) -> RecordingProgress {
    let session = state.session.lock();
    session.progress().unwrap_or(RecordingProgress {
        elapsed_secs: 0,
        state: session.state(),
        display_count: state.settings.lock().selected_monitor_ids.len(),
        output_paths: vec![],
        file_size_bytes: None,
        system_audio: state.settings.lock().system_audio_enabled,
        microphone: state.settings.lock().microphone_enabled,
    })
}
