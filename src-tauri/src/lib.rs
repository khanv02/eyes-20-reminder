//! BreakBuddy core: 1 Hz tick loop, IPC commands, tray wiring.
//!
//! The timer lives here, never in the frontend, so it keeps running with no
//! window open.

mod engine;
mod idle;
mod settings;
mod stats;
mod tray;
mod windows;

use engine::{Effect, Engine, RemindKind, Snapshot};
use serde::Serialize;
use settings::Settings;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::MacosLauncher;

const SHORTCUT: &str = "CommandOrControl+Shift+B";

pub struct AppState {
    settings: Mutex<Settings>,
    engine: Mutex<Engine>,
    db: Mutex<stats::Stats>,
    dir: PathBuf,
    /// SQLite row of the reminder on screen, completed when the user acts.
    session: Mutex<Option<i64>>,
    popup_shown: AtomicBool,
    reminder: Mutex<Option<Reminder>>,
    tray_state: Mutex<&'static str>,
    tip: AtomicUsize,
}

#[derive(Clone, Serialize)]
struct Reminder {
    kind: &'static str,
    tip: &'static str,
}

const TIPS: [&str; 4] = ["lookAway", "stretch", "water", "shoulders"];


fn spawn_ticker(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        tick(&app);
    });
}

fn tick(app: &AppHandle) {
    let state = app.state::<AppState>();
    let idle_secs = idle::idle_seconds();
    let (effects, snapshot, paused) = {
        let mut engine = state.engine.lock().unwrap();
        let effects = engine.tick(Instant::now(), idle_secs);
        (effects, engine.snapshot(), engine.is_paused())
    };
    let settings = state.settings.lock().unwrap().clone();

    for effect in effects {
        match effect {
            Effect::Show(kind) => {
                record_session(&state, &settings, kind, &snapshot);
                let tip = TIPS[state.tip.fetch_add(1, Ordering::Relaxed) % TIPS.len()];
                state.reminder.lock().unwrap().replace(Reminder { kind: kind.as_str(), tip });
            }
            Effect::Nudge => {
                let _ = app.emit("timer:nudge", ());
            }
            Effect::Hide => {}
        }
    }

    // Drive popup visibility from the snapshot so a deferred reminder appears as
    // soon as fullscreen, work hours or a manual pause stop blocking it.
    let allowed = settings.inside_work_hours()
        && !(settings.do_not_disturb_fullscreen && idle::fullscreen_foreground());
    let shown = state.popup_shown.load(Ordering::Relaxed);
    if snapshot.popup && allowed && !shown {
        if let Some(reminder) = state.reminder.lock().unwrap().clone() {
            if windows::show_popup(app, &settings.popup_corner) {
                state.popup_shown.store(true, Ordering::Relaxed);
                let _ = app.emit("timer:remind", reminder);
            }
        }
    } else if !snapshot.popup {
        state.reminder.lock().unwrap().take();
        if shown {
            state.popup_shown.store(false, Ordering::Relaxed);
            windows::hide_popup(app);
        }
    }

    let _ = app.emit("timer:tick", &snapshot);
    update_tray(app, &state, &snapshot, &settings, paused);
}

fn record_session(state: &AppState, settings: &Settings, kind: RemindKind, snapshot: &Snapshot) {
    let mode = match kind {
        RemindKind::Work => settings.mode.as_str(),
        RemindKind::Break => "pomodoro",
        RemindKind::Resume => return,
    };
    let id = state.db.lock().unwrap().record_reminder(mode, snapshot.active_sec);
    if let Some(id) = id {
        state.session.lock().unwrap().replace(id);
    }
}

fn complete_session(state: &AppState) {
    if let Some(id) = state.session.lock().unwrap().take() {
        state.db.lock().unwrap().complete(id);
    }
}

fn hide_popup(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.popup_shown.swap(false, Ordering::Relaxed) {
        windows::hide_popup(app);
    }
}

fn update_tray(
    app: &AppHandle,
    state: &AppState,
    snapshot: &Snapshot,
    settings: &Settings,
    paused: bool,
) {
    let icon = if snapshot.popup {
        "reminding"
    } else if paused || snapshot.state == "paused" {
        "paused"
    } else {
        "working"
    };
    let mut current = state.tray_state.lock().unwrap();
    if *current != icon {
        *current = icon;
        tray::set_state(app, icon);
    }
    drop(current);

    let vi = settings.language == "vi";
    let clock = mmss(snapshot.remaining_sec);
    let text = match snapshot.state {
        "reminding" | "breakPrompt" | "resumePrompt" => {
            if vi { "Đến giờ nghỉ ✨" } else { "Time for a break ✨" }.to_string()
        }
        "snoozed" => {
            if vi { format!("Nhắc lại sau {clock}") } else { format!("Reminding again in {clock}") }
        }
        "break" => {
            if vi { format!("Đang nghỉ {clock}") } else { format!("On a break {clock}") }
        }
        "paused" => if vi { "Đang tạm dừng".to_string() } else { "Paused".to_string() },
        _ => {
            if vi { format!("Nghỉ sau {clock}") } else { format!("Break in {clock}") }
        }
    };
    tray::set_tooltip(app, &format!("BreakBuddy — {text}"));
}

