//! Checks OKF bundles against the Open Knowledge Format and conceptory's rules, and generates
//! textlint dictionaries from document frontmatter.

mod bundle;
mod checks;
mod diagnostic;
mod markdown;
mod view;

use std::{
    fmt, io,
    path::{Path, PathBuf},
};

pub use crate::{
    bundle::Bundle,
    checks::schema::Schemas,
    diagnostic::Diagnostic,
    view::{DocumentDetail, DocumentSummary, Edge, EdgeKind, Overview, TermView, View},
};

/// A failure to load an OKF bundle or a schema.
#[derive(Debug)]
pub enum Error {
    /// A file or directory could not be read or written.
    Io {
        /// Path that the operation targeted
        path: PathBuf,
        /// Underlying error
        source: io::Error,
    },
    /// A schema could not be parsed or built.
    Schema(String),
}

impl Error {
    fn io(path: &Path, source: io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Schema(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Schema(_) => None,
        }
    }
}

/// Loaded OKF bundles and the problems found while loading them.
pub struct Loaded {
    /// Loaded OKF bundles
    pub bundles: Vec<Bundle>,
    /// Problems found while loading, such as frontmatter that cannot be parsed as YAML
    pub diagnostics: Vec<Diagnostic>,
}

/// Loads each of `roots` as the root of an OKF bundle.
///
/// # Errors
///
/// Returns [`Error::Io`] if a directory or file cannot be read.
pub fn load(roots: &[PathBuf]) -> Result<Loaded, Error> {
    let mut diagnostics = Vec::new();
    let bundles = roots
        .iter()
        .map(|root| Bundle::load(root, &mut diagnostics))
        .collect::<Result<_, _>>()?;
    Ok(Loaded {
        bundles,
        diagnostics,
    })
}

/// Runs the per-bundle checks, plus the checks that span OKF bundles, such as terms and avoided
/// words.
///
/// Returns every problem, including those found while loading, sorted by path and line.
#[must_use]
pub fn check(loaded: &Loaded, schemas: &Schemas) -> Vec<Diagnostic> {
    let mut diagnostics = loaded.diagnostics.clone();
    diagnostics.extend(checks::run(&loaded.bundles, schemas));
    diagnostics.sort();
    diagnostics.dedup();
    diagnostics
}

/// Generates the content of each directory's `index.md`. Returns pairs of target path and content.
#[must_use]
pub fn indexes(bundles: &[Bundle]) -> Vec<(PathBuf, String)> {
    bundles.iter().flat_map(checks::index::generate).collect()
}

/// Generates a prh dictionary in YAML from the `terms` in frontmatter.
#[must_use]
pub fn prh_dictionary(bundles: &[Bundle]) -> String {
    checks::terms::prh(bundles)
}

/// Generates a list of the terms in frontmatter `terms`, each with the documents that define it.
#[must_use]
pub fn term_list(bundles: &[Bundle]) -> String {
    checks::terms::list(bundles)
}

/// Generates a textlint-rule-preset-ai-words-ja dictionary in JSON from the `vocabulary` in
/// frontmatter.
#[must_use]
pub fn vocabulary_dictionary(bundles: &[Bundle]) -> String {
    checks::vocabulary::dictionary(bundles)
}

/// Builds the display data for the viewer. Pass the result of [`check`] as `diagnostics`.
#[must_use]
pub fn view(loaded: &Loaded, diagnostics: Vec<Diagnostic>) -> View {
    view::build(loaded, diagnostics)
}
