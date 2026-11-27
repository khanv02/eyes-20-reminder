//! Reads OS idle seconds. No input hooks, no keylogging: only the counter the
//! OS already keeps.

#[cfg(windows)]
mod platform {
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect};

    pub fn idle_seconds() -> u64 {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        // SAFETY: info is a valid LASTINPUTINFO with cbSize set.
        if unsafe { GetLastInputInfo(&mut info) } == 0 {
            return 0;
        }
        let now = unsafe { GetTickCount() };
        u64::from(now.wrapping_sub(info.dwTime)) / 1000
    }

    /// True when the foreground window covers its whole monitor (game, video,
    /// fullscreen presentation). Screenshare detection is not covered.
    pub fn fullscreen_foreground() -> bool {
        // SAFETY: handles are read-only queries.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return false;
            }
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect) == 0 {
                return false;
            }
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let Some(info) = monitor_info(monitor) else { return false };
            let covers_monitor = rect.left <= info.rcMonitor.left
                && rect.top <= info.rcMonitor.top
                && rect.right >= info.rcMonitor.right
                && rect.bottom >= info.rcMonitor.bottom;
            let covers_work_area = rect.left <= info.rcWork.left
                && rect.top <= info.rcWork.top
                && rect.right >= info.rcWork.right
                && rect.bottom >= info.rcWork.bottom;
            // The desktop/shell also covers the monitor; don't treat it as fullscreen.
            covers_monitor && !covers_work_area
        }
    }

    unsafe fn monitor_info(monitor: *mut core::ffi::c_void) -> Option<MONITORINFO> {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        // SAFETY: info has cbSize set and monitor came from the OS.
        if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
            None
        } else {
            Some(info)
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: u32, event_type: u32) -> f64;
    }

    pub fn idle_seconds() -> u64 {
        const COMBINED_SESSION_STATE: u32 = 0;
        const ANY_INPUT_EVENT: u32 = 0xFFFF_FFFF;
        // SAFETY: documented CoreGraphics call with constant arguments.
        let secs =
            unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT) };
        secs.max(0.0) as u64
    }

    pub fn fullscreen_foreground() -> bool {
        false // shortcut: macOS fullscreen deferral not implemented
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    pub fn idle_seconds() -> u64 {
        0
    }
    pub fn fullscreen_foreground() -> bool {
        false
    }
}

pub fn idle_seconds() -> u64 {
    platform::idle_seconds()
}

/// True when a fullscreen app owns the monitor. Used by the do-not-disturb
/// setting to defer the popup.
pub fn fullscreen_foreground() -> bool {
    platform::fullscreen_foreground()
}

