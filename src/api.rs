//! JSON API under `/api`. All amounts are in euros.

use std::collections::BTreeMap;
use std::sync::PoisonError;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, OpenApi, ToSchema};

use crate::AppState;
use crate::db::{IngestStatus, not_test};
use crate::model::{Attempt, Problem, Redo, Repair, Step1};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "subtitle-dashboard",
        description = "Costs and quality of the subtitle translation service (subtrans). All amounts in euros."
    ),
    paths(summary, costs, translations, translation, checks, shows, health),
    components(schemas(Problem, Step1, Redo, Repair, Attempt, IngestStatus))
)]
pub struct ApiDoc;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/summary", get(summary))
        .route("/costs", get(costs))
        .route("/translations", get(translations))
        .route("/translations/{id}", get(translation))
        .route("/checks", get(checks))
        .route("/shows", get(shows))
        .route("/health", get(health))
        .route("/openapi.json", get(|| async { Json(ApiDoc::openapi()) }))
        .fallback(|| async { ApiError::NotFound })
}

// ---------- errors ----------

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound,
    Internal(String),
}

#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error) = match self {
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            Self::Internal(msg) => {
                tracing::error!("{msg}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_string())
            }
        };
        (status, Json(ErrorBody { error })).into_response()
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

/// Runs a query on the blocking pool; SQLite calls must not block the async runtime.
async fn query<T, F>(state: &AppState, f: F) -> Result<Json<T>, ApiError>
where
    F: FnOnce(&Connection) -> Result<T, ApiError> + Send + 'static,
    T: Send + 'static,
{
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || f(&db.lock().unwrap_or_else(PoisonError::into_inner)))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map(Json)
}

fn json_or_default<T: for<'de> Deserialize<'de> + Default>(text: Option<String>) -> T {
    text.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn check_date(name: &str, value: &str) -> Result<(), ApiError> {
    let ok = value.len() == 10
        && value.char_indices().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() });
    if ok { Ok(()) } else { Err(ApiError::BadRequest(format!("{name} must be YYYY-MM-DD"))) }
}

fn check_month(value: &str) -> Result<(), ApiError> {
    check_date("month", &format!("{value}-01")).map_err(|_| ApiError::BadRequest("month must be YYYY-MM".into()))
}

