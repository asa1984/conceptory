//! Runs `conceptory okf` subcommands against OKF bundles created in a temporary directory.
#![allow(clippy::expect_used, clippy::missing_panics_doc)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const INDEX: &[&str] = &[
    "---",
    "okf_version: \"0.2\"",
    "---",
    "",
    "# Sample",
    "",
    "- [文章の原則](guide.md) - サンプルの文章の原則を定める。",
];

const GUIDE: &[&str] = &[
    "---",
    "type: StyleGuide",
    "title: 文章の原則",
    "description: サンプルの文章の原則を定める。",
    "tags:",
    "  - sample",
    "vocabulary:",
    "  - avoid: 足す",
    "    prefer:",
    "      - 追加する",
    "generated:",
    "  by: human:tester",
    "  at: 2026-10-10T00:00:00+09:00",
    "---",
    "",
    "# 文章の原則",
    "",
    "本文。",
];

fn text(lines: &[&str]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Prepares a working directory for each test and writes a minimal OKF bundle into it.
fn bundle(name: &str, guide: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("can create the working directory");
    fs::write(root.join("index.md"), text(INDEX)).expect("can write index.md");
    fs::write(root.join("guide.md"), guide).expect("can write guide.md");
    root
}

fn conceptory(args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_conceptory"))
        .args(args)
        .output()
        .expect("can run conceptory");
    (
        output.status.code().expect("has an exit code"),
        String::from_utf8(output.stdout).expect("output is UTF-8"),
    )
}

#[test]
fn okf_check_passes_valid_bundle() {
    let root = bundle("check-valid", &text(GUIDE));
    let (code, stdout) = conceptory(&["okf", "check", root.to_str().expect("path is UTF-8")]);
    assert_eq!(stdout, "");
    assert_eq!(code, 0);
}

#[test]
fn okf_check_fails_on_violation() {
    let root = bundle(
        "check-invalid",
        &text(GUIDE).replace("type: StyleGuide", "type: Unknown"),
    );
    let (code, stdout) = conceptory(&["okf", "check", root.to_str().expect("path is UTF-8")]);
    assert_eq!(code, 1);
    assert!(stdout.contains("[unknown-type]"), "{stdout}");
}

#[test]
fn okf_vocabulary_writes_dictionary() {
    let root = bundle("vocabulary", &text(GUIDE));
    let out = root.join("out/vocabulary.json");
    let (code, _) = conceptory(&[
        "okf",
        "vocabulary",
        root.to_str().expect("path is UTF-8"),
        "--out",
        out.to_str().expect("path is UTF-8"),
    ]);
    assert_eq!(code, 0);
    assert!(
        fs::read_to_string(out)
            .expect("dictionary was written")
            .contains("足す")
    );
}
