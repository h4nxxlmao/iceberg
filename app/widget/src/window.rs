use std::ffi::c_void;

use iceberg_core::config::Anchor;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use serde::Serialize;
use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW,
    MonitorFromWindow, ReleaseDC, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HGDIOBJ, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, IsWindowVisible,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, UpdateLayeredWindow, GWL_EXSTYLE, GWL_STYLE,
    HWND_TOPMOST, SM_CXSCREEN, SM_CYSCREEN, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WS_CAPTION,
    WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
};
use winit::window::Window;

pub fn hwnd_of(window: &Window) -> HWND {
    match window.window_handle().expect("window handle").as_raw() {
        RawWindowHandle::Win32(handle) => HWND(handle.hwnd.get() as *mut _),
        other => panic!("unexpected window handle: {other:?}"),
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct StyleReport {
    pub style: u32,
    pub ex_style: u32,
    pub borderless: bool,
    pub tool_window: bool,
    pub no_activate: bool,
    pub topmost: bool,
    pub layered: bool,
    pub app_window: bool,
    pub click_through: bool,
    pub visible: bool,
    pub foreground: bool,
}

pub fn apply_styles(hwnd: HWND, click_through: bool) -> StyleReport {
    // SAFETY: the handle is a live top-level window created by this process. Style bits are
    // applied explicitly rather than relying on winit defaults.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let style = (style
            & !(WS_CAPTION.0
                | WS_THICKFRAME.0
                | WS_MINIMIZEBOX.0
                | WS_MAXIMIZEBOX.0
                | WS_SYSMENU.0))
            | WS_POPUP.0;
        SetWindowLongPtrW(hwnd, GWL_STYLE, style as isize);

        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let mut ex =
            (ex | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0 | WS_EX_TOPMOST.0 | WS_EX_LAYERED.0)
                & !WS_EX_APPWINDOW.0;
        if click_through {
            ex |= WS_EX_TRANSPARENT.0;
        } else {
            ex &= !WS_EX_TRANSPARENT.0;
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex as isize);

        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
    report(hwnd)
}

pub fn set_click_through(hwnd: HWND, enabled: bool) -> StyleReport {
    // SAFETY: the handle is a live window owned by this process.
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let ex = if enabled {
            ex | WS_EX_TRANSPARENT.0
        } else {
            ex & !WS_EX_TRANSPARENT.0
        };
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex as isize);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
    report(hwnd)
}

pub fn report(hwnd: HWND) -> StyleReport {
    // SAFETY: reading styles and window state is safe for a live handle.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        StyleReport {
            style,
            ex_style,
            borderless: style & WS_CAPTION.0 == 0,
            tool_window: ex_style & WS_EX_TOOLWINDOW.0 != 0,
            no_activate: ex_style & WS_EX_NOACTIVATE.0 != 0,
            topmost: ex_style & WS_EX_TOPMOST.0 != 0,
            layered: ex_style & WS_EX_LAYERED.0 != 0,
            app_window: ex_style & WS_EX_APPWINDOW.0 != 0,
            click_through: ex_style & WS_EX_TRANSPARENT.0 != 0,
            visible: IsWindowVisible(hwnd).as_bool(),
            foreground: GetForegroundWindow() == hwnd,
        }
    }
}

