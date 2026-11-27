//! Timer state machine. Owns all timing logic; the UI only renders snapshots.
//!
//! Uses a monotonic `Instant` only, so sleep/wake and clock changes cannot
//! corrupt the count.

use serde::Serialize;
use std::time::{Duration, Instant};

const NUDGE_AFTER: Duration = Duration::from_secs(120);
const NUDGE_EVERY: Duration = Duration::from_secs(300);
/// A single tick never credits more than this, so a stalled loop cannot inflate
/// active time. Longer gaps are handled as away time.
const MAX_TICK_SECS: u64 = 5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Working,
    Paused,
    Reminding,
    Snoozed,
    Focus,
    BreakPrompt,
    Break,
    ResumePrompt,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Working => "working",
            State::Paused => "paused",
            State::Reminding => "reminding",
            State::Snoozed => "snoozed",
            State::Focus => "focus",
            State::BreakPrompt => "breakPrompt",
            State::Break => "break",
            State::ResumePrompt => "resumePrompt",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RemindKind {
    Work,
    Break,
    Resume,
}

impl RemindKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RemindKind::Work => "work",
            RemindKind::Break => "break",
            RemindKind::Resume => "resume",
        }
    }
}

/// What the tick loop must do to the popup window.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effect {
    Show(RemindKind),
    Hide,
    Nudge,
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub pomodoro: bool,
    pub work_sec: u64,
    pub idle_sec: u64,
    pub natural_sec: u64,
    pub focus_sec: u64,
    pub short_sec: u64,
    pub long_sec: u64,
    pub cycles: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pomodoro: false,
            work_sec: 20 * 60,
            idle_sec: 120,
            natural_sec: 300,
            focus_sec: 25 * 60,
            short_sec: 5 * 60,
            long_sec: 15 * 60,
            cycles: 4,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub state: &'static str,
    pub active_sec: u64,
    pub remaining_sec: u64,
    pub progress: f64,
    pub cycle: u32,
    pub cycles: u32,
    pub popup: bool,
}


pub struct Engine {
    cfg: Config,
    state: State,
    active: u64,
    break_left: u64,
    break_total: u64,
    cycle: u32,
    last: Instant,
    shown_at: Option<Instant>,
    last_nudge: Option<Instant>,
    snooze_until: Option<Instant>,
    pause_until: Option<Instant>,
}

impl Engine {
    pub fn new(cfg: Config, now: Instant) -> Self {
        let state = if cfg.pomodoro { State::Focus } else { State::Working };
        Self {
            cfg,
            state,
            active: 0,
            break_left: 0,
            break_total: 0,
            cycle: 0,
            last: now,
            shown_at: None,
            last_nudge: None,
            snooze_until: None,
            pause_until: None,
        }
    }

    /// Re-apply settings. Switching mode restarts the session so the two modes
    /// never share accumulated time.
    pub fn set_config(&mut self, cfg: Config) {
        let mode_changed = cfg.pomodoro != self.cfg.pomodoro;
        self.cfg = cfg;
        if mode_changed {
            self.reset(Instant::now());
        }
        let cap = if self.cfg.pomodoro { self.cfg.focus_sec } else { self.cfg.work_sec };
        self.active = self.active.min(cap);
    }

    pub fn state(&self) -> State {
        if self.pause_until.is_some() {
            return State::Paused;
        }
        self.state
    }

    pub fn is_paused(&self) -> bool {
        self.pause_until.is_some()
    }

    pub fn snapshot(&self) -> Snapshot {
        let state = self.state();
        let (remaining, progress) = match self.state {
            State::Working | State::Paused if !self.cfg.pomodoro => (
                self.cfg.work_sec.saturating_sub(self.active),
                ratio(self.active, self.cfg.work_sec),
            ),
            State::Focus => (
                self.cfg.focus_sec.saturating_sub(self.active),
                ratio(self.active, self.cfg.focus_sec),
            ),
            State::Break => (
                self.break_left,
                ratio(self.break_total.saturating_sub(self.break_left), self.break_total),
            ),
            State::Snoozed => (
                self.snooze_until
                    .map(|t| t.saturating_duration_since(Instant::now()).as_secs())
                    .unwrap_or(0),
                1.0,
            ),
            _ => (0, 1.0),
        };
        Snapshot {
            state: state.as_str(),
            active_sec: self.active,
            remaining_sec: remaining,
            progress,
            cycle: if self.cfg.cycles == 0 { 0 } else { self.cycle % self.cfg.cycles },
            cycles: self.cfg.cycles,
            popup: matches!(
                self.state,
                State::Reminding | State::BreakPrompt | State::ResumePrompt
            ),
        }
    }

