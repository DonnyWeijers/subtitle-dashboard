//! In-memory SQLite, filled from subtrans' files.
//!
//! The database is a cache: it is rebuilt from `costs.jsonl` and `state.json` on every start, so it needs no
//! volume or backup. subtrans is never written to.
//!
//! - `costs.jsonl` is append-only (one line per API call): only new, complete lines are read, from the byte offset
//!   reached last time. A file that got shorter was replaced, so it is read again from the start.
//! - `state.json` (one entry per subtitle) is re-read whenever its modification time or size changes. A file that
//!   doesn't parse (caught mid-write) is tried again on the next poll; the previous data stays until then.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::model::{Attempt, Problem, Step1, null_default};
use crate::title::{self, ShowKind};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

const SCHEMA: &str = "
CREATE TABLE calls (
    t         TEXT NOT NULL,   -- local time, 2026-10-23T14:20:52
    day       TEXT NOT NULL,
    month     TEXT NOT NULL,
    kind      TEXT NOT NULL,   -- translate | judge
    model     TEXT NOT NULL,
    eur       REAL NOT NULL,
    show      TEXT NOT NULL,
    show_kind TEXT NOT NULL,   -- series | movie | test
    season    INTEGER,
    episode   INTEGER
);
CREATE INDEX calls_day ON calls (day);
CREATE INDEX calls_month ON calls (month);
CREATE INDEX calls_show ON calls (show);

CREATE TABLE translations (
    id            TEXT PRIMARY KEY,  -- stable hash of the path
    path          TEXT NOT NULL,
    show          TEXT NOT NULL,
    show_kind     TEXT NOT NULL,
    season        INTEGER,
    episode       INTEGER,
    episode_title TEXT,
    file          TEXT,
    step          INTEGER NOT NULL,  -- 1 = first model, 2 = fallback
    model         TEXT NOT NULL,
    models_used   TEXT NOT NULL,     -- JSON array
    lines         INTEGER NOT NULL,
    missing       INTEGER NOT NULL,
    score         REAL,              -- only for the share the AI judge reviewed
    accuracy      REAL,
    fluency       REAL,
    weak          INTEGER NOT NULL,
    eur           REAL NOT NULL,
    judge_eur     REAL NOT NULL,
    seconds       REAL,
    at            TEXT NOT NULL,
    checks        TEXT,              -- JSON object, check -> share of lines
    failed_checks TEXT NOT NULL,     -- JSON array
    step1         TEXT,              -- JSON object (Step1), only when step 1 failed a check
    step1_failed  TEXT NOT NULL,     -- JSON array
    repair        TEXT,              -- Repair, only when step 1 failed a check
    problems      TEXT NOT NULL,     -- JSON array (Problem)
    history       TEXT NOT NULL      -- JSON array (Attempt)
);
CREATE INDEX translations_show ON translations (show);
CREATE INDEX translations_at ON translations (at);
";

pub fn open() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

