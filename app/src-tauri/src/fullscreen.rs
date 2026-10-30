use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::OnceLock;
use std::thread;

use tauri::AppHandle;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Accessibility::SetWinEventHook;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetClassNameW, GetForegroundWindow, GetMessageW, GetShellWindow,
    GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindowVisible, TranslateMessage,
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_SYSTEM_FOREGROUND, MSG, WINEVENT_OUTOFCONTEXT,
};

use crate::util::now_ms;

static APP: OnceLock<AppHandle> = OnceLock::new();
static LAST_LOCATION_CHECK: AtomicI64 = AtomicI64::new(0);

pub fn start(app: &AppHandle) {
    if APP.set(app.clone()).is_err() {
        return;
    }
    thread::Builder::new()
        .name("iceberg-fullscreen".into())
        .spawn(|| {
            // SAFETY: this thread owns the message queue used by the out-of-context hooks.
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let callback: windows::Win32::UI::Accessibility::WINEVENTPROC = Some(win_event);
                let _foreground = SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    None,
                    callback,
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
                let _location = SetWinEventHook(
                    EVENT_OBJECT_LOCATIONCHANGE,
                    EVENT_OBJECT_LOCATIONCHANGE,
                    None,
                    callback,
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
                evaluate(GetForegroundWindow());

                let mut message = MSG::default();
                while GetMessageW(&mut message, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        })
        .ok();
}

// SAFETY: the hooks are registered out-of-context on the thread running the message loop,
// and every Win32 call made here follows its documented preconditions.
unsafe extern "system" fn win_event(
    _hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if event == EVENT_OBJECT_LOCATIONCHANGE {
        if id_object != 0 || id_child != 0 {
            return;
        }
        let foreground = GetForegroundWindow();
        if foreground != hwnd {
            return;
        }
        let now = now_ms();
        let previous = LAST_LOCATION_CHECK.swap(now, Ordering::Relaxed);
        if now.saturating_sub(previous) < 400 {
            return;
        }
    }
    let target = if hwnd.is_invalid() {
        GetForegroundWindow()
    } else {
        hwnd
    };
    evaluate(target);
}

fn evaluate(hwnd: HWND) {
    let Some(app) = APP.get() else {
        return;
    };
    // SAFETY: is_fullscreen only reads window and monitor geometry for the given handle.
    let fullscreen = unsafe { is_fullscreen(hwnd) };
    crate::window::set_fullscreen(app, fullscreen);
}

unsafe fn is_fullscreen(hwnd: HWND) -> bool {
    if hwnd.is_invalid() || hwnd == GetShellWindow() {
        return false;
    }
    if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
        return false;
    }
    let mut process_id = 0u32;
    let _ = GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    if process_id == std::process::id() {
        return false;
    }

    let mut class_buffer = [0u16; 64];
    let class_len = GetClassNameW(hwnd, &mut class_buffer);
    if class_len > 0 {
        let class = String::from_utf16_lossy(&class_buffer[..class_len as usize]);
        if matches!(
            class.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "NotifyIconOverflowWindow"
        ) {
            return false;
        }
    }

    let mut rect = RECT::default();
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
    let area = info.rcMonitor;
    rect.left <= area.left + 2
        && rect.top <= area.top + 2
        && rect.right >= area.right - 2
        && rect.bottom >= area.bottom - 2
}
