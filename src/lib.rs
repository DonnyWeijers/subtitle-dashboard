//! A dashboard for the costs and quality of subtrans, the subtitle translation service.

pub mod api;
pub mod config;
pub mod db;
pub mod model;
pub mod title;

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use axum::Router;
use rusqlite::Connection;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::db::Ingester;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub ingester: Arc<Mutex<Ingester>>,
}

impl AppState {
    pub fn new(data_dir: impl AsRef<Path>, eur_per_usd: f64) -> rusqlite::Result<Self> {
        Ok(Self {
            db: Arc::new(Mutex::new(db::open()?)),
            ingester: Arc::new(Mutex::new(Ingester::new(data_dir.as_ref(), eur_per_usd))),
        })
    }

    /// Reads new data from subtrans' files. Blocking: call it from `spawn_blocking`.
    pub fn ingest(&self) -> Result<(), db::Error> {
        let mut ingester = self.ingester.lock().unwrap_or_else(PoisonError::into_inner);
        let mut conn = self.db.lock().unwrap_or_else(PoisonError::into_inner);
        ingester.run(&mut conn)
    }
}

/// The API under `/api`, and the built frontend (a single-page app) for every other path.
pub fn app(state: AppState, static_dir: Option<&Path>) -> Router {
    let mut router = Router::new().nest("/api", api::router());
    if let Some(dir) = static_dir {
        router = router.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html"))));
    }
    router.layer(TraceLayer::new_for_http()).with_state(state)
}
