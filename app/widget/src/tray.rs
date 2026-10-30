use std::sync::{Mutex, OnceLock};

use iceberg_core::config::Hotkeys;
use iceberg_core::hotkeys;
use iceberg_core::paths::assets_dir;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, GetCursorPos, LoadIconW, LoadImageW, PostMessageW, RegisterClassW,
    SetForegroundWindow, TrackPopupMenu, HICON, HWND_MESSAGE, IDI_APPLICATION, IMAGE_ICON,
    LR_LOADFROMFILE, MF_SEPARATOR, MF_STRING, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP, WM_COMMAND,
    WM_HOTKEY, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
};
use winit::event_loop::EventLoopProxy;

use crate::app::AppEvent;

const TRAY_ID: u32 = 1;
const TRAY_CALLBACK: u32 = WM_APP + 1;
const MENU_TOGGLE: usize = 1;
const MENU_SETTINGS: usize = 2;
const MENU_CLICK_THROUGH: usize = 3;
const MENU_CYCLE_MODE: usize = 4;
const MENU_QUIT: usize = 5;
const HOTKEY_TOGGLE: i32 = 101;
const HOTKEY_CLICK_THROUGH: i32 = 102;
const HOTKEY_OFFSET_UP: i32 = 103;
const HOTKEY_OFFSET_DOWN: i32 = 104;
const HOTKEY_MODE: i32 = 105;
const HOTKEY_PEEK: i32 = 106;
const HOTKEY_SETTINGS: i32 = 107;
const CLASS_NAME: PCWSTR = w!("IcebergTrayWindow");

#[derive(Clone, Copy, Debug)]
pub enum TrayAction {
    ToggleVisibility,
    Settings,
    ClickThrough,
    CycleMode,
    Hotkey(HotkeyAction),
    Quit,
}

#[derive(Clone, Copy, Debug)]
pub enum HotkeyAction {
    Toggle,
    ClickThrough,
    OffsetUp,
    OffsetDown,
    Mode,
    Peek,
    Settings,
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
    hotkeys: Vec<i32>,
    icon: HICON,
    owns_icon: bool,
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
            let (icon, owns_icon) = load_tray_icon();
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
                if owns_icon {
                    let _ = DestroyIcon(icon);
                }
                return Err("Could not add the tray icon".to_string());
            }
            Ok(Self {
                hwnd,
                data,
                hotkeys: Vec::new(),
                icon,
                owns_icon,
            })
        }
    }

    pub fn set_hotkeys(&mut self, hotkeys: &Hotkeys) {
        // SAFETY: the hotkeys are registered against this tray's live message window.
        unsafe {
            for id in self.hotkeys.drain(..) {
                let _ = UnregisterHotKey(Some(self.hwnd), id);
            }
            let definitions = [
                (HOTKEY_TOGGLE, &hotkeys.toggle),
                (HOTKEY_CLICK_THROUGH, &hotkeys.click_through),
                (HOTKEY_OFFSET_UP, &hotkeys.offset_up),
                (HOTKEY_OFFSET_DOWN, &hotkeys.offset_down),
                (HOTKEY_MODE, &hotkeys.mode),
                (HOTKEY_PEEK, &hotkeys.peek),
                (HOTKEY_SETTINGS, &hotkeys.settings),
            ];
            for (id, value) in definitions {
                if value.trim().is_empty() {
                    continue;
                }
                let Ok(parsed) = hotkeys::parse(value) else {
                    eprintln!("iceberg: invalid hotkey '{value}'");
                    continue;
                };
                let mut modifiers = MOD_NOREPEAT.0;
                if parsed.ctrl {
                    modifiers |= MOD_CONTROL.0;
                }
                if parsed.alt {
                    modifiers |= MOD_ALT.0;
                }
                if parsed.shift {
                    modifiers |= MOD_SHIFT.0;
                }
                if parsed.win {
                    modifiers |= MOD_WIN.0;
                }
                let virtual_key = parsed.key.virtual_key();
                if virtual_key == 0 {
                    eprintln!("iceberg: unsupported hotkey key '{value}'");
                    continue;
                }
                if RegisterHotKey(
                    Some(self.hwnd),
                    id,
                    HOT_KEY_MODIFIERS(modifiers),
                    virtual_key,
                )
                .is_ok()
                {
                    self.hotkeys.push(id);
                } else {
                    eprintln!("iceberg: hotkey '{value}' is already in use");
                }
            }
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        // SAFETY: the icon, window, and hotkeys were created in new()/set_hotkeys().
        unsafe {
            for id in self.hotkeys.drain(..) {
                let _ = UnregisterHotKey(Some(self.hwnd), id);
            }
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.data);
            let _ = DestroyWindow(self.hwnd);
            if self.owns_icon {
                let _ = DestroyIcon(self.icon);
            }
        }
        if let Some(slot) = PROXY.get() {
            if let Ok(mut guard) = slot.lock() {
                *guard = None;
            }
        }
    }
}

