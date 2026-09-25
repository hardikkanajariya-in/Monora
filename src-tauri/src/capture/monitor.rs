use crate::capture::frame::CapturedFrame;
use crate::logging;
use crate::models::MonitorCaptureTarget;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use windows::core::Interface;
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::GraphicsCaptureItem;
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    D3D11_BIND_FLAG, D3D11_CPU_ACCESS_FLAG, D3D11_RESOURCE_MISC_FLAG,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice;
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;

pub struct MonitorCaptureHandle {
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl MonitorCaptureHandle {
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn start_monitor_capture(
    target: MonitorCaptureTarget,
    fps: u32,
    frame_tx: Sender<CapturedFrame>,
    session_start_100ns: i64,
) -> Result<MonitorCaptureHandle, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    let monitor_id = target.info.id.clone();

    let thread = thread::Builder::new()
        .name(format!("capture-{}", target.info.index))
        .spawn(move || {
            if let Err(e) = capture_loop(target, fps, frame_tx, session_start_100ns, stop_flag) {
                logging::error(format!("Capture error on {monitor_id}: {e}"));
            }
        })
        .map_err(|e| e.to_string())?;

    Ok(MonitorCaptureHandle {
        stop,
        thread: Some(thread),
    })
}

fn capture_loop(
    target: MonitorCaptureTarget,
    fps: u32,
    frame_tx: Sender<CapturedFrame>,
    session_start_100ns: i64,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok();
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
        .map_err(|e| format!("D3D11CreateDevice failed: {e}"))?;
    }
    let device = device.ok_or("D3D11 device missing")?;

    let dxgi_device: IDXGIDevice = device.cast().map_err(|e| e.to_string())?;
    let inspectable = unsafe {
        CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device)
            .map_err(|e| format!("CreateDirect3D11DeviceFromDXGIDevice: {e}"))?
    };
    let d3d_device: IDirect3DDevice = inspectable.cast().map_err(|e| e.to_string())?;

    let item = create_capture_item_for_monitor(target.hmonitor)?;

    let size = item
        .Size()
        .map_err(|e| format!("Capture item size: {e}"))?;
    let frame_pool = windows::Graphics::Capture::Direct3D11CaptureFramePool::Create(
        &d3d_device,
        windows::Graphics::DirectX::DirectXPixelFormat::B8G8R8A8UIntNormalized,
        2,
        size,
    )
    .map_err(|e| format!("Create frame pool: {e}"))?;

    let session = frame_pool
        .CreateCaptureSession(&item)
        .map_err(|e| format!("CreateCaptureSession: {e}"))?;
    session.StartCapture().map_err(|e| format!("StartCapture: {e}"))?;

    let (notify_tx, notify_rx) = std::sync::mpsc::channel::<()>();
    let frame_pool_ref = frame_pool.clone();
    let _token = frame_pool
        .FrameArrived(
            &TypedEventHandler::new(move |pool, _| {
                let _ = pool;
                let _ = notify_tx.send(());
                Ok(())
            }),
        )
        .map_err(|e| format!("FrameArrived handler: {e}"))?;

    let frame_interval = Duration::from_nanos(1_000_000_000 / fps.max(1) as u64);
    let mut last_emit = std::time::Instant::now() - frame_interval;
    let mut staging: Option<ID3D11Texture2D> = None;

    while !stop.load(Ordering::SeqCst) {
        let _ = notify_rx.recv_timeout(Duration::from_millis(50));
        if stop.load(Ordering::SeqCst) {
            break;
        }

        let now = std::time::Instant::now();
        if now.duration_since(last_emit) < frame_interval {
            continue;
        }

        let frame = match frame_pool_ref.TryGetNextFrame() {
            Ok(f) => f,
            Err(_) => continue,
        };

        let surface = frame
            .Surface()
            .map_err(|e| format!("Surface: {e}"))?;
        let access: IDirect3DDxgiInterfaceAccess = surface
            .cast()
            .map_err(|e| format!("IDirect3DDxgiInterfaceAccess: {e}"))?;
        let texture: ID3D11Texture2D = unsafe {
            access
                .GetInterface()
                .map_err(|e| format!("GetInterface texture: {e}"))?
        };

        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.GetDesc(&mut desc) };

        if staging.is_none()
            || {
                let s = staging.as_ref().unwrap();
                let mut sd = D3D11_TEXTURE2D_DESC::default();
                unsafe { s.GetDesc(&mut sd) };
                sd.Width != desc.Width || sd.Height != desc.Height
            }
        {
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
            let mut st: Option<ID3D11Texture2D> = None;
            unsafe {
                device
                    .CreateTexture2D(&staging_desc, None, Some(&mut st))
                    .map_err(|e| format!("CreateTexture2D staging: {e}"))?;
            }
            staging = st;
        }

        let staging_tex = staging.as_ref().unwrap();
        let context = unsafe {
            device
                .GetImmediateContext()
                .map_err(|e| e.to_string())?
        };

        unsafe {
            context.CopyResource(staging_tex, &texture);
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context
                .Map(staging_tex, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|e| format!("Map staging: {e}"))?;
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

        let elapsed = session_elapsed_100ns(session_start_100ns);
        let captured = CapturedFrame {
            width,
            height,
            bgra,
            timestamp_100ns: elapsed,
        };
        if frame_tx.send(captured).is_err() {
            break;
        }
        last_emit = now;
    }

    session.Close().ok();
    frame_pool.Close().ok();
    Ok(())
}

fn create_capture_item_for_monitor(hmonitor: isize) -> Result<GraphicsCaptureItem, String> {
    let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
        .map_err(|e| format!("GraphicsCaptureItem factory: {e}"))?;
    let hmon = HMONITOR(hmonitor as *mut std::ffi::c_void);
    let item = unsafe {
        interop
            .CreateForMonitor(hmon)
            .map_err(|e| format!("CreateForMonitor: {e}"))?
    };
    Ok(item)
}

fn session_elapsed_100ns(session_start_100ns: i64) -> i64 {
    let now = recording_clock_100ns();
    now - session_start_100ns
}

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