    pub fn tick(&mut self, now: Instant, idle_secs: u64) -> Vec<Effect> {
        let dt = now.saturating_duration_since(self.last).as_secs();
        self.last = now;
        let mut fx = Vec::new();

        if let Some(until) = self.pause_until {
            if now < until {
                return fx;
            }
            self.pause_until = None;
        }

        // Sleep/wake or a stalled loop: treat the gap as away time.
        if dt >= self.cfg.natural_sec {
            fx.push(Effect::Hide);
            self.reset(now);
            return fx;
        }
        let dt = dt.min(MAX_TICK_SECS);

        if self.cfg.pomodoro {
            self.tick_pomodoro(now, idle_secs, dt, &mut fx);
        } else {
            self.tick_reminder(now, idle_secs, dt, &mut fx);
        }
        fx
    }

    fn tick_reminder(&mut self, now: Instant, idle: u64, dt: u64, fx: &mut Vec<Effect>) {
        match self.state {
            State::Working => {
                if idle >= self.cfg.natural_sec {
                    self.active = 0;
                } else if idle >= self.cfg.idle_sec {
                    self.state = State::Paused;
                } else {
                    self.active += dt;
                    if self.active >= self.cfg.work_sec {
                        self.enter_reminding(now, fx);
                    }
                }
            }
            State::Paused => {
                if idle >= self.cfg.natural_sec {
                    self.active = 0;
                    self.state = State::Working;
                } else if idle < self.cfg.idle_sec {
                    self.state = State::Working;
                }
            }
            State::Reminding => {
                // Away long enough while the popup is up counts as the break.
                if idle >= self.cfg.natural_sec {
                    self.reset(now);
                    fx.push(Effect::Hide);
                } else {
                    let shown = *self.shown_at.get_or_insert(now);
                    let due = now.saturating_duration_since(shown) >= NUDGE_AFTER
                        && self
                            .last_nudge
                            .map_or(true, |t| now.saturating_duration_since(t) >= NUDGE_EVERY);
                    if due {
                        self.last_nudge = Some(now);
                        fx.push(Effect::Nudge);
                    }
                }
            }
            State::Snoozed => {
                if self.snooze_until.is_some_and(|t| now >= t) {
                    self.enter_reminding(now, fx);
                }
            }
            _ => {}
        }
    }


    fn tick_pomodoro(&mut self, now: Instant, idle: u64, dt: u64, fx: &mut Vec<Effect>) {
        match self.state {
            State::Focus => {
                if idle >= self.cfg.natural_sec {
                    self.active = 0;
                } else if idle < self.cfg.idle_sec {
                    self.active += dt;
                    if self.active >= self.cfg.focus_sec {
                        self.state = State::BreakPrompt;
                        self.cycle += 1;
                        self.shown_at = Some(now);
                        self.last_nudge = None;
                        fx.push(Effect::Show(RemindKind::Break));
                    }
                }
            }
            State::Break => {
                self.break_left = self.break_left.saturating_sub(dt);
                if self.break_left == 0 {
                    self.state = State::ResumePrompt;
                    self.shown_at = Some(now);
                    self.last_nudge = None;
                    fx.push(Effect::Show(RemindKind::Resume));
                }
            }
            _ => {}
        }
    }

    fn enter_reminding(&mut self, now: Instant, fx: &mut Vec<Effect>) {
        self.state = State::Reminding;
        self.shown_at = Some(now);
        self.snooze_until = None;
        self.last_nudge = None;
        fx.push(Effect::Show(RemindKind::Work));
    }

    /// Done: acknowledge the reminder / resume work after a break.
    pub fn done(&mut self, now: Instant) -> Vec<Effect> {
        match self.state {
            State::Reminding | State::Snoozed => {
                self.reset(now);
                vec![Effect::Hide]
            }
            State::ResumePrompt => {
                self.active = 0;
                if self.cfg.cycles > 0 && self.cycle % self.cfg.cycles == 0 {
                    self.cycle = 0;
                }
                self.state = State::Focus;
                self.shown_at = None;
                vec![Effect::Hide]
            }
            _ => Vec::new(),
        }
    }

