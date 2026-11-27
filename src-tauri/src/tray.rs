//! System tray: state icon, tooltip, menu.

use crate::windows;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::AppHandle;

const ID: &str = "main";
const WORKING: &[u8] = include_bytes!("../icons/tray-working.png");
const PAUSED: &[u8] = include_bytes!("../icons/tray-paused.png");
const REMINDING: &[u8] = include_bytes!("../icons/tray-reminding.png");

fn icon(state: &str) -> Option<Image<'static>> {
    let bytes = match state {
        "pausing" | "paused" => PAUSED,
        "reminding" => REMINDING,
        _ => WORKING,
    };
    Image::from_bytes(bytes).ok()
}

fn label(language: &str, key: &str) -> &'static str {
    let vi = language == "vi";
    match key {
        "pause30" => if vi { "Tạm dừng 30 phút" } else { "Pause 30 min" },
        "pause60" => if vi { "Tạm dừng 1 giờ" } else { "Pause 1 hour" },
        "pauseToday" => if vi { "Tạm dừng đến mai" } else { "Pause until tomorrow" },
        "resume" => if vi { "Tiếp tục" } else { "Resume" },
        "settings" => if vi { "Cài đặt" } else { "Settings" },
        "stats" => if vi { "Thống kê" } else { "Stats" },
        "quit" => if vi { "Thoát" } else { "Quit" },
        "done" => if vi { "Đã nghỉ xong ✓" } else { "Done ✓" },
        _ => "",
    }
}

pub fn init(app: &AppHandle, language: &str) -> tauri::Result<()> {
    let pause30 = MenuItem::with_id(app, "pause-30", label(language, "pause30"), true, None::<&str>)?;
    let pause60 = MenuItem::with_id(app, "pause-60", label(language, "pause60"), true, None::<&str>)?;
    let pause_today =
        MenuItem::with_id(app, "pause-today", label(language, "pauseToday"), true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", label(language, "resume"), true, None::<&str>)?;
    let done = MenuItem::with_id(app, "done", label(language, "done"), true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", label(language, "settings"), true, None::<&str>)?;
    let stats = MenuItem::with_id(app, "stats", label(language, "stats"), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", label(language, "quit"), true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &done,
            &PredefinedMenuItem::separator(app)?,
            &pause30,
            &pause60,
            &pause_today,
            &resume,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &stats,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id(ID)
        .tooltip("BreakBuddy")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "done" => crate::finish(app),
            "pause-30" => crate::pause_for(app, Some(30)),
            "pause-60" => crate::pause_for(app, Some(60)),
            "pause-today" => crate::pause_for(app, None),
            "resume" => crate::resume(app),
            "settings" => windows::open_window(app, windows::SETTINGS_LABEL, "BreakBuddy — Settings", 460.0, 620.0),
            "stats" => windows::open_window(app, windows::STATS_LABEL, "BreakBuddy — Stats", 420.0, 480.0),
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(img) = icon("working") {
        builder = builder.icon(img);
    }
    builder.build(app)?;
    Ok(())
}

pub fn set_state(app: &AppHandle, state: &str) {
    let Some(tray) = app.tray_by_id(ID) else { return };
    if let Some(img) = icon(state) {
        let _ = tray.set_icon(Some(img));
    }
}

pub fn set_tooltip(app: &AppHandle, text: &str) {
    let Some(tray) = app.tray_by_id(ID) else { return };
    let _ = tray.set_tooltip(Some(text));
}
