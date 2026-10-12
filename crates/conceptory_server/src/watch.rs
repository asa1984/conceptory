//! Watches the OKF bundle roots and reloads on changes.
//!
//! Editors save files in different ways, so the kinds of events that arrive vary too. Instead of
//! dispatching on the event kind, this reloads the whole OKF bundle after receiving a batch of
//! changes within a short interval, and [`State::reload`] determines the display differences by
//! per-document hashes.

use std::{path::PathBuf, sync::Arc, time::Duration};

use notify_debouncer_full::{
    DebounceEventResult, Debouncer, RecommendedCache, new_debouncer,
    notify::{self, RecommendedWatcher, RecursiveMode},
};
use tokio::sync::mpsc;

use crate::snapshot::State;

/// Interval over which changes are batched
const DEBOUNCE: Duration = Duration::from_millis(200);

/// Starts watching. Dropping the return value stops watching.
///
/// # Errors
///
/// Returns an error if a root cannot be watched.
pub fn start(
    roots: &[PathBuf],
    state: Arc<State>,
) -> Result<Debouncer<RecommendedWatcher, RecommendedCache>, notify::Error> {
    let (sender, mut receiver) = mpsc::unbounded_channel::<()>();
    let mut debouncer = new_debouncer(DEBOUNCE, None, move |result: DebounceEventResult| {
        if result.is_ok_and(|events| !events.is_empty()) {
            // The receiver only stops when the server shuts down, so a failed send is fine
            let _ = sender.send(());
        }
    })?;
    for root in roots {
        debouncer.watch(root, RecursiveMode::Recursive)?;
    }

    tokio::spawn(async move {
        while receiver.recv().await.is_some() {
            // Collapse notifications that arrived during a reload into the next single reload
            while receiver.try_recv().is_ok() {}
            let state = Arc::clone(&state);
            // Keep watching even if a reload panics
            let _ = tokio::task::spawn_blocking(move || state.reload()).await;
        }
    });
    Ok(debouncer)
}
