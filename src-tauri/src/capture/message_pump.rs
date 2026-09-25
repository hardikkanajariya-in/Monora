use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, PM_REMOVE, TranslateMessage, MSG,
};

/// WinRT Graphics Capture delivers frame events through a thread message queue.
pub fn pump_messages() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
