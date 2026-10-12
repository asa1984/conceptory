//! Reloads the OKF bundles and notifies the viewer of the differences from the previous load.

use std::{
    io::{self, Write as _},
    path::PathBuf,
    process,
    sync::{Arc, PoisonError, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use conceptory_okf::{Schemas, View};
use serde::Serialize;
use tokio::sync::broadcast;

/// Display data of the OKF bundles at a point in time.
pub struct Snapshot {
    /// Number incremented on every reload
    pub revision: u64,
    pub view: View,
}

/// An event sent to the viewer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Event {
    /// Documents changed. If `overview` is true, the list and graph must be refetched too
    Changed {
        revision: u64,
        changed: Vec<String>,
        removed: Vec<String>,
        overview: bool,
    },
    /// The reload failed. The display stays as it was
    Error { message: String },
}

/// Server state.
pub struct State {
    /// Identifier that changes on every start. The viewer refetches everything when it changes
    pub server_id: String,
    roots: Vec<PathBuf>,
    schemas: Schemas,
    snapshot: RwLock<Arc<Snapshot>>,
    events: broadcast::Sender<Event>,
}

impl State {
    /// Loads the OKF bundles and builds the initial state.
    ///
    /// # Errors
    ///
    /// Returns an error if the OKF bundles cannot be loaded.
    pub fn new(roots: Vec<PathBuf>, schemas: Schemas) -> Result<Self, conceptory_okf::Error> {
        let view = build(&roots, &schemas)?;
        let started = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let (events, _) = broadcast::channel(64);
        Ok(Self {
            server_id: format!("{:x}-{started:x}", process::id()),
            roots,
            schemas,
            snapshot: RwLock::new(Arc::new(Snapshot { revision: 0, view })),
            events,
        })
    }

    pub fn snapshot(&self) -> Arc<Snapshot> {
        Arc::clone(&self.snapshot.read().unwrap_or_else(PoisonError::into_inner))
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    /// Reloads the OKF bundles and sends a notification if any document changed.
    pub fn reload(&self) {
        let view = match build(&self.roots, &self.schemas) {
            Ok(view) => view,
            Err(error) => {
                let message = error.to_string();
                // Even if stderr is unwritable, still notify the viewer
                let _ = writeln!(
                    io::stderr(),
                    "OKF bundle を読み込み直せませんでした: {message}"
                );
                let _ = self.events.send(Event::Error { message });
                return;
            },
        };
        let previous = self.snapshot();
        let changed: Vec<String> = view
            .details
            .iter()
            .filter(|(id, detail)| {
                previous
                    .view
                    .details
                    .get(*id)
                    .is_none_or(|old| old.summary.hash != detail.summary.hash)
            })
            .map(|(id, _)| id.clone())
            .collect();
        let removed: Vec<String> = previous
            .view
            .details
            .keys()
            .filter(|id| !view.details.contains_key(*id))
            .cloned()
            .collect();
        let overview = previous.view.overview != view.overview;
        if changed.is_empty() && removed.is_empty() && !overview {
            return;
        }
        let revision = previous.revision + 1;
        *self
            .snapshot
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Arc::new(Snapshot { revision, view });
        // Sending fails when no viewer is subscribed, which is fine
        let _ = self.events.send(Event::Changed {
            revision,
            changed,
            removed,
            overview,
        });
    }
}

/// Loads and checks the OKF bundles and builds the display data.
///
/// # Errors
///
/// Returns an error if the OKF bundles cannot be loaded.
fn build(roots: &[PathBuf], schemas: &Schemas) -> Result<View, conceptory_okf::Error> {
    let loaded = conceptory_okf::load(roots)?;
    let diagnostics = conceptory_okf::check(&loaded, schemas);
    Ok(conceptory_okf::view(&loaded, diagnostics))
}

#[cfg(test)]
#[allow(
    clippy::missing_panics_doc,
    reason = "tests report failures by panicking"
)]
mod tests {
    use std::{env, fs, path::Path};

    use super::*;

    /// Creates an OKF bundle in a separate temporary directory for each test
    fn bundle(name: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("conceptory-server-{}-{name}", process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("can create the directory");
        fs::write(
            root.join("index.md"),
            "---\nokf_version: \"0.2\"\n---\n\n# Root\n",
        )
        .expect("can write the file");
        fs::write(root.join("a.md"), "---\ntype: Note\n---\n\n# A\n").expect("can write the file");
        root
    }

    fn state(root: &Path) -> State {
        let schemas = Schemas::load(&[]).expect("can load the built-in schemas");
        State::new(vec![root.to_path_buf()], schemas).expect("can load the bundle")
    }

    #[test]
    fn reload_reports_changed_and_removed_documents() {
        let root = bundle("reload");
        let state = state(&root);
        let mut events = state.subscribe();

        fs::write(root.join("a.md"), "---\ntype: Note\n---\n\n# A\n\nBody.\n")
            .expect("can write the file");
        state.reload();
        let a = root.join("a.md").display().to_string();
        // The title is unchanged, but the hash in the listed summary changes, so the list is
        // refetched too
        assert_eq!(
            events.try_recv().expect("change is notified"),
            Event::Changed {
                revision: 1,
                changed: vec![a.clone()],
                removed: Vec::new(),
                overview: true,
            }
        );

        fs::remove_file(root.join("a.md")).expect("can remove the file");
        state.reload();
        assert_eq!(
            events.try_recv().expect("removal is notified"),
            Event::Changed {
                revision: 2,
                changed: Vec::new(),
                removed: vec![a],
                overview: true,
            }
        );
        assert_eq!(state.snapshot().revision, 2);
    }

    #[test]
    fn reload_without_changes_sends_nothing() {
        let root = bundle("unchanged");
        let state = state(&root);
        let mut events = state.subscribe();
        state.reload();
        assert!(events.try_recv().is_err());
        assert_eq!(state.snapshot().revision, 0);
    }
}
