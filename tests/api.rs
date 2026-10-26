//! The API against the fake data in `fixtures/` (no real viewing data).

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use subtitle_dashboard::{AppState, app};
use tower::ServiceExt;

const RATE: f64 = 0.86;

fn fixtures() -> AppState {
    let state = AppState::new(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures"), RATE).unwrap();
    state.ingest().unwrap();
    state
}

async fn get(state: &AppState, uri: &str) -> (StatusCode, Value) {
    let response = app(state.clone(), None).oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

fn eur(v: &Value) -> f64 {
    v.as_f64().expect("a number")
}

fn assert_eur(actual: &Value, usd: f64) {
    let expected = usd * RATE;
    assert!((eur(actual) - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[tokio::test]
async fn health_counts_rows_and_skips() {
    let (status, body) = get(&fixtures(), "/api/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["calls"], 12);
    assert_eq!(body["translations"], 6); // the test run is stored, only left out of the views
    assert_eq!(body["skipped"], 2); // one broken costs line, one broken state entry
    assert_eq!(body["last_error"], Value::Null);
}

#[tokio::test]
async fn summary_for_a_month() {
    let (status, s) = get(&fixtures(), "/api/summary?month=2026-10").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s["month"], "2026-10");
    assert_eur(&s["eur_month"], 0.2191);
    assert_eur(&s["eur_month_judge"], 0.038);
    assert_eur(&s["eur_prev_month"], 0.012);
    assert_eur(&s["eur_total"], 0.2311);
    assert_eq!(s["calls_month"], 10);
    assert_eq!(s["translations"], 5);
    assert_eq!(s["translations_month"], 4);
    assert_eq!(s["judged"], 4);
    assert_eq!(s["avg_score"], 76.3);
    assert_eq!(s["fallbacks"], 2);
    assert_eq!(s["fallback_pct"], 40.0);
    assert_eq!(s["weak"], 1);
    assert_eq!(s["last_translation_at"], "2026-10-11T22:00:00");
    let buckets: Vec<i64> =
        s["score_buckets"].as_array().unwrap().iter().map(|b| b["count"].as_i64().unwrap()).collect();
    assert_eq!(buckets, [0, 0, 0, 0, 0, 1, 0, 1, 1, 1]);
}

#[tokio::test]
async fn bad_parameters_are_400() {
    let state = fixtures();
    assert_eq!(get(&state, "/api/summary?month=october").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(get(&state, "/api/costs?from=2026-1-1").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(get(&state, "/api/costs?group=year").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(get(&state, "/api/translations?sort=title").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(get(&state, "/api/nope").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn costs_grouped() {
    let state = fixtures();

    let (_, rows) = get(&state, "/api/costs?group=model").await;
    let keys: Vec<&str> = rows.as_array().unwrap().iter().map(|r| r["key"].as_str().unwrap()).collect();
    assert_eq!(keys, ["google/gemini-3.7-flash", "anthropic/claude-sonnet-5.5", "openai/gpt-6-luna"]);
    assert_eur(&rows[0]["eur"], 0.17);
    assert_eq!(rows[0]["model"], Value::Null);

    let (_, rows) = get(&state, "/api/costs?group=show").await;
    assert_eq!(rows[0]["key"], "Harbour Watch");
    assert_eur(&rows[0]["eur"], 0.198);
    assert_eq!(rows[0]["calls"], 7);
    assert!(rows.as_array().unwrap().iter().any(|r| r["key"] == "Tests"));

    let (_, rows) = get(&state, "/api/costs?group=day&from=2026-10-05&to=2026-10-05").await;
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 3); // Gemini, Claude and Luna that day
    assert!(rows.iter().all(|r| r["key"] == "2026-10-05" && r["model"].is_string()));

    let (_, rows) = get(&state, "/api/costs?group=month").await;
    let months: Vec<&str> = rows.as_array().unwrap().iter().map(|r| r["key"].as_str().unwrap()).collect();
    assert_eq!(months.first(), Some(&"2026-09"));
    assert_eq!(months.last(), Some(&"2026-10"));
}

#[tokio::test]
async fn translations_filter_sort_and_page() {
    let state = fixtures();

    let (_, page) = get(&state, "/api/translations").await;
    assert_eq!(page["total"], 5);
    assert_eq!(page["items"][0]["show"], "Paper Lanterns"); // newest first
    assert_eq!(page["items"][0]["show_kind"], "movie");

    let (_, page) = get(&state, "/api/translations?weak=true").await;
    assert_eq!(page["total"], 1);
    assert_eq!(page["items"][0]["episode_title"], "Fog");
    assert_eq!(page["items"][0]["step1_failed"], serde_json::json!(["missing", "untranslated"]));
    assert_eq!(page["items"][0]["repair"], "unfixed");

    let (_, page) = get(&state, "/api/translations?step=2&sort=show&order=asc").await;
    let episodes: Vec<i64> = page["items"].as_array().unwrap().iter().map(|r| r["episode"].as_i64().unwrap()).collect();
    assert_eq!(episodes, [2, 3]);

    let (_, page) = get(&state, "/api/translations?sort=score&order=asc").await;
    assert_eq!(page["items"][0]["score"], 58.0);
    assert_eq!(page["items"][4]["score"], Value::Null); // unjudged last

    let (_, page) = get(&state, "/api/translations?q=lantern").await;
    assert_eq!(page["total"], 1);
    let (_, page) = get(&state, "/api/translations?q=%25").await; // a literal %, not a wildcard
    assert_eq!(page["total"], 0);

    let (_, page) = get(&state, "/api/translations?show=Harbour%20Watch&per_page=2&page=2").await;
    assert_eq!((page["total"].as_i64(), page["items"].as_array().unwrap().len()), (Some(3), 1));
}

#[tokio::test]
async fn translation_detail() {
    let state = fixtures();
    let (_, page) = get(&state, "/api/translations?weak=true").await;
    let id = page["items"][0]["id"].as_str().unwrap().to_string();

    let (status, d) = get(&state, &format!("/api/translations/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(d["show"], "Harbour Watch");
    assert_eq!(d["missing"], 14);
    assert_eur(&d["judge_eur"], 0.012);
    assert_eur(&d["eur"], 0.102);
    assert_eq!(d["problems"].as_array().unwrap().len(), 2);
    assert_eq!(d["problems"][0]["kind"], "untranslated");
    assert_eq!(d["step1"]["failed"], serde_json::json!(["missing", "untranslated"]));
    assert_eq!(d["step1"]["redo"].as_array().unwrap().len(), 3);
    assert_eq!(d["step1"]["redo"][2]["batches"], Value::Null); // the whole file
    assert_eq!(d["history"][0]["step"], 2);
    assert_eq!(d["history"][0]["failed"], serde_json::json!(["missing"]));
    assert_eq!(d["checks"]["missing"], 0.024);

    assert_eq!(get(&state, "/api/translations/0000000000000000").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn checks_and_shows() {
    let state = fixtures();

    let (_, c) = get(&state, "/api/checks?days=3650").await;
    assert_eq!((c["checked"].as_i64(), c["failed"].as_i64(), c["still_weak"].as_i64()), (Some(4), Some(3), Some(1)));
    assert_eq!(c["fail_pct"], 75.0);
    let by_repair: Vec<(&str, i64)> = c["by_repair"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["repair"].as_str().unwrap(), r["count"].as_i64().unwrap()))
        .collect();
    assert_eq!(by_repair, [("step1-batches", 1), ("step2-batches", 1), ("unfixed", 1)]);
    let by_check: Vec<(&str, i64)> = c["by_check"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["check"].as_str().unwrap(), r["failed"].as_i64().unwrap()))
        .collect();
    assert_eq!(by_check, [("shifted", 2), ("missing", 1), ("untranslated", 1)]);

    let (_, shows) = get(&state, "/api/shows").await;
    let names: Vec<&str> = shows.as_array().unwrap().iter().map(|r| r["show"].as_str().unwrap()).collect();
    assert_eq!(names, ["Paper Lanterns", "The Night Bakery", "Harbour Watch"]);
    let harbour = &shows[2];
    assert_eq!((harbour["translations"].as_i64(), harbour["weak"].as_i64()), (Some(3), Some(1)));
    assert_eur(&harbour["eur"], 0.194);
    assert_eur(&harbour["eur_spent"], 0.198);
    assert_eq!(harbour["avg_score"], 71.0);
}

/// Copies the fixtures to a temp folder the test can change.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("subtitle-dashboard-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for file in ["costs.jsonl", "state.json"] {
        fs::copy(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(file), dir.join(file)).unwrap();
    }
    dir
}

#[tokio::test]
async fn reads_only_new_complete_lines() {
    let dir = scratch("append");
    let state = AppState::new(&dir, RATE).unwrap();
    state.ingest().unwrap();

    let mut costs = fs::OpenOptions::new().append(true).open(dir.join("costs.jsonl")).unwrap();
    costs.write_all(br#"{"t": "2026-10-12T10:00:00", "kind": "translate", "model": "openai/gpt-6-luna", "usd": 1.0, "title": "Harbour Watch - S01E04 - Rain"}"#).unwrap();
    state.ingest().unwrap();
    assert_eq!(get(&state, "/api/health").await.1["calls"], 12); // no newline yet: still being written

    costs.write_all(b"\n").unwrap();
    state.ingest().unwrap();
    assert_eq!(get(&state, "/api/health").await.1["calls"], 13);
    state.ingest().unwrap();
    assert_eq!(get(&state, "/api/health").await.1["calls"], 13); // nothing read twice

    // A replaced, shorter file is read from the start.
    fs::write(
        dir.join("costs.jsonl"),
        "{\"t\": \"2026-10-12T10:00:00\", \"kind\": \"judge\", \"model\": \"m\", \"usd\": 1.0, \"title\": \"x\"}\n",
    )
    .unwrap();
    state.ingest().unwrap();
    assert_eq!(get(&state, "/api/health").await.1["calls"], 1);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn keeps_old_state_while_the_file_is_half_written() {
    let dir = scratch("state");
    let state = AppState::new(&dir, RATE).unwrap();
    state.ingest().unwrap();

    fs::write(dir.join("state.json"), "{\"/data/series/a.srt\": {\"title\": ").unwrap();
    state.ingest().unwrap();
    assert_eq!(get(&state, "/api/translations").await.1["total"], 5);

    fs::write(
        dir.join("state.json"),
        r#"{"/data/series/a.srt": {"title": "A - S01E01 - B", "tier": 1, "model": "m", "at": "2026-10-20T10:00:00"}}"#,
    )
    .unwrap();
    state.ingest().unwrap();
    let (_, page) = get(&state, "/api/translations").await;
    assert_eq!((page["total"].as_i64(), page["items"][0]["show"].as_str()), (Some(1), Some("A")));
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn missing_files_are_fine() {
    let dir = std::env::temp_dir().join(format!("subtitle-dashboard-empty-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let state = AppState::new(&dir, RATE).unwrap();
    state.ingest().unwrap();
    let (status, s) = get(&state, "/api/summary").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!((s["translations"].as_i64(), s["avg_score"].clone()), (Some(0), Value::Null));
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn openapi_lists_every_endpoint() {
    let (_, spec) = get(&fixtures(), "/api/openapi.json").await;
    for path in [
        "/api/summary",
        "/api/costs",
        "/api/translations",
        "/api/translations/{id}",
        "/api/checks",
        "/api/shows",
        "/api/health",
    ] {
        assert!(spec["paths"][path].is_object(), "{path} missing");
    }
}
