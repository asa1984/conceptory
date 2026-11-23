//! The style guides in conceptory's `guides/`, embedded in the binary.
//!
//! Downstream repositories generate dictionaries with their vocabulary through this (ADR 0019
//! "Distribute artifacts via npm, Nix flake, and the conceptory CLI"). Debug builds read from disk
//! instead of embedding, so edits to the guides take effect without a rebuild.

use rust_embed::RustEmbed;

use crate::bundle::Bundle;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../guides"]
#[include = "*.md"]
struct Guides;

/// Prefix of the paths shown for the embedded style guides
const DISPLAY_ROOT: &str = "conceptory/guides";

/// Loads the embedded style guides as an OKF bundle.
pub fn load() -> Bundle {
    let sources = Guides::iter().filter_map(|rel| {
        let file = Guides::get(&rel)?;
        let source = String::from_utf8_lossy(&file.data).into_owned();
        Some((rel.into_owned(), source))
    });
    // The guides are checked by `conceptory okf check` in conceptory itself, so problems found
    // while parsing them are not reported again here.
    Bundle::from_sources(DISPLAY_ROOT, sources, &mut Vec::new())
}
