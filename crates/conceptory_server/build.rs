//! Decides where the viewer build output to embed is located.
//!
//! Uses the environment variable `CONCEPTORY_VIEWER_DIST` if set, and the repository's
//! `viewer/dist` otherwise. Nix builds pass the location of the viewer built in a separate
//! derivation through this variable.
#![allow(
    clippy::print_stdout,
    reason = "build scripts pass instructions to cargo through stdout"
)]

use std::{env, path::PathBuf};

fn main() {
    println!("cargo::rerun-if-env-changed=CONCEPTORY_VIEWER_DIST");
    let dist = env::var("CONCEPTORY_VIEWER_DIST").unwrap_or_else(|_| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../viewer/dist")
            .display()
            .to_string()
    });
    println!("cargo::rerun-if-changed={dist}");
    println!("cargo::rustc-env=CONCEPTORY_VIEWER_DIST={dist}");
}