/// FNV-1a: the same path gives the same id after a restart, so links keep working.
pub fn stable_id(path: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct IngestStatus {
    /// Rows from costs.jsonl.
    pub calls: i64,
    /// Entries from state.json, test runs included.
    pub translations: i64,
    /// costs.jsonl lines and state.json entries that could not be read.
    pub skipped: i64,
    pub last_ingest: Option<String>,
    pub last_error: Option<String>,
}

pub struct Ingester {
    dir: PathBuf,
    eur_per_usd: f64,
    costs_offset: u64,
    state_seen: Option<(SystemTime, u64)>,
    pub status: IngestStatus,
}

#[derive(Deserialize)]
struct CostLine {
    t: String,
    kind: String,
    model: String,
    usd: f64,
    #[serde(default)]
    title: String,
}

#[derive(Deserialize)]
struct StateEntry {
    #[serde(default)]
    title: String,
    #[serde(default)]
    tier: i64,
    #[serde(default)]
    model: String,
    #[serde(default, deserialize_with = "null_default")]
    models_used: Vec<String>,
    #[serde(default)]
    lines: i64,
    #[serde(default)]
    missing: i64,
    score: Option<f64>,
    accuracy: Option<f64>,
    fluency: Option<f64>,
    #[serde(default, deserialize_with = "null_default")]
    problems: Vec<Value>,
    #[serde(default)]
    weak: bool,
    #[serde(default)]
    usd: f64,
    #[serde(default)]
    judge_usd: f64,
    seconds: Option<f64>,
    #[serde(default)]
    at: String,
    #[serde(default, deserialize_with = "null_default")]
    history: Vec<Value>,
    checks: Option<Value>,
    #[serde(default, deserialize_with = "null_default")]
    failed_checks: Vec<String>,
    step1: Option<Value>,
}

impl Ingester {
    pub fn new(dir: impl Into<PathBuf>, eur_per_usd: f64) -> Self {
        Self { dir: dir.into(), eur_per_usd, costs_offset: 0, state_seen: None, status: IngestStatus::default() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Reads whatever changed. Errors are kept in `status` as well as returned.
    pub fn run(&mut self, conn: &mut Connection) -> Result<(), Error> {
        let result = self.ingest_costs(conn).and_then(|()| self.ingest_state(conn));
        self.status.calls = conn.query_row("SELECT COUNT(*) FROM calls", [], |r| r.get(0))?;
        self.status.translations = conn.query_row("SELECT COUNT(*) FROM translations", [], |r| r.get(0))?;
        self.status.last_ingest =
            Some(conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%S', 'now', 'localtime')", [], |r| r.get(0))?);
        self.status.last_error = result.as_ref().err().map(ToString::to_string);
        result
    }

    fn ingest_costs(&mut self, conn: &mut Connection) -> Result<(), Error> {
        let mut file = match File::open(self.dir.join("costs.jsonl")) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let len = file.metadata()?.len();
        if len < self.costs_offset {
            tracing::info!("costs.jsonl got shorter, reading it again from the start");
            conn.execute("DELETE FROM calls", [])?;
            self.costs_offset = 0;
        }
        if len == self.costs_offset {
            return Ok(());
        }

        file.seek(SeekFrom::Start(self.costs_offset))?;
        let mut buf = Vec::new();
        file.take(len - self.costs_offset).read_to_end(&mut buf)?;
        // A line without its newline is still being written: leave it for the next poll.
        let Some(end) = buf.iter().rposition(|&b| b == b'\n').map(|i| i + 1) else {
            return Ok(());
        };

        let tx = conn.transaction()?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO calls (t, day, month, kind, model, eur, show, show_kind, season, episode)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;
            for line in buf[..end].split(|&b| b == b'\n').filter(|l| !l.trim_ascii().is_empty()) {
                let call: CostLine = match serde_json::from_slice::<CostLine>(line) {
                    Ok(call) if call.t.len() >= 10 => call,
                    Ok(_) | Err(_) => {
                        tracing::warn!("skipping unreadable costs.jsonl line: {}", String::from_utf8_lossy(line));
                        self.status.skipped += 1;
                        continue;
                    }
                };
                let parsed = title::parse(&call.title);
                insert.execute(params![
                    call.t,
                    &call.t[..10],
                    &call.t[..7],
                    call.kind,
                    call.model,
                    call.usd * self.eur_per_usd,
                    parsed.show,
                    parsed.kind.as_str(),
                    parsed.season,
                    parsed.episode,
                ])?;
            }
        }
        tx.commit()?;
        self.costs_offset += end as u64;
        Ok(())
    }

    fn ingest_state(&mut self, conn: &mut Connection) -> Result<(), Error> {
        let path = self.dir.join("state.json");
        let meta = match fs::metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let seen = (meta.modified()?, meta.len());
        if self.state_seen == Some(seen) {
            return Ok(());
        }

        let entries: BTreeMap<String, Value> = match serde_json::from_slice(&fs::read(&path)?) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!("state.json doesn't parse (yet), trying again next poll: {e}");
                return Ok(());
            }
        };

        let mut skipped = 0;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM translations", [])?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO translations (id, path, show, show_kind, season, episode, episode_title, file, step, model,
                     models_used, lines, missing, score, accuracy, fluency, weak, eur, judge_eur, seconds, at, checks,
                     failed_checks, step1, step1_failed, repair, problems, history)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21,
                     ?22, ?23, ?24, ?25, ?26, ?27, ?28)",
            )?;
            for (path, value) in entries {
                let entry: StateEntry = match serde_json::from_value(value) {
                    Ok(entry) => entry,
                    Err(e) => {
                        tracing::warn!("skipping state.json entry {path}: {e}");
                        skipped += 1;
                        continue;
                    }
                };
                let parsed = title::parse_with_path(&path, &entry.title);
                let step1: Option<Step1> = entry.step1.and_then(|v| serde_json::from_value(v).ok());
                let checks: Option<BTreeMap<String, f64>> = entry.checks.and_then(|v| serde_json::from_value(v).ok());
                let problems: Vec<Problem> =
                    entry.problems.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect();
                let history: Vec<Attempt> =
                    entry.history.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect();
                let step1_failed = step1.as_ref().map(|s| s.failed.clone()).unwrap_or_default();
                let repair = step1.as_ref().map(|s| s.repair(&entry.failed_checks).as_str());

                insert.execute(params![
                    stable_id(&path),
                    path,
                    parsed.show,
                    parsed.kind.as_str(),
                    parsed.season,
                    parsed.episode,
                    parsed.episode_title,
                    parsed.file,
                    entry.tier,
                    entry.model,
                    serde_json::to_string(&entry.models_used)?,
                    entry.lines,
                    entry.missing,
                    entry.score,
                    entry.accuracy,
                    entry.fluency,
                    entry.weak,
                    entry.usd * self.eur_per_usd,
                    entry.judge_usd * self.eur_per_usd,
                    entry.seconds,
                    entry.at,
                    checks.map(|c| serde_json::to_string(&c)).transpose()?,
                    serde_json::to_string(&entry.failed_checks)?,
                    step1.map(|s| serde_json::to_string(&s)).transpose()?,
                    serde_json::to_string(&step1_failed)?,
                    repair,
                    serde_json::to_string(&problems)?,
                    serde_json::to_string(&history)?,
                ])?;
            }
        }
        tx.commit()?;
        self.status.skipped += skipped;
        self.state_seen = Some(seen);
        Ok(())
    }
}

/// Test runs are left out of everything about translations, like subtrans' own stats do.
pub fn not_test() -> String {
    format!("show_kind != '{}'", ShowKind::Test.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_id_is_stable() {
        assert_eq!(stable_id("/data/series/a.srt"), stable_id("/data/series/a.srt"));
        assert_ne!(stable_id("/data/series/a.srt"), stable_id("/data/series/b.srt"));
        assert_eq!(stable_id("").len(), 16);
    }
}
