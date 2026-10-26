use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::process::ExitCode;
use std::time::Duration;

use axum::serve::ListenerExt;
use subtitle_dashboard::api::ApiDoc;
use subtitle_dashboard::config::Config;
use subtitle_dashboard::{AppState, app};
use tokio::signal::unix::{SignalKind, signal};
use utoipa::OpenApi;

#[tokio::main]
async fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--openapi") => {
            println!("{}", ApiDoc::openapi().to_pretty_json().expect("serialisable spec"));
            return ExitCode::SUCCESS;
        }
        Some("--healthcheck") => return healthcheck(),
        Some(other) => {
            eprintln!("unknown argument {other}; use --openapi or --healthcheck, or none to serve");
            return ExitCode::FAILURE;
        }
        None => {}
    }

    tracing_subscriber::fmt().with_target(false).init();
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => {
            tracing::error!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let state = match AppState::new(&config.data_dir, config.eur_per_usd) {
        Ok(state) => state,
        Err(e) => {
            tracing::error!("can't open the database: {e}");
            return ExitCode::FAILURE;
        }
    };
    // First read before listening, so the first page load has data.
    if let Err(e) = state.ingest() {
        tracing::error!("reading {}: {e}", config.data_dir.display());
    }
    tracing::info!("read {}: {:?}", config.data_dir.display(), state.ingester.lock().unwrap().status);

    let poller = state.clone();
    let poll = config.poll;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(poll);
        interval.tick().await;
        loop {
            interval.tick().await;
            let state = poller.clone();
            match tokio::task::spawn_blocking(move || state.ingest()).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => tracing::warn!("ingest: {e}"),
                Err(e) => tracing::error!("ingest task: {e}"),
            }
        }
    });

    let static_dir = config.static_dir.is_dir().then_some(config.static_dir.as_path());
    if static_dir.is_none() {
        tracing::warn!("{} not found: serving the API only", config.static_dir.display());
    }
    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        // No Nagle: a file response is written as headers + body, and Nagle with the client's delayed ACK held
        // the body back ~40 ms per request.
        Ok(listener) => listener.tap_io(|tcp| {
            if let Err(e) = tcp.set_nodelay(true) {
                tracing::warn!("TCP_NODELAY: {e}");
            }
        }),
        Err(e) => {
            tracing::error!("can't listen on {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };
    tracing::info!("listening on {addr}");
    if let Err(e) = axum::serve(listener, app(state, static_dir)).with_graceful_shutdown(shutdown()).await {
        tracing::error!("{e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

async fn shutdown() {
    let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
}

/// For the container's HEALTHCHECK: the distroless image has no curl.
fn healthcheck() -> ExitCode {
    let port = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080u16);
    let check = || -> std::io::Result<bool> {
        let mut stream =
            TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)), Duration::from_secs(3))?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        stream.write_all(b"GET /api/health HTTP/1.0\r\nHost: localhost\r\n\r\n")?;
        let mut head = [0u8; 12];
        stream.read_exact(&mut head)?;
        Ok(head.ends_with(b" 200"))
    };
    if matches!(check(), Ok(true)) { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
