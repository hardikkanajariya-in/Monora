use crate::audio::mixer::PcmChunk;
use crate::logging;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use wasapi::*;

pub struct MicrophoneHandle {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl MicrophoneHandle {
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn enumerate_microphones() -> Result<Vec<(String, String, bool)>, String> {
    initialize_mta().ok();
    let enumerator = DeviceEnumerator::new().map_err(|e| e.to_string())?;
    let default = enumerator
        .get_default_device(&Direction::Capture)
        .map_err(|e| e.to_string())?;
    let default_id = default.get_id().map_err(|e| e.to_string())?;
    let collection = enumerator
        .get_device_collection(&Direction::Capture)
        .map_err(|e| e.to_string())?;
    let count = collection.get_nbr_devices().map_err(|e| e.to_string())?;
    let mut devices = Vec::new();
    for i in 0..count {
        let dev = collection.get_device_at_index(i).map_err(|e| e.to_string())?;
        let id = dev.get_id().map_err(|e| e.to_string())?;
        let name = dev.get_friendlyname().map_err(|e| e.to_string())?;
        let is_default = id == default_id;
        devices.push((id, name, is_default));
    }
    Ok(devices)
}

pub fn start_microphone_capture(
    device_id: Option<String>,
    out_tx: Sender<PcmChunk>,
) -> Result<MicrophoneHandle, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    let thread = thread::Builder::new()
        .name("microphone".into())
        .spawn(move || {
            if let Err(e) = mic_loop(device_id, out_tx, stop_flag) {
                logging::error(format!("Microphone capture failed: {e}"));
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(MicrophoneHandle {
        stop,
        thread: Some(thread),
    })
}

fn mic_loop(
    device_id: Option<String>,
    out_tx: Sender<PcmChunk>,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    initialize_mta().ok();

    let enumerator = DeviceEnumerator::new().map_err(|e| e.to_string())?;
    let device = if let Some(id) = device_id {
        enumerator.get_device(&id).map_err(|e| e.to_string())?
    } else {
        enumerator
            .get_default_device(&Direction::Capture)
            .map_err(|e| e.to_string())?
    };

    let mut audio_client = device.get_iaudioclient().map_err(|e| e.to_string())?;
    let desired_format = WaveFormat::new(32, 32, &SampleType::Float, 48_000, 2, None);
    let (_, min_time) = audio_client.get_device_period().map_err(|e| e.to_string())?;

    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: min_time,
    };
    audio_client
        .initialize_client(&desired_format, &Direction::Capture, &mode)
        .map_err(|e| e.to_string())?;

    let capture_client = audio_client
        .get_audiocaptureclient()
        .map_err(|e| e.to_string())?;
    let h_event = audio_client
        .set_get_eventhandle()
        .map_err(|e| e.to_string())?;
    audio_client.start_stream().map_err(|e| e.to_string())?;

    logging::info("Microphone capture started");

    let mut sample_queue: VecDeque<u8> = VecDeque::new();

    while !stop.load(Ordering::SeqCst) {
        capture_client
            .read_from_device_to_deque(&mut sample_queue)
            .map_err(|e| e.to_string())?;

        let block = desired_format.get_blockalign() as usize * 960;
        while sample_queue.len() >= block {
            let chunk_bytes: Vec<u8> = sample_queue.drain(..block).collect();
            let samples = bytes_to_f32_stereo(&chunk_bytes);
            let ts = crate::capture::monitor::recording_clock_100ns();
            if out_tx
                .send(PcmChunk {
                    samples,
                    timestamp_100ns: ts,
                })
                .is_err()
            {
                break;
            }
        }

        if h_event.wait_for_event(200).is_err() {
            continue;
        }
    }

    audio_client.stop_stream().ok();
    Ok(())
}

fn bytes_to_f32_stereo(bytes: &[u8]) -> Vec<f32> {
    let count = bytes.len() / 4;
    let mut floats = Vec::with_capacity(count);
    for i in 0..count {
        let off = i * 4;
        floats.push(f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()));
    }
    floats
}