fn load_tray_icon() -> (HICON, bool) {
    let path = assets_dir().join("icons").join("iceberg.ico");
    let wide: Vec<u16> = path.as_os_str().to_string_lossy().encode_utf16().collect();
    if !wide.is_empty() {
        let mut terminated = wide;
        terminated.push(0);
        // SAFETY: loading an icon from an existing file does not retain the path pointer.
        if let Ok(handle) = unsafe {
            LoadImageW(
                None,
                PCWSTR(terminated.as_ptr()),
                IMAGE_ICON,
                16,
                16,
                LR_LOADFROMFILE,
            )
        } {
            return (HICON(handle.0), true);
        }
    }
    let fallback = unsafe { LoadIconW(None, IDI_APPLICATION) }.unwrap_or_default();
    (fallback, false)
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
                MENU_SETTINGS => send(TrayAction::Settings),
                MENU_CLICK_THROUGH => send(TrayAction::ClickThrough),
                MENU_CYCLE_MODE => send(TrayAction::CycleMode),
                MENU_QUIT => send(TrayAction::Quit),
                _ => {}
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            if let Some(action) = hotkey_action(wparam.0 as i32) {
                send(TrayAction::Hotkey(action));
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

fn hotkey_action(id: i32) -> Option<HotkeyAction> {
    match id {
        HOTKEY_TOGGLE => Some(HotkeyAction::Toggle),
        HOTKEY_CLICK_THROUGH => Some(HotkeyAction::ClickThrough),
        HOTKEY_OFFSET_UP => Some(HotkeyAction::OffsetUp),
        HOTKEY_OFFSET_DOWN => Some(HotkeyAction::OffsetDown),
        HOTKEY_MODE => Some(HotkeyAction::Mode),
        HOTKEY_PEEK => Some(HotkeyAction::Peek),
        HOTKEY_SETTINGS => Some(HotkeyAction::Settings),
        _ => None,
    }
}

unsafe fn show_menu(hwnd: HWND) {
    let Ok(menu) = CreatePopupMenu() else {
        return;
    };
    let _ = AppendMenuW(menu, MF_STRING, MENU_TOGGLE, w!("Show / hide"));
    let _ = AppendMenuW(menu, MF_STRING, MENU_SETTINGS, w!("Settings"));
    let _ = AppendMenuW(menu, MF_STRING, MENU_CLICK_THROUGH, w!("Click-through"));
    let _ = AppendMenuW(menu, MF_STRING, MENU_CYCLE_MODE, w!("Cycle mode"));
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
        MENU_SETTINGS => send(TrayAction::Settings),
        MENU_CLICK_THROUGH => send(TrayAction::ClickThrough),
        MENU_CYCLE_MODE => send(TrayAction::CycleMode),
        MENU_QUIT => send(TrayAction::Quit),
        _ => {}
    }
}
