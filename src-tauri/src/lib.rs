mod audio;
mod capture;
mod commands;
mod encoder;
mod logging;
mod models;
mod recording;
mod settings_store;

use commands::{audio as audio_cmds, monitors, recording as recording_cmds, settings};
use tauri::Emitter;
use models::AppSettings;
use parking_lot::Mutex;
use recording::session::RecordingSessionController;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub session: Mutex<RecordingSessionController>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::init_logging();
    logging::info("Simple Recorder starting");

    let saved = settings_store::load_settings();
    let mut settings = saved;
    if settings.selected_monitor_ids.is_empty() {
        let mons = capture::enumerate_monitors();
        settings.selected_monitor_ids = mons.iter().map(|m| m.id.clone()).collect();
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            settings: Mutex::new(settings),
            session: Mutex::new(RecordingSessionController::new()),
        })
        .setup(|app| {
            setup_tray(app)?;
            setup_hotkey(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            monitors::get_monitors,
            audio_cmds::get_audio_devices,
            settings::get_settings,
            settings::save_settings,
            recording_cmds::get_recording_status,
            recording_cmds::start_recording,
            recording_cmds::stop_recording,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let open_i = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let stop_i = MenuItem::with_id(app, "stop", "Stop Recording", true, None::<&str>)?;
    let exit_i = MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_i, &stop_i, &exit_i])?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Simple Recorder")
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "open" => {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
                "stop" => {
                    let state = app.state::<AppState>();
                    state.session.lock().stop();
                }
                "exit" => {
                    let state = app.state::<AppState>();
                    let active = state.session.lock().state() == models::RecordingState::Recording
                        || state.session.lock().state() == models::RecordingState::Starting;
                    if active {
                        // User must confirm via UI; tray exit is ignored while recording.
                        logging::warn("Exit ignored while recording is active");
                        return;
                    }
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn setup_hotkey(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

    let handle = app.handle().clone();
    app.handle().plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                let state = app.state::<AppState>();
                let current = state.session.lock().state();
                if current == models::RecordingState::Recording
                    || current == models::RecordingState::Starting
                {
                    state.session.lock().stop();
                    let _ = app.emit("recording_progress", ());
                } else if current == models::RecordingState::Idle
                    || current == models::RecordingState::Completed
                    || current == models::RecordingState::Error
                {
                    let _ = recording_cmds::start_recording(app.clone(), app.state());
                }
            })
            .build(),
    )?;

    let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyR);
    handle.global_shortcut().register(shortcut)?;
    Ok(())
}
