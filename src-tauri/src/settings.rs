//! Settings: one JSON file, merged patches, applied immediately.

use crate::engine;
use chrono::{Local, NaiveTime};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Pomodoro {
    pub focus: u64,
    pub short: u64,
    pub long: u64,
    pub cycles: u32,
}

impl Default for Pomodoro {
    fn default() -> Self {
        Self { focus: 25, short: 5, long: 15, cycles: 4 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkHours {
    pub start: String,
    pub end: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub mode: String,
    pub work_minutes: u64,
    pub idle_threshold_sec: u64,
    pub natural_break_sec: u64,
    pub popup_corner: String,
    pub sound: bool,
    pub snooze_enabled: bool,
    pub snooze_minutes: u64,
    pub pomodoro: Pomodoro,
    pub autostart: bool,
    pub theme: String,
    pub language: String,
    pub work_hours: Option<WorkHours>,
    pub do_not_disturb_fullscreen: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: "reminder".into(),
            work_minutes: 20,
            idle_threshold_sec: 120,
            natural_break_sec: 300,
            popup_corner: "bottom-right".into(),
            sound: true,
            snooze_enabled: true,
            snooze_minutes: 5,
            pomodoro: Pomodoro::default(),
            autostart: true,
            theme: "system".into(),
            language: "en".into(),
            work_hours: None,
            do_not_disturb_fullscreen: true,
        }
    }
}

impl Settings {
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(dir.join("settings.json"))
            .ok()
            // A hand-edited file may start with a BOM; don't silently fall back to defaults.
            .and_then(|raw| serde_json::from_str(raw.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) {
        if let Err(err) = std::fs::create_dir_all(dir) {
            eprintln!("settings dir: {err}");
            return;
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(err) = std::fs::write(dir.join("settings.json"), json) {
                    eprintln!("settings write: {err}");
                }
            }
            Err(err) => eprintln!("settings serialise: {err}"),
        }
    }

    /// Merge a partial object (what the settings window sends) into the current
    /// settings. Unknown keys are ignored; invalid values keep their old value.
    pub fn apply_patch(&mut self, patch: serde_json::Value) {
        let Ok(base) = serde_json::to_value(&*self) else { return };
        let mut merged = base;
        merge(&mut merged, patch);
        if let Ok(next) = serde_json::from_value::<Settings>(merged) {
            *self = next;
        }
    }

    pub fn engine_config(&self) -> engine::Config {
        engine::Config {
            pomodoro: self.mode == "pomodoro",
            work_sec: self.work_minutes.max(1) * 60,
            idle_sec: self.idle_threshold_sec.max(1),
            natural_sec: self.natural_break_sec.max(1),
            focus_sec: self.pomodoro.focus.max(1) * 60,
            short_sec: self.pomodoro.short.max(1) * 60,
            long_sec: self.pomodoro.long.max(1) * 60,
            cycles: self.pomodoro.cycles.max(1),
        }
    }

    /// Reminders only appear inside this window when one is configured.
    pub fn inside_work_hours(&self) -> bool {
        let Some(hours) = &self.work_hours else { return true };
        let parse = |s: &str| NaiveTime::parse_from_str(s, "%H:%M").ok();
        let (Some(start), Some(end)) = (parse(&hours.start), parse(&hours.end)) else {
            return true;
        };
        let now = Local::now().time();
        if start == end {
            true // start == end is a full day, not an empty window
        } else if start < end {
            now >= start && now <= end
        } else {
            // Window crosses midnight.
            now >= start || now <= end
        }
    }
}

fn merge(base: &mut serde_json::Value, patch: serde_json::Value) {
    match (base, patch) {
        (serde_json::Value::Object(b), serde_json::Value::Object(p)) => {
            for (k, v) in p {
                match b.get_mut(&k) {
                    Some(slot) => merge(slot, v),
                    None => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (b, p) => *b = p,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_updates_only_given_keys() {
        let mut s = Settings::default();
        s.apply_patch(serde_json::json!({ "workMinutes": 30, "pomodoro": { "focus": 50 } }));
        assert_eq!(s.work_minutes, 30);
        assert_eq!(s.pomodoro.focus, 50);
        assert_eq!(s.pomodoro.short, 5, "missing nested keys keep defaults");
        assert_eq!(s.idle_threshold_sec, 120);
    }

    #[test]
    fn work_hours_window() {
        let mut s = Settings::default();
        assert!(s.inside_work_hours());
        s.work_hours = Some(WorkHours { start: "00:00".into(), end: "23:59".into() });
        assert!(s.inside_work_hours());
        s.work_hours = Some(WorkHours { start: "00:00".into(), end: "00:00".into() });
        assert!(s.inside_work_hours(), "start == end is a full day, not empty");
    }
}
