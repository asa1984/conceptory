//! Local server for the OKF bundle viewer.
//!
//! Watches files for changes, reloads the OKF bundles, and notifies the viewer of changed
//! documents through Server-Sent Events.

mod assets;
mod http;
mod snapshot;
mod watch;

use std::{
    fmt,
    io::{self, Write as _},
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use conceptory_okf::Schemas;
use tokio::net::TcpListener;

use crate::snapshot::State;

/// Server configuration.
pub struct Config {
    /// OKF bundle roots
    pub roots: Vec<PathBuf>,
    /// Schema directories loaded on top of the built-in schemas
    pub schemas: Vec<PathBuf>,
    /// Port to listen on. 0 picks a free port
    pub port: u16,
    /// Whether to open the default browser after starting
    pub open: bool,
    /// Origins allowed besides the server itself. Pass the dev server origin when developing the
    /// viewer
    pub allowed_origins: Vec<String>,
}

/// A failure to start or run the server.
#[derive(Debug)]
pub enum Error {
    /// An OKF bundle or schema could not be loaded.
    Okf(conceptory_okf::Error),
    /// File watching could not be started.
    Watch(notify_debouncer_full::notify::Error),
    /// The port or runtime could not be set up.
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Okf(error) => error.fmt(f),
            Self::Watch(error) => write!(f, "ファイルの監視を開始できません: {error}"),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<conceptory_okf::Error> for Error {
    fn from(error: conceptory_okf::Error) -> Self {
        Self::Okf(error)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Starts the server and waits until it is stopped with Ctrl-C.
///
/// # Errors
///
/// Returns an error if the OKF bundles cannot be loaded initially, the port cannot be used, or
/// file watching cannot be started.
pub fn serve(config: Config) -> Result<(), Error> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run(config))
}

/// # Errors
///
/// Same as [`serve`].
async fn run(config: Config) -> Result<(), Error> {
    let schemas = Schemas::load(&config.schemas)?;
    let state = Arc::new(State::new(config.roots.clone(), schemas)?);
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, config.port))).await?;
    let address = listener.local_addr()?;
    let url = format!("http://{address}/");

    let _watcher = watch::start(&config.roots, Arc::clone(&state)).map_err(Error::Watch)?;
    let mut stderr = io::stderr();
    writeln!(stderr, "viewer を起動しました: {url}")?;
    writeln!(stderr, "終了するには Ctrl+C を押してください。")?;
    if config.open
        && let Err(error) = opener::open(&url)
    {
        writeln!(stderr, "ブラウザを開けませんでした: {error}")?;
    }

    let router = http::router(state, address.port(), config.allowed_origins);
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            // If the signal cannot be received there is no other way to stop the process, so keep
            // waiting
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
