//! Local stats in SQLite. Writes stay in Rust; the frontend only reads a summary.

use chrono::{Duration, Local, NaiveDate};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Day {
    pub date: String,
    pub reminders: u32,
    pub done: u32,
    pub active_secs: u64,
    pub pomodoro_cycles: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsPayload {
    pub today: Day,
    pub days: Vec<Day>,
    pub streak: u32,
}

pub struct Stats {
    conn: Connection,
}

impl Stats {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
               id            INTEGER PRIMARY KEY AUTOINCREMENT,
               started_at    TEXT NOT NULL,
               active_secs   INTEGER NOT NULL,
               reminded_at   TEXT,
               completed_at  TEXT,
               mode          TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_sessions_started ON sessions(started_at);",
        )?;
        Ok(Self { conn })
    }

    /// Returns the new session id so the popup acknowledgement can complete it.
    pub fn record_reminder(&self, mode: &str, active_secs: u64) -> Option<i64> {
        let now = Local::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO sessions (started_at, active_secs, reminded_at, mode) VALUES (?1, ?2, ?1, ?3)",
                params![now, active_secs as i64, mode],
            )
            .ok()?;
        Some(self.conn.last_insert_rowid())
    }

    pub fn complete(&self, id: i64) {
        let _ = self.conn.execute(
            "UPDATE sessions SET completed_at = ?1 WHERE id = ?2 AND completed_at IS NULL",
            params![Local::now().to_rfc3339(), id],
        );
    }

    pub fn payload(&self) -> rusqlite::Result<StatsPayload> {
        let mut by_date: HashMap<String, Day> = HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT substr(started_at, 1, 10) AS d,
                    COUNT(*),
                    SUM(completed_at IS NOT NULL),
                    COALESCE(SUM(active_secs), 0),
                    SUM(completed_at IS NOT NULL AND mode = 'pomodoro')
             FROM sessions GROUP BY d",
        )?;
        let rows = stmt.query_map([], |row| {
            let date: String = row.get(0)?;
            Ok(Day {
                date,
                reminders: row.get::<_, i64>(1)? as u32,
                done: row.get::<_, i64>(2)? as u32,
                active_secs: row.get::<_, i64>(3)? as u64,
                pomodoro_cycles: row.get::<_, i64>(4)? as u32,
            })
        })?;
        for day in rows {
            let day = day?;
            by_date.insert(day.date.clone(), day);
        }

        let today = Local::now().date_naive();
        let days: Vec<Day> = (0..7)
            .rev()
            .map(|offset| {
                let date = today - Duration::days(offset);
                by_date.get(&date.to_string()).cloned().unwrap_or_else(|| Day {
                    date: date.to_string(),
                    ..Day::default()
                })
            })
            .collect();

        Ok(StatsPayload {
            today: days.last().cloned().unwrap_or_default(),
            days,
            streak: self.streak(today, &by_date),
        })
    }

    fn streak(&self, today: NaiveDate, by_date: &HashMap<String, Day>) -> u32 {
        let has_break = |d: NaiveDate| by_date.get(&d.to_string()).is_some_and(|day| day.done > 0);
        let mut day = if has_break(today) { today } else { today - Duration::days(1) };
        let mut streak = 0;
        while has_break(day) {
            streak += 1;
            day -= Duration::days(1);
        }
        streak
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_completes_a_session() {
        let stats = Stats::init(Connection::open_in_memory().unwrap()).unwrap();
        let id = stats.record_reminder("reminder", 1200).unwrap();
        stats.complete(id);

        let payload = stats.payload().unwrap();
        assert_eq!(payload.today.reminders, 1);
        assert_eq!(payload.today.done, 1);
        assert_eq!(payload.today.active_secs, 1200);
        assert_eq!(payload.today.pomodoro_cycles, 0);
        assert_eq!(payload.days.len(), 7);
        assert_eq!(payload.streak, 1);
    }
}
