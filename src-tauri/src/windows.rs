//! Window management: one popup at a time, placed on the monitor under the
//! cursor, plus the settings and stats windows.

use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const POPUP_LABEL: &str = "popup";
pub const SETTINGS_LABEL: &str = "settings";
pub const STATS_LABEL: &str = "stats";

const POPUP_W: f64 = 320.0;
const POPUP_H: f64 = 192.0;
const MARGIN: f64 = 16.0;

pub fn show_popup(app: &AppHandle, corner: &str) -> bool {
    let win = match app.get_webview_window(POPUP_LABEL) {
        Some(win) => win,
        None => match build_popup(app) {
            Ok(win) => win,
            Err(err) => {
                eprintln!("popup build failed: {err}");
                return false;
            }
        },
    };
    place(&app, &win, corner);
    present(&win)
}

fn present(win: &WebviewWindow) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};
        if let Ok(hwnd) = win.hwnd() {
            // Show without activating: the user's keyboard focus must stay put.
            unsafe { ShowWindow(hwnd.0 as *mut core::ffi::c_void, SW_SHOWNOACTIVATE) };
            return true;
        }
    }
    win.show().is_ok()
}

/// Borderless tool window that never takes focus (WCAG-friendly: clicks still work).
#[cfg(windows)]
fn no_activate(win: &WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    let Ok(hwnd) = win.hwnd() else { return };
    let hwnd = hwnd.0 as *mut core::ffi::c_void;
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let extra = (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | extra);
    }
}

#[cfg(not(windows))]
fn no_activate(_win: &WebviewWindow) {}


pub fn hide_popup(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(POPUP_LABEL) {
        let _ = win.hide();
    }
}

fn build_popup(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let win = WebviewWindowBuilder::new(app, POPUP_LABEL, WebviewUrl::App("index.html".into()))
        .title("BreakBuddy")
        .inner_size(POPUP_W, POPUP_H)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        // Not focusable: clicks work, keyboard focus never moves away from the user's app.
        .focusable(false)
        .resizable(false)
        .visible(false)
        .build()?;
    no_activate(&win);
    Ok(win)
}

fn place(app: &AppHandle, win: &WebviewWindow, corner: &str) {
    let cursor = match app.cursor_position() {
        Ok(pos) => pos,
        Err(_) => return,
    };
    let monitor = match app.monitor_from_point(cursor.x, cursor.y) {
        Ok(Some(monitor)) => monitor,
        _ => return,
    };
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    let margin = (MARGIN * scale).round() as i32;
    let Ok(size) = win.outer_size() else { return };

    let left = area.position.x + margin;
    let top = area.position.y + margin;
    let right = area.position.x + area.size.width as i32 - size.width as i32 - margin;
    let bottom = area.position.y + area.size.height as i32 - size.height as i32 - margin;

    let x = if corner.starts_with("top") || corner.starts_with("bottom") {
        if corner.ends_with("left") { left } else { right }
    } else {
        right
    };
    let y = if corner.starts_with("top") { top } else { bottom };
    let _ = win.set_position(PhysicalPosition::new(x.max(left), y.max(top)));
    let _ = win.set_always_on_top(true);
}

/// Opens (or focuses) a normal window for settings / stats.
pub fn open_window(app: &AppHandle, label: &str, title: &str, width: f64, height: f64) {
    if let Some(win) = app.get_webview_window(label) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(width, height)
        .min_inner_size(width, height)
        .resizable(true)
        .center()
        .build();
    if let Err(err) = built {
        eprintln!("{label} window failed: {err}");
    }
}
