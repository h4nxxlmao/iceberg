use std::sync::{Mutex, OnceLock};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    GetCursorPos, LoadIconW, PostMessageW, RegisterClassW, SetForegroundWindow, TrackPopupMenu,
    HWND_MESSAGE, IDI_APPLICATION, MF_SEPARATOR, MF_STRING, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP,
    WM_COMMAND, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
};
use winit::event_loop::EventLoopProxy;

use crate::app::AppEvent;

const TRAY_ID: u32 = 1;
const TRAY_CALLBACK: u32 = WM_APP + 1;
const MENU_TOGGLE: usize = 1;
const MENU_QUIT: usize = 2;
const CLASS_NAME: PCWSTR = w!("IcebergTrayWindow");

#[derive(Clone, Copy, Debug)]
pub enum TrayAction {
    ToggleVisibility,
    Quit,
}

static PROXY: OnceLock<Mutex<Option<EventLoopProxy<AppEvent>>>> = OnceLock::new();

fn send(action: TrayAction) {
    let slot = PROXY.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = slot.lock() {
        if let Some(proxy) = guard.as_ref() {
            let _ = proxy.send_event(AppEvent::Tray(action));
        }
    }
}

pub struct Tray {
    hwnd: HWND,
    data: NOTIFYICONDATAW,
}

impl Tray {
    pub fn new(proxy: EventLoopProxy<AppEvent>) -> Result<Self, String> {
        let slot = PROXY.get_or_init(|| Mutex::new(None));
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(proxy);
        }
        // SAFETY: the hidden message-only window and shell icon are owned by this struct and
        // created on the event-loop thread.
        unsafe {
            let instance = GetModuleHandleW(None).map_err(|error| error.to_string())?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(tray_proc),
                hInstance: instance.into(),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            let _ = RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                Default::default(),
                CLASS_NAME,
                w!("Iceberg"),
                Default::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|error| error.to_string())?;
            let icon = LoadIconW(None, IDI_APPLICATION).map_err(|error| error.to_string())?;
            let mut data = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: TRAY_ID,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: TRAY_CALLBACK,
                hIcon: icon,
                ..Default::default()
            };
            write_wide(&mut data.szTip, "Iceberg");
            if !Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
                let _ = DestroyWindow(hwnd);
                return Err("Could not add the tray icon".to_string());
            }
            Ok(Self { hwnd, data })
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        // SAFETY: the icon and window were created in new().
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.data);
            let _ = DestroyWindow(self.hwnd);
        }
        if let Some(slot) = PROXY.get() {
            if let Ok(mut guard) = slot.lock() {
                *guard = None;
            }
        }
    }
}

fn write_wide(target: &mut [u16], text: &str) {
    let encoded: Vec<u16> = text.encode_utf16().collect();
    let length = encoded.len().min(target.len().saturating_sub(1));
    target[..length].copy_from_slice(&encoded[..length]);
    target[length] = 0;
}

unsafe extern "system" fn tray_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        TRAY_CALLBACK => {
            let event = (lparam.0 as u32) & 0xFFFF;
            if event == WM_LBUTTONUP {
                send(TrayAction::ToggleVisibility);
            } else if event == WM_RBUTTONUP {
                show_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            match wparam.0 & 0xFFFF {
                MENU_TOGGLE => send(TrayAction::ToggleVisibility),
                MENU_QUIT => send(TrayAction::Quit),
                _ => {}
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn show_menu(hwnd: HWND) {
    let Ok(menu) = CreatePopupMenu() else {
        return;
    };
    let _ = AppendMenuW(menu, MF_STRING, MENU_TOGGLE, w!("Show / hide"));
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    let _ = AppendMenuW(menu, MF_STRING, MENU_QUIT, w!("Quit"));
    let mut point = POINT { x: 0, y: 0 };
    let _ = GetCursorPos(&mut point);
    let _ = SetForegroundWindow(hwnd);
    let command = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_RIGHTBUTTON,
        point.x,
        point.y,
        Some(0),
        hwnd,
        None,
    )
    .0 as usize;
    let _ = DestroyMenu(menu);
    let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
    match command {
        MENU_TOGGLE => send(TrayAction::ToggleVisibility),
        MENU_QUIT => send(TrayAction::Quit),
        _ => {}
    }
}