// ---------- /summary ----------

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SummaryQuery {
    /// YYYY-MM; defaults to the current month.
    month: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct ScoreBucket {
    /// Lower bound, inclusive (0, 10, …, 90).
    from: i64,
    /// Upper bound, exclusive (100 is included in the last bucket).
    to: i64,
    count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct Summary {
    month: String,
    eur_month: f64,
    /// The AI judge's share of `eur_month`.
    eur_month_judge: f64,
    eur_prev_month: f64,
    /// Everything since the first call, test runs included.
    eur_total: f64,
    calls_month: i64,
    translations: i64,
    translations_month: i64,
    /// Translations the AI judge reviewed (a random share).
    judged: i64,
    avg_score: Option<f64>,
    /// Translations step 2 had to redo.
    fallbacks: i64,
    fallback_pct: Option<f64>,
    weak: i64,
    last_translation_at: Option<String>,
    score_buckets: Vec<ScoreBucket>,
}

#[utoipa::path(get, path = "/api/summary", params(SummaryQuery), responses((status = 200, body = Summary)))]
async fn summary(State(state): State<AppState>, Query(q): Query<SummaryQuery>) -> Result<Json<Summary>, ApiError> {
    if let Some(month) = &q.month {
        check_month(month)?;
    }
    query(&state, move |conn| {
        let (month, prev_month): (String, String) = match q.month {
            Some(month) => conn.query_row("SELECT ?1, strftime('%Y-%m', ?1 || '-01', '-1 month')", [&month], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?,
            None => conn.query_row(
                "SELECT strftime('%Y-%m', 'now', 'localtime'),
                        strftime('%Y-%m', 'now', 'localtime', 'start of month', '-1 month')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?,
        };
        let (eur_month, eur_month_judge, calls_month): (f64, f64, i64) = conn.query_row(
            "SELECT COALESCE(SUM(eur), 0), COALESCE(SUM(CASE WHEN kind = 'judge' THEN eur END), 0), COUNT(*)
             FROM calls WHERE month = ?1",
            [&month],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let eur_prev_month: f64 =
            conn.query_row("SELECT COALESCE(SUM(eur), 0) FROM calls WHERE month = ?1", [&prev_month], |r| r.get(0))?;
        let eur_total: f64 = conn.query_row("SELECT COALESCE(SUM(eur), 0) FROM calls", [], |r| r.get(0))?;

        let sql = format!(
            "SELECT COUNT(*), COALESCE(SUM(substr(at, 1, 7) = ?1), 0), COUNT(score), AVG(score),
                    COALESCE(SUM(step = 2), 0), COALESCE(SUM(weak), 0), MAX(at)
             FROM translations WHERE {}",
            not_test()
        );
        #[allow(clippy::type_complexity)]
        let (translations, translations_month, judged, avg_score, fallbacks, weak, last_translation_at): (
            i64,
            i64,
            i64,
            Option<f64>,
            i64,
            i64,
            Option<String>,
        ) = conn.query_row(&sql, [&month], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
        })?;

        let sql = format!(
            "SELECT MIN(CAST(score / 10 AS INTEGER), 9) * 10 AS bucket, COUNT(*)
             FROM translations WHERE score IS NOT NULL AND {} GROUP BY bucket",
            not_test()
        );
        let counts: BTreeMap<i64, i64> =
            conn.prepare(&sql)?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        let score_buckets = (0..10)
            .map(|b| ScoreBucket { from: b * 10, to: b * 10 + 10, count: counts.get(&(b * 10)).copied().unwrap_or(0) })
            .collect();

        Ok(Summary {
            month,
            eur_month,
            eur_month_judge,
            eur_prev_month,
            eur_total,
            calls_month,
            translations,
            translations_month,
            judged,
            avg_score: avg_score.map(|s| (s * 10.0).round() / 10.0),
            fallbacks,
            fallback_pct: (translations > 0).then(|| (1000.0 * fallbacks as f64 / translations as f64).round() / 10.0),
            weak,
            last_translation_at,
            score_buckets,
        })
    })
    .await
}

// ---------- /costs ----------

#[derive(Clone, Copy, Default, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CostGroup {
    #[default]
    Day,
    Month,
    Model,
    Show,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CostsQuery {
    /// day and month are split per model; model and show are totals, most expensive first.
    #[param(inline)]
    group: Option<CostGroup>,
    /// YYYY-MM-DD, inclusive.
    from: Option<String>,
    /// YYYY-MM-DD, inclusive.
    to: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct CostRow {
    /// Day, month, model or show, depending on `group`.
    key: String,
    /// Set for `group=day` and `group=month`.
    model: Option<String>,
    eur: f64,
    calls: i64,
}

#[utoipa::path(get, path = "/api/costs", params(CostsQuery), responses((status = 200, body = Vec<CostRow>)))]
async fn costs(State(state): State<AppState>, Query(q): Query<CostsQuery>) -> Result<Json<Vec<CostRow>>, ApiError> {
    if let Some(from) = &q.from {
        check_date("from", from)?;
    }
    if let Some(to) = &q.to {
        check_date("to", to)?;
    }
    let from = q.from.unwrap_or_else(|| "0000-00-00".into());
    let to = q.to.unwrap_or_else(|| "9999-99-99".into());
    let sql = match q.group.unwrap_or_default() {
        CostGroup::Day => {
            "SELECT day, model, SUM(eur), COUNT(*) FROM calls WHERE day BETWEEN ?1 AND ?2
                           GROUP BY day, model ORDER BY day, model"
        }
        CostGroup::Month => {
            "SELECT month, model, SUM(eur), COUNT(*) FROM calls WHERE day BETWEEN ?1 AND ?2
                             GROUP BY month, model ORDER BY month, model"
        }
        CostGroup::Model => {
            "SELECT model, NULL, SUM(eur), COUNT(*) FROM calls WHERE day BETWEEN ?1 AND ?2
                             GROUP BY model ORDER BY SUM(eur) DESC"
        }
        CostGroup::Show => {
            "SELECT show, NULL, SUM(eur), COUNT(*) FROM calls WHERE day BETWEEN ?1 AND ?2
                            GROUP BY show ORDER BY SUM(eur) DESC"
        }
    };
    query(&state, move |conn| {
        let rows = conn
            .prepare(sql)?
            .query_map(params![from, to], |r| {
                Ok(CostRow { key: r.get(0)?, model: r.get(1)?, eur: r.get(2)?, calls: r.get(3)? })
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    })
    .await
}

// ---------- /translations ----------

#[derive(Clone, Copy, Default, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortBy {
    #[default]
    At,
    Score,
    Eur,
    Show,
}

#[derive(Clone, Copy, Default, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Order {
    Asc,
    #[default]
    Desc,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TranslationsQuery {
    /// Exact show name.
    show: Option<String>,
    weak: Option<bool>,
    /// 1 or 2.
    step: Option<i64>,
    /// Text search in show, episode title and file name.
    q: Option<String>,
    #[param(inline)]
    sort: Option<SortBy>,
    #[param(inline)]
    order: Option<Order>,
    /// Starts at 1.
    page: Option<i64>,
    /// 1–200, default 50.
    per_page: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct TranslationRow {
    id: String,
    show: String,
    /// series or movie.
    show_kind: String,
    season: Option<i64>,
    episode: Option<i64>,
    episode_title: Option<String>,
    /// 1 = first model, 2 = fallback.
    step: i64,
    model: String,
    lines: i64,
    score: Option<f64>,
    weak: bool,
    /// Translation plus judge.
    eur: f64,
    seconds: Option<f64>,
    at: String,
    failed_checks: Vec<String>,
    /// Checks step 1 failed, which started a repair.
    step1_failed: Vec<String>,
    repair: Option<Repair>,
}

#[derive(Serialize, ToSchema)]
pub struct TranslationPage {
    total: i64,
    page: i64,
    per_page: i64,
    items: Vec<TranslationRow>,
}

const ROW_COLUMNS: &str = "id, show, show_kind, season, episode, episode_title, step, model, lines, score, weak,
    eur + judge_eur, seconds, at, failed_checks, step1_failed, repair";

fn translation_row(r: &Row) -> rusqlite::Result<TranslationRow> {
    Ok(TranslationRow {
        id: r.get(0)?,
        show: r.get(1)?,
        show_kind: r.get(2)?,
        season: r.get(3)?,
        episode: r.get(4)?,
        episode_title: r.get(5)?,
        step: r.get(6)?,
        model: r.get(7)?,
        lines: r.get(8)?,
        score: r.get(9)?,
        weak: r.get(10)?,
        eur: r.get(11)?,
        seconds: r.get(12)?,
        at: r.get(13)?,
        failed_checks: json_or_default(r.get(14)?),
        step1_failed: json_or_default(r.get(15)?),
        repair: r.get::<_, Option<String>>(16)?.and_then(|t| serde_json::from_value(Value::String(t)).ok()),
    })
}

fn escape_like(text: &str) -> String {
    text.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

#[utoipa::path(get, path = "/api/translations", params(TranslationsQuery), responses((status = 200, body = TranslationPage)))]
async fn translations(
    State(state): State<AppState>,
    Query(q): Query<TranslationsQuery>,
) -> Result<Json<TranslationPage>, ApiError> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(50).clamp(1, 200);

    let mut filter = format!(" FROM translations WHERE {}", not_test());
    let mut args: Vec<SqlValue> = Vec::new();
    if let Some(show) = q.show {
        filter += " AND show = ?";
        args.push(show.into());
    }
    if let Some(weak) = q.weak {
        filter += " AND weak = ?";
        args.push(i64::from(weak).into());
    }
    if let Some(step) = q.step {
        filter += " AND step = ?";
        args.push(step.into());
    }
    if let Some(text) = q.q.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
        filter += " AND (show LIKE ? ESCAPE '\\' OR episode_title LIKE ? ESCAPE '\\' OR file LIKE ? ESCAPE '\\')";
        let pattern = format!("%{}%", escape_like(&text));
        args.extend(std::iter::repeat_n(SqlValue::from(pattern), 3));
    }

    let dir = match q.order.unwrap_or_default() {
        Order::Asc => "ASC",
        Order::Desc => "DESC",
    };
    let order = match q.sort.unwrap_or_default() {
        SortBy::At => format!("at {dir}"),
        SortBy::Score => format!("score {dir} NULLS LAST, at DESC"),
        SortBy::Eur => format!("eur + judge_eur {dir}, at DESC"),
        SortBy::Show => format!("show {dir}, season {dir}, episode {dir}"),
    };

    query(&state, move |conn| {
        let total = conn.query_row(&format!("SELECT COUNT(*){filter}"), params_from_iter(&args), |r| r.get(0))?;
        let sql =
            format!("SELECT {ROW_COLUMNS}{filter} ORDER BY {order} LIMIT {per_page} OFFSET {}", (page - 1) * per_page);
        let items =
            conn.prepare(&sql)?.query_map(params_from_iter(&args), translation_row)?.collect::<Result<_, _>>()?;
        Ok(TranslationPage { total, page, per_page, items })
    })
    .await
}

#[derive(Serialize, ToSchema)]
pub struct TranslationDetail {
    #[serde(flatten)]
    row: TranslationRow,
    /// Subtitle path inside Bazarr's container.
    path: String,
    file: Option<String>,
    models_used: Vec<String>,
    missing: i64,
    accuracy: Option<f64>,
    fluency: Option<f64>,
    judge_eur: f64,
    /// Share of lines per free check on the final translation, 0–1.
    checks: BTreeMap<String, f64>,
    step1: Option<Step1>,
    problems: Vec<Problem>,
    history: Vec<Attempt>,
}

#[utoipa::path(
    get,
    path = "/api/translations/{id}",
    params(("id" = String, Path)),
    responses((status = 200, body = TranslationDetail), (status = 404, body = ErrorBody))
)]
async fn translation(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TranslationDetail>, ApiError> {
    query(&state, move |conn| {
        let sql = format!(
            "SELECT {ROW_COLUMNS}, path, file, models_used, missing, accuracy, fluency, judge_eur, checks, step1,
                    problems, history
             FROM translations WHERE id = ?1"
        );
        conn.query_row(&sql, [&id], |r| {
            Ok(TranslationDetail {
                row: translation_row(r)?,
                path: r.get(17)?,
                file: r.get(18)?,
                models_used: json_or_default(r.get(19)?),
                missing: r.get(20)?,
                accuracy: r.get(21)?,
                fluency: r.get(22)?,
                judge_eur: r.get(23)?,
                checks: json_or_default(r.get(24)?),
                step1: r.get::<_, Option<String>>(25)?.and_then(|t| serde_json::from_str(&t).ok()),
                problems: json_or_default(r.get(26)?),
                history: json_or_default(r.get(27)?),
            })
        })
        .optional()?
        .ok_or(ApiError::NotFound)
    })
    .await
}

// ---------- /checks ----------

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ChecksQuery {
    /// Look-back window in days, 1–3650, default 30.
    days: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct CheckCount {
    check: String,
    failed: i64,
}

#[derive(Serialize, ToSchema)]
pub struct CheckWeek {
    /// ISO-like week label, 2026-W43 (weeks start on Monday).
    week: String,
    checked: i64,
    failed: i64,
}

#[derive(Serialize, ToSchema)]
pub struct Checks {
    days: i64,
    /// Translations that started at step 1 and went through the free checks.
    checked: i64,
    /// Of those, how many failed a check and were repaired.
    failed: i64,
    fail_pct: Option<f64>,
    /// Still weak after step 2.
    still_weak: i64,
    by_check: Vec<CheckCount>,
    /// How the failures were repaired.
    by_repair: Vec<RepairCount>,
    by_week: Vec<CheckWeek>,
}

#[derive(Serialize, ToSchema)]
pub struct RepairCount {
    repair: Repair,
    count: i64,
}

#[utoipa::path(get, path = "/api/checks", params(ChecksQuery), responses((status = 200, body = Checks)))]
async fn checks(State(state): State<AppState>, Query(q): Query<ChecksQuery>) -> Result<Json<Checks>, ApiError> {
    let days = q.days.unwrap_or(30).clamp(1, 3650);
    query(&state, move |conn| {
        // Same population as subtrans' luna_checks: entries made with the checks that started at step 1.
        let window = format!(
            "{} AND checks IS NOT NULL AND (step = 1 OR step1 IS NOT NULL)
             AND at >= strftime('%Y-%m-%dT%H:%M:%S', 'now', 'localtime', '-{days} days')",
            not_test()
        );
        let (checked, failed, still_weak): (i64, i64, i64) = conn.query_row(
            &format!(
                "SELECT COUNT(*), COALESCE(SUM(step1 IS NOT NULL), 0), COALESCE(SUM(weak), 0)
                 FROM translations WHERE {window}"
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let by_check = conn
            .prepare(&format!(
                "SELECT j.value, COUNT(*) FROM translations t, json_each(t.step1_failed) j
                 WHERE {window} GROUP BY j.value ORDER BY COUNT(*) DESC, j.value"
            ))?
            .query_map([], |r| Ok(CheckCount { check: r.get(0)?, failed: r.get(1)? }))?
            .collect::<Result<_, _>>()?;
        let by_repair = conn
            .prepare(&format!(
                "SELECT repair, COUNT(*) FROM translations WHERE {window} AND repair IS NOT NULL
                 GROUP BY repair ORDER BY COUNT(*) DESC, repair"
            ))?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get(1)?)))?
            .filter_map(|row| match row {
                Ok((repair, count)) => {
                    serde_json::from_value(Value::String(repair)).ok().map(|repair| Ok(RepairCount { repair, count }))
                }
                Err(e) => Some(Err(e)),
            })
            .collect::<Result<_, _>>()?;
        let by_week = conn
            .prepare(&format!(
                "SELECT strftime('%Y-W%W', at) AS week, COUNT(*), COALESCE(SUM(step1 IS NOT NULL), 0)
                 FROM translations WHERE {window} GROUP BY week ORDER BY week"
            ))?
            .query_map([], |r| Ok(CheckWeek { week: r.get(0)?, checked: r.get(1)?, failed: r.get(2)? }))?
            .collect::<Result<_, _>>()?;
        Ok(Checks {
            days,
            checked,
            failed,
            fail_pct: (checked > 0).then(|| (1000.0 * failed as f64 / checked as f64).round() / 10.0),
            still_weak,
            by_check,
            by_repair,
            by_week,
        })
    })
    .await
}

// ---------- /shows ----------

#[derive(Serialize, ToSchema)]
pub struct ShowRow {
    show: String,
    /// series or movie.
    show_kind: String,
    translations: i64,
    /// What the current translations cost (translation plus judge).
    eur: f64,
    /// Every call for this show, retries and replaced translations included.
    eur_spent: f64,
    avg_score: Option<f64>,
    judged: i64,
    weak: i64,
    fallbacks: i64,
    last_at: String,
}

#[utoipa::path(get, path = "/api/shows", responses((status = 200, body = Vec<ShowRow>)))]
async fn shows(State(state): State<AppState>) -> Result<Json<Vec<ShowRow>>, ApiError> {
    query(&state, |conn| {
        let sql = format!(
            "SELECT t.show, MIN(t.show_kind), COUNT(*), SUM(t.eur + t.judge_eur),
                    (SELECT COALESCE(SUM(c.eur), 0) FROM calls c WHERE c.show = t.show),
                    AVG(t.score), COUNT(t.score), SUM(t.weak), SUM(t.step = 2), MAX(t.at)
             FROM translations t WHERE t.{} GROUP BY t.show ORDER BY MAX(t.at) DESC",
            not_test()
        );
        let rows = conn
            .prepare(&sql)?
            .query_map([], |r| {
                Ok(ShowRow {
                    show: r.get(0)?,
                    show_kind: r.get(1)?,
                    translations: r.get(2)?,
                    eur: r.get(3)?,
                    eur_spent: r.get(4)?,
                    avg_score: r.get::<_, Option<f64>>(5)?.map(|s| (s * 10.0).round() / 10.0),
                    judged: r.get(6)?,
                    weak: r.get(7)?,
                    fallbacks: r.get(8)?,
                    last_at: r.get(9)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    })
    .await
}

// ---------- /health ----------

#[utoipa::path(get, path = "/api/health", responses((status = 200, body = IngestStatus)))]
async fn health(State(state): State<AppState>) -> Json<IngestStatus> {
    Json(state.ingester.lock().unwrap_or_else(PoisonError::into_inner).status.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert!(check_date("from", "2026-10-01").is_ok());
        assert!(check_date("from", "2026-10-1").is_err());
        assert!(check_date("from", "2026/10/01").is_err());
        assert!(check_date("from", "2026-10-01' OR 1=1").is_err());
        assert!(check_month("2026-10").is_ok());
        assert!(check_month("2026-1").is_err());
    }

    #[test]
    fn like_is_escaped() {
        assert_eq!(escape_like(r"50%_a\b"), r"50\%\_a\\b");
    }
}
