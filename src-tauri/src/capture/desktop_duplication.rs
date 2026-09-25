use crate::capture::frame::CapturedFrame;
use crate::models::MonitorCaptureTarget;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use windows::core::Interface;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11_BIND_FLAG,
    D3D11_RESOURCE_MISC_FLAG,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication, IDXGIFactory1,
    DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

pub struct DuplicationCaptureHandle {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl DuplicationCaptureHandle {
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn start_desktop_duplication_capture(
    target: MonitorCaptureTarget,
    fps: u32,
    frame_tx: SyncSender<CapturedFrame>,
    session_start_100ns: i64,
) -> Result<DuplicationCaptureHandle, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    let thread = thread::Builder::new()
        .name(format!("dxgi-capture-{}", target.info.index))
        .spawn(move || {
            if let Err(e) = duplication_loop(target, fps, frame_tx, session_start_100ns, stop_flag) {
                crate::logging::error(format!("DXGI capture error: {e}"));
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(DuplicationCaptureHandle {
        stop,
        thread: Some(thread),
    })
}

fn duplication_loop(
    target: MonitorCaptureTarget,
    fps: u32,
    frame_tx: SyncSender<CapturedFrame>,
    session_start_100ns: i64,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let mut device: Option<ID3D11Device> = None;
    unsafe {
        D3D11CreateDevice(
            None,
            windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            None,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
        .map_err(|e| format!("D3D11CreateDevice: {e}"))?;
    }
    let device = device.ok_or("D3D11 device missing")?;
    let context = unsafe {
        device
            .GetImmediateContext()
            .map_err(|e| e.to_string())?
    };

    let duplication = find_duplication_for_monitor(&device, &target)?;
    let mut staging: Option<ID3D11Texture2D> = None;
    let frame_interval = Duration::from_nanos(1_000_000_000 / fps.max(1) as u64);
    let mut last_emit = std::time::Instant::now() - frame_interval;

    while !stop.load(Ordering::SeqCst) {
        let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource = None;
        match unsafe { duplication.AcquireNextFrame(16, &mut frame_info, &mut resource) } {
            Ok(()) => {}
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                thread::sleep(Duration::from_millis(2));
                continue;
            }
            Err(e) => return Err(format!("AcquireNextFrame failed: {e}")),
        }

        let resource = resource.ok_or("Desktop texture missing")?;
        let texture: ID3D11Texture2D = resource.cast().map_err(|e| e.to_string())?;

        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.GetDesc(&mut desc) };

        if staging.is_none() {
            let staging_desc = D3D11_TEXTURE2D_DESC {
                Width: desc.Width,
                Height: desc.Height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: D3D11_BIND_FLAG(0).0 as u32,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: D3D11_RESOURCE_MISC_FLAG(0).0 as u32,
            };
            let mut st = None;
            unsafe {
                device
                    .CreateTexture2D(&staging_desc, None, Some(&mut st))
                    .map_err(|e| format!("CreateTexture2D: {e}"))?;
            }
            staging = st;
        }

        let staging_tex = staging.as_ref().unwrap();
        unsafe {
            context.CopyResource(staging_tex, &texture);
            duplication.ReleaseFrame().map_err(|e| e.to_string())?;
        }

        let now = std::time::Instant::now();
        if now.duration_since(last_emit) < frame_interval {
            continue;
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context
                .Map(staging_tex, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|e| format!("Map: {e}"))?;
        }

        let width = desc.Width;
        let height = desc.Height;
        let row_pitch = mapped.RowPitch as usize;
        let mut bgra = vec![0u8; (width * height * 4) as usize];
        let src = unsafe {
            std::slice::from_raw_parts(mapped.pData as *const u8, row_pitch * height as usize)
        };
        for y in 0..height as usize {
            let src_off = y * row_pitch;
            let dst_off = y * width as usize * 4;
            bgra[dst_off..dst_off + width as usize * 4]
                .copy_from_slice(&src[src_off..src_off + width as usize * 4]);
        }
        unsafe {
            context.Unmap(staging_tex, 0);
        }

        let ts = crate::capture::monitor::recording_clock_100ns() - session_start_100ns;
        let captured = CapturedFrame {
            width,
            height,
            bgra,
            timestamp_100ns: ts,
        };
        match frame_tx.try_send(captured) {
            Ok(()) => {}
            Err(std::sync::mpsc::TrySendError::Full(_)) => {}
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => break,
        }
        last_emit = now;
    }

    Ok(())
}

fn find_duplication_for_monitor(
    device: &ID3D11Device,
    target: &MonitorCaptureTarget,
) -> Result<IDXGIOutputDuplication, String> {
    let factory: IDXGIFactory1 =
        unsafe { CreateDXGIFactory1() }.map_err(|e| e.to_string())?;
    let want = monitor_rect(&target.info);

    let mut adapter_index = 0u32;
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(adapter_index) } {
        let mut output_index = 0u32;
        while let Ok(output) = unsafe { adapter.EnumOutputs(output_index) } {
            let desc = unsafe { output.GetDesc() }.map_err(|e| e.to_string())?;
            if rects_equal(desc.DesktopCoordinates, want) {
                let output1: IDXGIOutput1 = output.cast().map_err(|e| e.to_string())?;
                return unsafe {
                    output1
                        .DuplicateOutput(device)
                        .map_err(|e| format!("DuplicateOutput: {e}"))
                };
            }
            output_index += 1;
        }
        adapter_index += 1;
    }

    Err(format!(
        "No DXGI output found for monitor {} ({}x{} at {}, {})",
        target.info.name,
        target.info.width,
        target.info.height,
        target.info.x,
        target.info.y
    ))
}

fn monitor_rect(info: &crate::models::MonitorInfo) -> RECT {
    RECT {
        left: info.x,
        top: info.y,
        right: info.x + info.width as i32,
        bottom: info.y + info.height as i32,
    }
}

fn rects_equal(a: RECT, b: RECT) -> bool {
    a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom
}
