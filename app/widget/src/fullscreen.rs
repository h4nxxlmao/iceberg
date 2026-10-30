use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetClassNameW, GetForegroundWindow, GetMessageW, GetWindowRect,
    IsWindowVisible, TranslateMessage, EVENT_SYSTEM_FOREGROUND, MSG, WINEVENT_OUTOFCONTEXT,
};
use winit::event_loop::EventLoopProxy;

use crate::app::AppEvent;

static PROXY: OnceLock<Mutex<Option<EventLoopProxy<AppEvent>>>> = OnceLock::new();

pub fn start(proxy: EventLoopProxy<AppEvent>) {
    let slot = PROXY.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(proxy);
    }
    std::thread::Builder::new()
        .name("iceberg-fullscreen".into())
        .spawn(|| {
            // SAFETY: the callback is a static function and the hook is removed after the
            // thread's message loop ends.
            unsafe {
                let hook = SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    None,
                    Some(foreground_changed),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
                if hook.is_invalid() {
                    return;
                }
                let mut message = MSG::default();
                while GetMessageW(&mut message, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                let _ = UnhookWinEvent(hook);
            }
        })
        .ok();
}

pub fn foreground_is_fullscreen() -> bool {
    // SAFETY: all calls only read desktop geometry and window metadata.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() || !IsWindowVisible(hwnd).as_bool() {
            return false;
        }
        let mut class = [0u16; 128];
        let length = GetClassNameW(hwnd, &mut class);
        if length > 0 {
            let name = String::from_utf16_lossy(&class[..length as usize]);
            if matches!(
                name.as_str(),
                "Progman"
                    | "WorkerW"
                    | "Shell_TrayWnd"
                    | "Shell_SecondaryTrayWnd"
                    | "Windows.UI.Core.CoreWindow"
                    | "MultitaskingViewFrame"
            ) {
                return false;
            }
        }
        let mut rect = Default::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return false;
        }
        let screen = info.rcMonitor;
        rect.left <= screen.left + 2
            && rect.top <= screen.top + 2
            && rect.right >= screen.right - 2
            && rect.bottom >= screen.bottom - 2
    }
}

unsafe extern "system" fn foreground_changed(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    let fullscreen = foreground_is_fullscreen();
    if let Some(slot) = PROXY.get() {
        if let Ok(guard) = slot.lock() {
            if let Some(proxy) = guard.as_ref() {
                let _ = proxy.send_event(AppEvent::Fullscreen(fullscreen));
            }
        }
    }
}
