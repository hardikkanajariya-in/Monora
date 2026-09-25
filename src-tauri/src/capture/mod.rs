pub mod frame;
pub mod message_pump;
pub mod monitor;

use crate::models::{MonitorCaptureTarget, MonitorInfo};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashMap;
use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

static HMONITOR_MAP: Lazy<Mutex<HashMap<String, isize>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    HMONITOR_MAP.lock().clear();
    let mut monitors: Vec<MonitorInfo> = Vec::new();
    let mut index = 0usize;

    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM(std::ptr::from_mut(&mut monitors) as isize),
        );
    }

    // Re-assign indices in enumeration order
    for (i, m) in monitors.iter_mut().enumerate() {
        m.index = i + 1;
        index = i + 1;
    }
    let _ = index;
    monitors
}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: windows::Win32::Graphics::Gdi::HMONITOR,
    _hdc: windows::Win32::Graphics::Gdi::HDC,
    _lprc: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);
    let mut info = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        szDevice: [0; 32],
    };
    if GetMonitorInfoW(hmonitor, &mut info as *mut _ as *mut MONITORINFO).as_bool() {
        let rect = info.monitorInfo.rcMonitor;
        let end = info
            .szDevice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(info.szDevice.len());
        let name = String::from_utf16_lossy(&info.szDevice[..end]);
        let handle = hmonitor.0 as isize;
        let id = format!("monitor:{:p}", hmonitor.0);
        HMONITOR_MAP.lock().insert(id.clone(), handle);

        let primary_x = GetSystemMetrics(SM_CXSCREEN);
        let primary_y = GetSystemMetrics(SM_CYSCREEN);
        let is_primary = info.monitorInfo.dwFlags & 1 != 0;

        let display_name = if name.is_empty() {
            format!("Monitor {}", monitors.len() + 1)
        } else {
            format!("Monitor {} ({})", monitors.len() + 1, name.trim())
        };

        monitors.push(MonitorInfo {
            id,
            name: display_name,
            index: monitors.len() + 1,
            x: rect.left,
            y: rect.top,
            width: (rect.right - rect.left) as u32,
            height: (rect.bottom - rect.top) as u32,
            refresh_rate_hz: None,
            is_primary,
        });

        let _ = (primary_x, primary_y);
    }
    BOOL(1)
}

pub fn resolve_capture_targets(ids: &[String]) -> Result<Vec<MonitorCaptureTarget>, String> {
    let all = enumerate_monitors();
    let map = HMONITOR_MAP.lock().clone();
    let mut out = Vec::new();
    for id in ids {
        let hmon = map.get(id).ok_or_else(|| format!("Unknown monitor: {id}"))?;
        let info = all
            .iter()
            .find(|m| m.id == *id)
            .ok_or_else(|| format!("Monitor info missing: {id}"))?
            .clone();
        out.push(MonitorCaptureTarget {
            info,
            hmonitor: *hmon,
        });
    }
    if out.is_empty() {
        return Err("No monitors selected".into());
    }
    Ok(out)
}