    pub fn start_break(&mut self, now: Instant) -> Vec<Effect> {
        if self.state != State::BreakPrompt {
            return Vec::new();
        }
        let long = self.cfg.cycles > 0 && self.cycle % self.cfg.cycles == 0;
        self.break_total = if long { self.cfg.long_sec } else { self.cfg.short_sec };
        self.break_left = self.break_total;
        self.state = State::Break;
        self.shown_at = Some(now);
        vec![Effect::Hide]
    }

    pub fn snooze(&mut self, now: Instant, minutes: u64) -> Vec<Effect> {
        if self.state != State::Reminding {
            return Vec::new();
        }
        self.state = State::Snoozed;
        self.snooze_until = Some(now + Duration::from_secs(minutes * 60));
        vec![Effect::Hide]
    }

    pub fn pause(&mut self, until: Option<Instant>) {
        self.pause_until = until;
        if until.is_some() {
            // A manual pause also dismisses the popup; time already counted is kept.
            self.state = if self.cfg.pomodoro { State::Focus } else { State::Working };
            self.shown_at = None;
        }
    }

    pub fn resume(&mut self) {
        self.pause_until = None;
    }

    fn reset(&mut self, now: Instant) {
        self.active = 0;
        self.break_left = 0;
        self.break_total = 0;
        self.cycle = 0;
        self.shown_at = None;
        self.last_nudge = None;
        self.snooze_until = None;
        self.state = if self.cfg.pomodoro { State::Focus } else { State::Working };
        self.last = now;
    }
}