pub fn show_no_activate(hwnd: HWND) {
    // SAFETY: the handle is a live window; SW_SHOWNOACTIVATE never takes focus.
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

pub fn hide(hwnd: HWND) {
    // SAFETY: the handle is a live window owned by this process.
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

pub fn place(hwnd: HWND, x: i32, y: i32, width: i32, height: i32) {
    // SAFETY: the handle is live and the requested geometry is in physical pixels.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

pub fn resize(hwnd: HWND, width: i32, height: i32) {
    // SAFETY: the handle is live; only the size changes and z-order is preserved.
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            width.max(1),
            height.max(1),
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkArea {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl WorkArea {
    pub fn width(self) -> i32 {
        (self.right - self.left).max(1)
    }

    pub fn height(self) -> i32 {
        (self.bottom - self.top).max(1)
    }
}

pub fn work_area(hwnd: HWND) -> WorkArea {
    // SAFETY: the window and monitor queries only read geometry.
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            return WorkArea {
                left: info.rcWork.left,
                top: info.rcWork.top,
                right: info.rcWork.right,
                bottom: info.rcWork.bottom,
            };
        }
        WorkArea {
            left: 0,
            top: 0,
            right: GetSystemMetrics(SM_CXSCREEN).max(1),
            bottom: GetSystemMetrics(SM_CYSCREEN).max(1),
        }
    }
}

/// Snaps a dragged window to the nearest work-area edge when it is close, and
/// keeps it from being left completely outside the work area.
pub fn snap_position(
    work: WorkArea,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    threshold: i32,
) -> (i32, i32) {
    let max_x = (work.right - width).max(work.left);
    let max_y = (work.bottom - height).max(work.top);
    let x = if (x - work.left).abs() <= threshold {
        work.left
    } else if (work.right - (x + width)).abs() <= threshold {
        work.right - width
    } else {
        x.clamp(work.left, max_x)
    };
    let y = if (y - work.top).abs() <= threshold {
        work.top
    } else if (work.bottom - (y + height)).abs() <= threshold {
        work.bottom - height
    } else {
        y.clamp(work.top, max_y)
    };
    (x, y)
}

/// Pure positioning so DPI and anchor behavior can be unit tested. `offset_*` and `margin`
/// are already in physical pixels; `offset_y` is measured from the card edge, not the shadow.
#[allow(clippy::too_many_arguments)]
pub fn anchored_position(
    work: WorkArea,
    window_w: i32,
    window_h: i32,
    card_w: i32,
    card_h: i32,
    margin: i32,
    anchor: Anchor,
    offset_x: i32,
    offset_y: i32,
) -> (i32, i32) {
    let centered_x = work.left + (work.width() - window_w) / 2 + offset_x;
    let centered_y = work.top + (work.height() - window_h) / 2 + offset_y;
    let (x, y) = match anchor {
        Anchor::TopLeft => (work.left + offset_x - margin, work.top + offset_y - margin),
        Anchor::TopCenter => (centered_x, work.top + offset_y - margin),
        Anchor::TopRight => (
            work.right - card_w - offset_x - margin,
            work.top + offset_y - margin,
        ),
        Anchor::CenterLeft => (work.left + offset_x - margin, centered_y),
        Anchor::Center => (centered_x, centered_y),
        Anchor::CenterRight => (work.right - card_w - offset_x - margin, centered_y),
        Anchor::BottomLeft => (
            work.left + offset_x - margin,
            work.bottom - card_h - offset_y - margin,
        ),
        Anchor::BottomCenter => (centered_x, work.bottom - card_h - offset_y - margin),
        Anchor::BottomRight => (
            work.right - card_w - offset_x - margin,
            work.bottom - card_h - offset_y - margin,
        ),
    };
    clamp_position(work, x, y, card_w, card_h)
}

fn clamp_position(work: WorkArea, x: i32, y: i32, card_w: i32, card_h: i32) -> (i32, i32) {
    let visible_x = 60.min(card_w.max(1));
    let visible_y = 40.min(card_h.max(1));
    let min_x = work.left - card_w + visible_x;
    let max_x = work.right - visible_x;
    let min_y = work.top - card_h + visible_y;
    let max_y = work.bottom - visible_y;
    (
        x.clamp(min_x.min(max_x), max_x.max(min_x)),
        y.clamp(min_y.min(max_y), max_y.max(min_y)),
    )
}

pub struct LayeredSurface {
    pub width: i32,
    pub height: i32,
    memory_dc: windows::Win32::Graphics::Gdi::HDC,
    bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
}

impl LayeredSurface {
    pub fn new(width: i32, height: i32) -> Result<Self, String> {
        if width <= 0 || height <= 0 {
            return Err("The widget surface has an invalid size".to_string());
        }
        // SAFETY: all GDI objects are created and owned by the returned struct.
        unsafe {
            let screen = GetDC(None);
            if screen.0.is_null() {
                return Err("Could not read the screen device context".to_string());
            }
            let memory_dc = CreateCompatibleDC(Some(screen));
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            let bitmap =
                match CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
                    Ok(bitmap) => bitmap,
                    Err(error) => {
                        let _ = DeleteDC(memory_dc);
                        let _ = ReleaseDC(None, screen);
                        return Err(format!("Could not create the widget bitmap: {error}"));
                    }
                };
            let old = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
            let _ = ReleaseDC(None, screen);
            Ok(Self {
                width,
                height,
                memory_dc,
                bitmap,
                old,
                bits: bits as *mut u8,
            })
        }
    }

    pub fn resize(&mut self, width: i32, height: i32) -> Result<(), String> {
        if self.width == width && self.height == height {
            return Ok(());
        }
        let next = Self::new(width, height)?;
        *self = next;
        Ok(())
    }

    pub fn pixels_mut(&mut self) -> &mut [u8] {
        // SAFETY: the DIB owns width * height * 4 bytes for the lifetime of this struct.
        unsafe {
            std::slice::from_raw_parts_mut(self.bits, (self.width * self.height * 4) as usize)
        }
    }

    pub fn present(&self, hwnd: HWND) {
        // SAFETY: the DIB and memory DC are live and sized for the window.
        unsafe {
            let mut rect = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rect);
            let destination = POINT {
                x: rect.left,
                y: rect.top,
            };
            let size = SIZE {
                cx: self.width,
                cy: self.height,
            };
            let source = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let screen = GetDC(None);
            let _ = UpdateLayeredWindow(
                hwnd,
                Some(screen),
                Some(&destination),
                Some(&size),
                Some(self.memory_dc),
                Some(&source),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
            let _ = ReleaseDC(None, screen);
        }
    }
}

impl Drop for LayeredSurface {
    fn drop(&mut self) {
        // SAFETY: these objects were created in new() and are no longer used.
        unsafe {
            SelectObject(self.memory_dc, self.old);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.memory_dc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: WorkArea = WorkArea {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };

    #[test]
    fn snap_position_snaps_to_the_near_edge() {
        assert_eq!(snap_position(WORK, 8, 300, 388, 245, 16), (0, 300));
        assert_eq!(snap_position(WORK, 1524, 300, 388, 245, 16), (1532, 300));
        assert_eq!(snap_position(WORK, 300, 827, 388, 245, 16), (300, 835));
    }

    #[test]
    fn snap_position_keeps_far_windows_in_the_work_area() {
        assert_eq!(snap_position(WORK, 700, 300, 388, 245, 16), (700, 300));
        assert_eq!(snap_position(WORK, -500, -500, 388, 245, 16), (0, 0));
    }

    #[test]
    fn top_center_accounts_for_the_shadow_margin() {
        let (x, y) = anchored_position(WORK, 384, 240, 360, 216, 12, Anchor::TopCenter, 0, 24);
        assert_eq!(x, 768);
        assert_eq!(y, 12);
    }

    #[test]
    fn center_is_true_center() {
        let (x, y) = anchored_position(WORK, 384, 240, 360, 216, 12, Anchor::Center, 0, 0);
        assert_eq!(x, 768);
        assert_eq!(y, 420);
    }

    #[test]
    fn bottom_right_uses_the_card_edge() {
        let (x, y) = anchored_position(WORK, 384, 240, 360, 216, 12, Anchor::BottomRight, 0, 0);
        assert_eq!(x, 1548);
        assert_eq!(y, 852);
    }

    #[test]
    fn huge_offsets_keep_some_of_the_widget_visible() {
        let (x, y) = anchored_position(
            WORK,
            384,
            240,
            360,
            216,
            12,
            Anchor::TopLeft,
            -10_000,
            -10_000,
        );
        assert!(x >= WORK.left - 360 + 60);
        assert!(y >= WORK.top - 216 + 40);
        assert!(x + 360 > WORK.left);
        assert!(y + 216 > WORK.top);
    }

    #[test]
    fn dpi_scale_changes_positions_through_the_caller() {
        // The caller multiplies offsets by the effective scale; this test documents the
        // physical-pixel contract of the positioning function.
        let (x, y) = anchored_position(WORK, 576, 360, 540, 324, 18, Anchor::TopCenter, 0, 36);
        assert_eq!(x, (1920 - 576) / 2);
        assert_eq!(y, 18);
    }
}
