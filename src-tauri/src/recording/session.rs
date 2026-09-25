use crate::audio::microphone::{start_microphone_capture, MicrophoneHandle};
use crate::audio::mixer::{start_audio_mixer, PcmChunk};
use crate::audio::system::{start_system_audio_capture, SystemAudioHandle};
use crate::capture::frame::CapturedFrame;
use crate::capture::monitor::{start_monitor_capture, MonitorCaptureHandle, recording_clock_100ns};
use crate::encoder::muxer::Mp4Writer;
use crate::logging;
use crate::recording::preview;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use crate::models::{
    AppSettings, MonitorCaptureTarget, RecordingMode, RecordingProgress,
    RecordingState,
};
use chrono::Local;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub struct RecordingSessionController {
    state: Arc<Mutex<RecordingState>>,
    progress: Arc<Mutex<Option<RecordingProgress>>>,
    last_error: Arc<Mutex<Option<String>>>,
    stop_signal: Arc<std::sync::atomic::AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl RecordingSessionController {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(RecordingState::Idle)),
            progress: Arc::new(Mutex::new(None)),
            last_error: Arc::new(Mutex::new(None)),
            stop_signal: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            worker: None,
        }
    }

    pub fn state(&self) -> RecordingState {
        *self.state.lock().unwrap()
    }

    pub fn progress(&self) -> Option<RecordingProgress> {
        self.progress.lock().unwrap().clone()
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error.lock().unwrap().clone()
    }

    pub fn start(
        &mut self,
        app: AppHandle,
        settings: AppSettings,
        targets: Vec<MonitorCaptureTarget>,
    ) -> Result<(), String> {
        if *self.state.lock().unwrap() != RecordingState::Idle
            && *self.state.lock().unwrap() != RecordingState::Completed
            && *self.state.lock().unwrap() != RecordingState::Error
        {
            return Err("A recording session is already active".into());
        }

        *self.state.lock().unwrap() = RecordingState::Starting;
        *self.last_error.lock().unwrap() = None;
        self.stop_signal
            .store(false, std::sync::atomic::Ordering::SeqCst);

        let output_paths = build_output_paths(&settings, &targets)?;
        fs::create_dir_all(&settings.output_directory).map_err(|e| e.to_string())?;

        let session_start = recording_clock_100ns();
        let state = self.state.clone();
        let progress = self.progress.clone();
        let stop_signal = self.stop_signal.clone();

        let state_for_err = state.clone();
        let last_error_for_err = self.last_error.clone();
        let worker = thread::Builder::new()
            .name("recording-session".into())
            .spawn(move || {
                if let Err(e) = run_session(
                    app,
                    settings,
                    targets,
                    output_paths,
                    session_start,
                    state,
                    progress,
                    stop_signal,
                ) {
                    logging::error(format!("Recording session error: {e}"));
                    *last_error_for_err.lock().unwrap() = Some(e);
                    *state_for_err.lock().unwrap() = RecordingState::Error;
                }
            })
            .map_err(|e| e.to_string())?;

        self.worker = Some(worker);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.stop_signal
            .store(true, std::sync::atomic::Ordering::SeqCst);
        *self.state.lock().unwrap() = RecordingState::Stopping;
    }

    pub fn join_if_finished(&mut self) {
        if self.worker.is_none() {
            return;
        }
        let stopping = *self.state.lock().unwrap() == RecordingState::Stopping
            || *self.state.lock().unwrap() == RecordingState::Completed
            || *self.state.lock().unwrap() == RecordingState::Error
            || *self.state.lock().unwrap() == RecordingState::Idle;
        if stopping {
            if let Some(handle) = self.worker.take() {
                if handle.is_finished() {
                    let _ = handle.join();
                }
            }
        }
    }
}

