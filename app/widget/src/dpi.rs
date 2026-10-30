use windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, GetThreadDpiAwarenessContext, SetProcessDpiAwareness,
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, DPI_AWARENESS_CONTEXT_SYSTEM_AWARE,
    PROCESS_PER_MONITOR_DPI_AWARE,
};
use windows::Win32::UI::WindowsAndMessaging::SetProcessDPIAware;

/// Must run before the event loop creates any window. Prefers per-monitor v2, then the
/// shcore per-monitor mode, then the legacy system-DPI fallback for older Windows 10.
pub fn enable_per_monitor_v2() -> Result<(), String> {
    // SAFETY: these calls only change the process-wide DPI awareness before any window exists.
    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_ok() {
            return Ok(());
        }
        if SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE).is_ok() {
            return Ok(());
        }
        if SetProcessDPIAware().as_bool() {
            return Ok(());
        }
    }
    Err("Windows refused per-monitor DPI awareness".to_string())
}

pub fn awareness_label() -> &'static str {
    // SAFETY: this only reads the calling thread's DPI awareness context.
    unsafe {
        let current = GetThreadDpiAwarenessContext();
        if AreDpiAwarenessContextsEqual(current, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
            .as_bool()
        {
            "per_monitor_v2"
        } else if AreDpiAwarenessContextsEqual(current, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE)
            .as_bool()
        {
            "per_monitor"
        } else if AreDpiAwarenessContextsEqual(current, DPI_AWARENESS_CONTEXT_SYSTEM_AWARE)
            .as_bool()
        {
            "system"
        } else {
            "unknown"
        }
    }
}

/// OS scale (100% = 1.0) combined with the user's global scale (50-200%).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    pub os: f64,
    pub user_percent: u32,
}

impl Scale {
    pub fn new(os: f64, user_percent: u32) -> Self {
        Self {
            os: if os.is_finite() {
                os.clamp(1.0, 4.0)
            } else {
                1.0
            },
            user_percent: user_percent.clamp(50, 200),
        }
    }

    pub fn effective(self) -> f64 {
        self.os * self.user_percent as f64 / 100.0
    }

    #[cfg(test)]
    pub fn px(self, logical: f64) -> f64 {
        logical * self.effective()
    }

    #[cfg(test)]
    pub fn px_i32(self, logical: f64) -> i32 {
        self.px(logical).round() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_multiplies_os_and_user_values() {
        assert_eq!(Scale::new(1.0, 100).effective(), 1.0);
        assert_eq!(Scale::new(1.5, 100).effective(), 1.5);
        assert_eq!(Scale::new(2.0, 100).effective(), 2.0);
        assert_eq!(Scale::new(1.5, 150).effective(), 2.25);
        assert_eq!(Scale::new(1.0, 50).effective(), 0.5);
        assert_eq!(Scale::new(1.0, 200).effective(), 2.0);
    }

    #[test]
    fn pixels_are_rounded_at_the_boundary() {
        let scale = Scale::new(1.5, 100);
        assert_eq!(scale.px(360.0), 540.0);
        assert_eq!(scale.px_i32(17.0), 26);
        assert_eq!(Scale::new(2.0, 100).px_i32(1.0), 2);
    }

    #[test]
    fn invalid_values_are_clamped() {
        assert_eq!(Scale::new(f64::NAN, 100).os, 1.0);
        assert_eq!(Scale::new(9.0, 100).os, 4.0);
        assert_eq!(Scale::new(1.0, 10).effective(), 0.5);
        assert_eq!(Scale::new(1.0, 999).effective(), 2.0);
    }
}
