pub use crate::capture::desktop_duplication::{
    DuplicationCaptureHandle as MonitorCaptureHandle,
    start_desktop_duplication_capture as start_monitor_capture,
};

pub fn recording_clock_100ns() -> i64 {
    unsafe {
        let mut freq = 0i64;
        let mut counter = 0i64;
        windows::Win32::System::Performance::QueryPerformanceFrequency(&mut freq).ok();
        windows::Win32::System::Performance::QueryPerformanceCounter(&mut counter).ok();
        if freq > 0 {
            return (counter * 10_000_000) / freq;
        }
    }
    0
}