fn ratio(done: u64, total: u64) -> f64 {
    if total == 0 {
        return 1.0;
    }
    (done as f64 / total as f64).clamp(0.0, 1.0)
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Drives the engine one tick per second, returns at the first tick that
    /// emitted an effect (or after `secs`).
    fn run(e: &mut Engine, t: Instant, secs: u64, idle: u64) -> (Instant, Vec<Effect>) {
        let mut effects = Vec::new();
        for i in 1..=secs {
            effects = e.tick(t + Duration::from_secs(i), idle);
            if !effects.is_empty() {
                return (t + Duration::from_secs(i), effects);
            }
        }
        (t + Duration::from_secs(secs), effects)
    }

    fn engine(pomodoro: bool) -> (Engine, Instant) {
        let now = Instant::now();
        let cfg = Config {
            pomodoro,
            work_sec: 20 * 60,
            ..Config::default()
        };
        (Engine::new(cfg, now), now)
    }

    #[test]
    fn reminds_after_work_minutes() {
        let (mut e, t0) = engine(false);
        let t = run(&mut e, t0, 20 * 60 - 1, 0).0;
        assert_eq!(e.state(), State::Working);
        let fx = e.tick(t + Duration::from_secs(1), 0);
        assert_eq!(e.state(), State::Reminding);
        assert_eq!(fx, vec![Effect::Show(RemindKind::Work)]);
    }

    #[test]
    fn idle_pauses_and_resumes() {
        let (mut e, t0) = engine(false);
        let t = run(&mut e, t0, 600, 0).0;
        assert_eq!(e.snapshot().active_sec, 600);

        let t = run(&mut e, t, 5, 130).0;
        assert_eq!(e.state(), State::Paused);
        assert_eq!(e.snapshot().active_sec, 600);

        e.tick(t + Duration::from_secs(1), 0);
        assert_eq!(e.state(), State::Working);
    }

    #[test]
    fn natural_break_resets_timer() {
        let (mut e, t0) = engine(false);
        let t = run(&mut e, t0, 600, 0).0;
        e.tick(t + Duration::from_secs(1), 300);
        assert_eq!(e.snapshot().active_sec, 0);
        assert_eq!(e.state(), State::Working);
    }

    #[test]
    fn done_resets_timer() {
        let (mut e, t0) = engine(false);
        e.set_config(Config { work_sec: 60, ..Config::default() });
        let t = run(&mut e, t0, 60, 0).0;
        assert_eq!(e.state(), State::Reminding);

        assert_eq!(e.done(t), vec![Effect::Hide]);
        assert_eq!(e.state(), State::Working);
        e.tick(t + Duration::from_secs(1), 0);
        assert_eq!(e.snapshot().active_sec, 1);
    }

    #[test]
    fn popup_never_auto_dismisses() {
        let (mut e, t0) = engine(false);
        e.set_config(Config { work_sec: 5, ..Config::default() });
        let t = run(&mut e, t0, 5, 0).0;
        assert_eq!(e.state(), State::Reminding);
        run(&mut e, t, 900, 0);
        assert_eq!(e.state(), State::Reminding);
        assert!(e.snapshot().popup);
    }

    #[test]
    fn snooze_then_remind_again() {
        let (mut e, t0) = engine(false);
        e.set_config(Config { work_sec: 5, ..Config::default() });
        let t = run(&mut e, t0, 5, 0).0;
        assert_eq!(e.snooze(t, 5), vec![Effect::Hide]);
        assert_eq!(e.state(), State::Snoozed);

        let t = run(&mut e, t, 299, 0).0;
        assert_eq!(e.state(), State::Snoozed);
        let fx = e.tick(t + Duration::from_secs(1), 0);
        assert_eq!(e.state(), State::Reminding);
        assert_eq!(fx, vec![Effect::Show(RemindKind::Work)]);
    }

    #[test]
    fn nudge_is_rate_limited() {
        let (mut e, t0) = engine(false);
        e.set_config(Config { work_sec: 5, ..Config::default() });
        let t = run(&mut e, t0, 5, 0).0;

        let (t, fx) = run(&mut e, t, 119, 0);
        assert!(fx.is_empty());
        let nudge_at = t + Duration::from_secs(1);
        let fx = e.tick(nudge_at, 0);
        assert_eq!(fx, vec![Effect::Nudge]);

        let (t, fx) = run(&mut e, nudge_at, 298, 0);
        assert!(fx.is_empty());
        let fx = e.tick(t + Duration::from_secs(2), 0);
        assert_eq!(fx, vec![Effect::Nudge]);
    }


    #[test]
    fn away_while_reminding_counts_as_break() {
        let (mut e, t0) = engine(false);
        e.set_config(Config { work_sec: 5, ..Config::default() });
        let t = run(&mut e, t0, 5, 0).0;
        let fx = e.tick(t + Duration::from_secs(1), 300);
        assert_eq!(fx, vec![Effect::Hide]);
        assert_eq!(e.state(), State::Working);
        assert_eq!(e.snapshot().active_sec, 0);
    }

    #[test]
    fn sleep_gap_resets_and_hides() {
        let (mut e, t0) = engine(false);
        let t = run(&mut e, t0, 600, 0).0;
        let fx = e.tick(t + Duration::from_secs(3600), 0);
        assert_eq!(fx, vec![Effect::Hide]);
        assert_eq!(e.snapshot().active_sec, 0);
    }

    #[test]
    fn manual_pause_freezes_then_resumes() {
        let (mut e, t0) = engine(false);
        let t = run(&mut e, t0, 100, 0).0;
        e.pause(Some(t + Duration::from_secs(60)));
        assert_eq!(e.state(), State::Paused);

        let t = run(&mut e, t, 59, 0).0;
        assert_eq!(e.snapshot().active_sec, 100);

        let t = t + Duration::from_secs(1);
        e.tick(t, 0);
        assert_eq!(e.state(), State::Working);
        assert!(!e.is_paused());
        assert_eq!(e.snapshot().active_sec, 101);

        e.resume();
        e.tick(t + Duration::from_secs(1), 0);
        assert_eq!(e.snapshot().active_sec, 102);
    }

    #[test]
    fn pomodoro_cycle_and_long_break() {
        let now = Instant::now();
        let cfg = Config {
            pomodoro: true,
            focus_sec: 60,
            short_sec: 30,
            long_sec: 90,
            cycles: 2,
            ..Config::default()
        };
        let mut e = Engine::new(cfg, now);
        assert_eq!(e.state(), State::Focus);

        let t = run(&mut e, now, 60, 0).0;
        assert_eq!(e.state(), State::BreakPrompt);
        assert_eq!(e.snapshot().cycle, 1);

        e.start_break(t);
        assert_eq!(e.state(), State::Break);
        let t = run(&mut e, t, 30, 0).0;
        assert_eq!(e.state(), State::ResumePrompt);
        e.done(t);
        assert_eq!(e.state(), State::Focus);

        let t = run(&mut e, t, 60, 0).0;
        assert_eq!(e.state(), State::BreakPrompt);
        e.start_break(t);
        assert_eq!(e.snapshot().remaining_sec, 90);
    }

    #[test]
    fn pomodoro_pauses_when_idle() {
        let now = Instant::now();
        let cfg = Config { pomodoro: true, focus_sec: 60, ..Config::default() };
        let mut e = Engine::new(cfg, now);
        let t = run(&mut e, now, 10, 130).0;
        assert_eq!(e.snapshot().active_sec, 0);
        run(&mut e, t, 60, 0);
        assert_eq!(e.state(), State::BreakPrompt);
    }
}