fn mmss(secs: u64) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}


// ---- actions shared by IPC, tray menu and global shortcut -------------------

/// Done: acknowledge the reminder or return from a break.
pub fn finish(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.engine.lock().unwrap().done(Instant::now());
    complete_session(&state);
    hide_popup(app);
    let _ = app.emit("stats:changed", ());
}

pub fn pause_for(app: &AppHandle, minutes: Option<u64>) {
    let state = app.state::<AppState>();
    let until = match minutes {
        Some(m) => Instant::now() + Duration::from_secs(m.max(1) * 60),
        None => Instant::now() + until_tomorrow(),
    };
    state.engine.lock().unwrap().pause(Some(until));
    hide_popup(app);
}

pub fn resume(app: &AppHandle) {
    app.state::<AppState>().engine.lock().unwrap().resume();
}

fn until_tomorrow() -> Duration {
    use chrono::{Local, TimeZone};
    let now = Local::now();
    let midnight = (now + chrono::Duration::days(1))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap_or_else(|| now.naive_local());
    let target = Local.from_local_datetime(&midnight).single().unwrap_or(now);
    (target - now).to_std().unwrap_or(Duration::from_secs(3600))
}

// ---- IPC commands -----------------------------------------------------------

#[tauri::command]
fn complete_break(app: AppHandle) {
    finish(&app);
}

#[tauri::command]
fn start_break(app: AppHandle, state: State<'_, AppState>) {
    state.engine.lock().unwrap().start_break(Instant::now());
    complete_session(&state);
    hide_popup(&app);
    let _ = app.emit("stats:changed", ());
}

#[tauri::command]
fn snooze(app: AppHandle, state: State<'_, AppState>, minutes: u64) {
    state.engine.lock().unwrap().snooze(Instant::now(), minutes.max(1));
    hide_popup(&app);
}

#[tauri::command]
fn pause(app: AppHandle, minutes: Option<u64>) {
    pause_for(&app, minutes);
}

#[tauri::command]
fn resume_timer(app: AppHandle) {
    resume(&app);
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn update_settings(app: AppHandle, state: State<'_, AppState>, patch: serde_json::Value) -> Settings {
    let next = {
        let mut settings = state.settings.lock().unwrap();
        settings.apply_patch(patch);
        settings.save(&state.dir);
        settings.clone()
    };
    state.engine.lock().unwrap().set_config(next.engine_config());
    apply_autostart(&app, next.autostart);
    let _ = app.emit("settings:changed", &next);
    next
}

#[tauri::command]
fn get_stats(state: State<'_, AppState>) -> Result<stats::StatsPayload, String> {
    state.db.lock().unwrap().payload().map_err(|err| err.to_string())
}

#[tauri::command]
fn get_snapshot(state: State<'_, AppState>) -> Snapshot {
    state.engine.lock().unwrap().snapshot()
}

#[tauri::command]
fn get_reminder(state: State<'_, AppState>) -> Option<Reminder> {
    state.reminder.lock().unwrap().clone()
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    windows::open_window(&app, windows::SETTINGS_LABEL, "BreakBuddy — Settings", 460.0, 620.0);
}

#[tauri::command]
fn open_stats(app: AppHandle) {
    windows::open_window(&app, windows::STATS_LABEL, "BreakBuddy — Stats", 420.0, 480.0);
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

fn apply_autostart(app: &AppHandle, enabled: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let result = if enabled { manager.enable() } else { manager.disable() };
    if let Err(err) = result {
        eprintln!("autostart: {err}");
    }
}


// ---- app entry --------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        finish(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            complete_break,
            start_break,
            snooze,
            pause,
            resume_timer,
            get_settings,
            update_settings,
            get_stats,
            get_snapshot,
            get_reminder,
            open_settings,
            open_stats,
            quit
        ])
        .setup(|app| {
            use tauri_plugin_global_shortcut::GlobalShortcutExt;

            let handle = app.handle().clone();
            let dir = handle.path().app_config_dir()?;
            let settings = settings::Settings::load(&dir);
            let db_path = handle.path().app_data_dir()?.join("breakbuddy.db");
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let db = stats::Stats::open(&db_path)?;
            let engine = Engine::new(settings.engine_config(), Instant::now());

            app.manage(AppState {
                settings: Mutex::new(settings.clone()),
                engine: Mutex::new(engine),
                db: Mutex::new(db),
                dir,
                session: Mutex::new(None),
                popup_shown: AtomicBool::new(false),
                reminder: Mutex::new(None),
                tray_state: Mutex::new("working"),
                tip: AtomicUsize::new(0),
            });

            tray::init(&handle, &settings.language)?;
            apply_autostart(&handle, settings.autostart);
            if let Err(err) = handle.global_shortcut().register(SHORTCUT) {
                eprintln!("global shortcut: {err}");
            }
            spawn_ticker(handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("BreakBuddy failed to start");
}