fn build_output_paths(
    settings: &AppSettings,
    targets: &[MonitorCaptureTarget],
) -> Result<Vec<PathBuf>, String> {
    let stamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let base = PathBuf::from(&settings.output_directory);
    let mut paths = Vec::new();
    match settings.recording_mode {
        RecordingMode::Combined => {
            let file = unique_path(&base, &format!("SimpleRecorder_{}", stamp), ".mp4");
            paths.push(file);
        }
        RecordingMode::Separate => {
            for t in targets {
                let file = unique_path(
                    &base,
                    &format!("SimpleRecorder_{}_Monitor-{}", stamp, t.info.index),
                    ".mp4",
                );
                paths.push(file);
            }
        }
    }
    Ok(paths)
}

fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut path = dir.join(format!("{}{}", stem, ext));
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("{}_{}{}", stem, n, ext));
        n += 1;
    }
    path
}

#[derive(Clone, Serialize)]
struct PreviewEvent {
    jpeg_base64: String,
}

fn run_session(
    app: AppHandle,
    settings: AppSettings,
    targets: Vec<MonitorCaptureTarget>,
    output_paths: Vec<PathBuf>,
    session_start: i64,
    state: Arc<Mutex<RecordingState>>,
    progress: Arc<Mutex<Option<RecordingProgress>>>,
    stop_signal: Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    let include_audio = settings.system_audio_enabled || settings.microphone_enabled;
    let (canvas_w, canvas_h, min_x, min_y) = combined_canvas(&targets);

    let (sys_tx, sys_rx) = mpsc::channel::<PcmChunk>();
    let (mic_tx, mic_rx) = mpsc::channel::<PcmChunk>();
    let (mix_tx, mix_rx) = mpsc::channel::<PcmChunk>();

    let mut system_handle: Option<SystemAudioHandle> = None;
    let mut mic_handle: Option<MicrophoneHandle> = None;

    if settings.system_audio_enabled {
        system_handle = Some(start_system_audio_capture(sys_tx)?);
    }
    if settings.microphone_enabled {
        mic_handle = Some(start_microphone_capture(
            settings.selected_microphone_id.clone(),
            mic_tx,
        )?);
    }

    let sys_rx_opt = if settings.system_audio_enabled {
        Some(sys_rx)
    } else {
        None
    };
    let mic_rx_opt = if settings.microphone_enabled {
        Some(mic_rx)
    } else {
        None
    };

    let mut capture_handles: Vec<MonitorCaptureHandle> = Vec::new();
    let mut frame_channels: Vec<(String, Receiver<CapturedFrame>)> = Vec::new();

    for target in targets.iter().cloned() {
        let (tx, rx) = mpsc::sync_channel(2);
        let handle = start_monitor_capture(target.clone(), settings.fps, tx, session_start)?;
        capture_handles.push(handle);
        frame_channels.push((target.info.id.clone(), rx));
    }

    let latest_frames: Arc<Mutex<HashMap<String, CapturedFrame>>> =
        Arc::new(Mutex::new(HashMap::new()));

    for (id, rx) in frame_channels {
        let map = latest_frames.clone();
        thread::spawn(move || {
            while let Ok(frame) = rx.recv() {
                map.lock().unwrap().insert(id.clone(), frame);
            }
        });
    }

    let warmup_deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < warmup_deadline && !stop_signal.load(std::sync::atomic::Ordering::SeqCst) {
        if !latest_frames.lock().unwrap().is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    if latest_frames.lock().unwrap().is_empty() {
        return Err("Display capture did not start. Try again or select one monitor.".into());
    }

    let mut writers: Vec<Mp4Writer> = Vec::new();
    match settings.recording_mode {
        RecordingMode::Combined => {
            let w = Mp4Writer::create(
                &output_paths[0],
                canvas_w,
                canvas_h,
                settings.fps,
                settings.quality,
                include_audio,
            )?;
            writers.push(w);
        }
        RecordingMode::Separate => {
            for (i, t) in targets.iter().enumerate() {
                let w = Mp4Writer::create(
                    &output_paths[i],
                    t.info.width,
                    t.info.height,
                    settings.fps,
                    settings.quality,
                    include_audio && i == 0,
                )?;
                writers.push(w);
            }
        }
    }

    let mut mixer_handle = start_audio_mixer(sys_rx_opt, mic_rx_opt, mix_tx, session_start);
    *state.lock().unwrap() = RecordingState::Recording;

    let start_instant = Instant::now();
    let mut last_progress = Instant::now();
    let path_strings = output_paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    let frame_period = Duration::from_millis((1000 / settings.fps.max(1)) as u64);
    let mut last_encode_at = Instant::now() - frame_period;
    let mut last_combined_ts: i64 = -1;
    let mut last_separate_ts: HashMap<String, i64> = HashMap::new();
    let mut last_preview = Instant::now() - Duration::from_secs(1);

    while !stop_signal.load(std::sync::atomic::Ordering::SeqCst) {
        if last_progress.elapsed() >= Duration::from_millis(500) {
            let elapsed = start_instant.elapsed().as_secs();
            let size = output_paths
                .iter()
                .filter_map(|p| fs::metadata(p).ok())
                .map(|m| m.len())
                .sum();
            *progress.lock().unwrap() = Some(RecordingProgress {
                elapsed_secs: elapsed,
                state: RecordingState::Recording,
                display_count: targets.len(),
                output_paths: path_strings.clone(),
                file_size_bytes: Some(size),
                system_audio: settings.system_audio_enabled,
                microphone: settings.microphone_enabled,
            });
            last_progress = Instant::now();
        }

        while let Ok(audio) = mix_rx.try_recv() {
            if let Some(writer) = writers.first_mut() {
                writer.write_audio_pcm(&audio.samples, audio.timestamp_100ns).ok();
            }
        }

        if last_preview.elapsed() >= Duration::from_millis(800) {
            let preview_frame = preview_source_frame(
                &settings.recording_mode,
                &targets,
                &latest_frames.lock().unwrap(),
                canvas_w,
                canvas_h,
                min_x,
                min_y,
            );
            if let Some(frame) = preview_frame {
                if let Some(jpeg) = preview::preview_jpeg_base64(&frame) {
                    let _ = app.emit(
                        "recording_preview",
                        PreviewEvent {
                            jpeg_base64: jpeg,
                        },
                    );
                }
            }
            last_preview = Instant::now();
        }

        if last_encode_at.elapsed() < frame_period {
            thread::sleep(Duration::from_millis(8));
            continue;
        }

        match settings.recording_mode {
            RecordingMode::Combined => {
                if let Some(writer) = writers.first_mut() {
                    if let Some(frame) = composite_frame(
                        &targets,
                        &latest_frames.lock().unwrap(),
                        canvas_w,
                        canvas_h,
                        min_x,
                        min_y,
                    ) {
                        if frame.timestamp_100ns != last_combined_ts {
                            writer
                                .write_video_frame(&frame.bgra, frame.timestamp_100ns)
                                .map_err(|e| e.to_string())?;
                            last_combined_ts = frame.timestamp_100ns;
                            last_encode_at = Instant::now();
                        }
                    }
                }
            }
            RecordingMode::Separate => {
                let map = latest_frames.lock().unwrap();
                let mut wrote = false;
                for (i, t) in targets.iter().enumerate() {
                    if let Some(frame) = map.get(&t.info.id) {
                        let prev = last_separate_ts.get(&t.info.id).copied().unwrap_or(-1);
                        if frame.timestamp_100ns != prev {
                            writers[i]
                                .write_video_frame(&frame.bgra, frame.timestamp_100ns)
                                .map_err(|e| e.to_string())?;
                            last_separate_ts.insert(t.info.id.clone(), frame.timestamp_100ns);
                            wrote = true;
                        }
                    }
                }
                if wrote {
                    last_encode_at = Instant::now();
                }
            }
        }

        thread::sleep(Duration::from_millis(2));
    }

    *state.lock().unwrap() = RecordingState::Stopping;

    for mut h in capture_handles {
        h.stop();
    }
    if let Some(mut h) = system_handle {
        h.stop();
    }
    if let Some(mut h) = mic_handle {
        h.stop();
    }
    mixer_handle.stop();

    while let Ok(audio) = mix_rx.try_recv() {
        if let Some(writer) = writers.first_mut() {
            writer.write_audio_pcm(&audio.samples, audio.timestamp_100ns).ok();
        }
    }

    for writer in writers {
        writer.finalize()?;
    }

    for p in &output_paths {
        if !p.exists() {
            return Err(format!("Output file missing after finalize: {}", p.display()));
        }
        logging::info(format!(
            "Recording saved: {} ({} bytes)",
            p.display(),
            fs::metadata(p).map(|m| m.len()).unwrap_or(0)
        ));
    }

    *state.lock().unwrap() = RecordingState::Completed;
    *progress.lock().unwrap() = None;
    Ok(())
}

fn preview_source_frame(
    mode: &RecordingMode,
    targets: &[MonitorCaptureTarget],
    frames: &HashMap<String, CapturedFrame>,
    canvas_w: u32,
    canvas_h: u32,
    min_x: i32,
    min_y: i32,
) -> Option<CapturedFrame> {
    match mode {
        RecordingMode::Combined => composite_frame(targets, frames, canvas_w, canvas_h, min_x, min_y)
            .or_else(|| frames.values().next().cloned()),
        RecordingMode::Separate => frames.values().next().cloned(),
    }
}

fn combined_canvas(targets: &[MonitorCaptureTarget]) -> (u32, u32, i32, i32) {
    let min_x = targets.iter().map(|t| t.info.x).min().unwrap_or(0);
    let min_y = targets.iter().map(|t| t.info.y).min().unwrap_or(0);
    let max_x = targets
        .iter()
        .map(|t| t.info.x + t.info.width as i32)
        .max()
        .unwrap_or(1920);
    let max_y = targets
        .iter()
        .map(|t| t.info.y + t.info.height as i32)
        .max()
        .unwrap_or(1080);
    let w = (max_x - min_x) as u32;
    let h = (max_y - min_y) as u32;
    (w.max(2), h.max(2), min_x, min_y)
}

fn composite_frame(
    targets: &[MonitorCaptureTarget],
    frames: &HashMap<String, CapturedFrame>,
    canvas_w: u32,
    canvas_h: u32,
    min_x: i32,
    min_y: i32,
) -> Option<CapturedFrame> {
    let mut canvas = vec![0u8; (canvas_w * canvas_h * 4) as usize];
    let mut timestamp = 0i64;
    let mut any = false;
    for t in targets {
        let frame = frames.get(&t.info.id)?;
        any = true;
        timestamp = timestamp.max(frame.timestamp_100ns);
        blit_bgra(
            &mut canvas,
            canvas_w,
            canvas_h,
            (t.info.x - min_x) as u32,
            (t.info.y - min_y) as u32,
            frame.width,
            frame.height,
            &frame.bgra,
        );
    }
    if !any {
        return None;
    }
    Some(CapturedFrame {
        width: canvas_w,
        height: canvas_h,
        bgra: canvas,
        timestamp_100ns: timestamp,
    })
}

fn blit_bgra(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    dst_x: u32,
    dst_y: u32,
    src_w: u32,
    src_h: u32,
    src: &[u8],
) {
    for y in 0..src_h {
        let cy = dst_y + y;
        if cy >= canvas_h {
            break;
        }
        for x in 0..src_w {
            let cx = dst_x + x;
            if cx >= canvas_w {
                break;
            }
            let src_i = ((y * src_w + x) * 4) as usize;
            let dst_i = ((cy * canvas_w + cx) * 4) as usize;
            if src_i + 3 < src.len() && dst_i + 3 < canvas.len() {
                canvas[dst_i..dst_i + 4].copy_from_slice(&src[src_i..src_i + 4]);
            }
        }
    }
}
