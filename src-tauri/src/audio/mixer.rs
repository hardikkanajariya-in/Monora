use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;

#[derive(Clone)]
pub struct PcmChunk {
    pub samples: Vec<f32>,
    pub timestamp_100ns: i64,
}

pub struct AudioMixerHandle {
    stop: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl AudioMixerHandle {
    pub fn stop(&mut self) {
        self.stop
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn start_audio_mixer(
    system_rx: Option<Receiver<PcmChunk>>,
    mic_rx: Option<Receiver<PcmChunk>>,
    out_tx: Sender<PcmChunk>,
    session_start_100ns: i64,
) -> AudioMixerHandle {
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag = stop.clone();

    let thread = thread::Builder::new()
        .name("audio-mixer".into())
        .spawn(move || {
            mixer_loop(system_rx, mic_rx, out_tx, session_start_100ns, stop_flag);
        })
        .expect("audio mixer thread");

    AudioMixerHandle {
        stop,
        thread: Some(thread),
    }
}

fn mixer_loop(
    system_rx: Option<Receiver<PcmChunk>>,
    mic_rx: Option<Receiver<PcmChunk>>,
    out_tx: Sender<PcmChunk>,
    session_start_100ns: i64,
    stop: Arc<std::sync::atomic::AtomicBool>,
) {
    let block_samples = (SAMPLE_RATE / 50) as usize * CHANNELS as usize; // ~20ms
    let mut sys_buf: Vec<f32> = Vec::new();
    let mut mic_buf: Vec<f32> = Vec::new();
    let mut last_sys = PcmChunk {
        samples: vec![0.0; block_samples],
        timestamp_100ns: 0,
    };
    let mut last_mic = PcmChunk {
        samples: vec![0.0; block_samples],
        timestamp_100ns: 0,
    };

    while !stop.load(std::sync::atomic::Ordering::SeqCst) {
        if let Some(rx) = &system_rx {
            while let Ok(chunk) = rx.try_recv() {
                last_sys = chunk;
            }
        }
        if let Some(rx) = &mic_rx {
            while let Ok(chunk) = rx.try_recv() {
                last_mic = chunk;
            }
        }

        let sys_gain = if system_rx.is_some() { 0.85 } else { 0.0 };
        let mic_gain = if mic_rx.is_some() { 1.0 } else { 0.0 };

        let len = block_samples;
        let mut mixed = vec![0.0f32; len];
        for i in 0..len {
            let s = last_sys.samples.get(i).copied().unwrap_or(0.0) * sys_gain;
            let m = last_mic.samples.get(i).copied().unwrap_or(0.0) * mic_gain;
            mixed[i] = (s + m).clamp(-1.0, 1.0);
        }

        let ts = crate::capture::monitor::recording_clock_100ns() - session_start_100ns;
        if out_tx
            .send(PcmChunk {
                samples: mixed,
                timestamp_100ns: ts,
            })
            .is_err()
        {
            break;
        }

        thread::sleep(Duration::from_millis(20));
    }
}

pub fn sample_rate() -> u32 {
    SAMPLE_RATE
}

pub fn channels() -> u16 {
    CHANNELS
}

pub type SharedMicName = Arc<Mutex<Option<String>>>;
