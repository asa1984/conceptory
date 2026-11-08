//! Runs the file name check against files created in a temporary directory.
#![allow(clippy::expect_used, clippy::missing_panics_doc)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};

/// Prepares a working directory for each test and creates `files` in it as empty files.
///
/// `CARGO_TARGET_TMPDIR` lives under `target/`, which the repository's `.gitignore` excludes, so
/// this uses the system temporary directory instead.
fn tree(name: &str, files: &[&str]) -> PathBuf {
    let root = env::temp_dir().join(format!("conceptory-files-{}-{name}", process::id()));
    let _ = fs::remove_dir_all(&root);
    for file in files {
        let path = root.join(file);
        fs::create_dir_all(path.parent().expect("file has a parent directory"))
            .expect("can create the directory");
        fs::write(&path, "").expect("can write the file");
    }
    fs::create_dir_all(&root).expect("can create the working directory");
    root
}

/// Writes `content` to `file` under `root`.
fn write(root: &Path, file: &str, content: &str) {
    let path = root.join(file);
    fs::create_dir_all(path.parent().expect("file has a parent directory"))
        .expect("can create the directory");
    fs::write(&path, content).expect("can write the file");
}

fn violations(root: &Path, excludes: &[&str]) -> Vec<String> {
    let excludes: Vec<String> = excludes.iter().map(ToString::to_string).collect();
    conceptory_files::check(root, &excludes)
        .expect("check succeeds")
        .into_iter()
        .map(|diagnostic| {
            Path::new(&diagnostic.path)
                .strip_prefix(root)
                .expect("diagnostic path is under root")
                .display()
                .to_string()
        })
        .collect()
}

#[test]
fn reports_yml_files() {
    let root = tree("reports", &[
        "a.yml",
        "b.yaml",
        ".hidden.yml",
        "nested/c.YML",
        "d.txt",
    ]);
    assert_eq!(violations(&root, &[]), [
        ".hidden.yml",
        "a.yml",
        "nested/c.YML"
    ]);
}

#[test]
fn skips_generated_and_ignored_files() {
    let root = tree("skips", &[
        "rules/__snapshots__/no-await-snapshot.yml",
        "rules/__snapshots__/typescript/no-function-snapshot.yml",
        "ignored.yml",
        "vendor/tool.yml",
        "kept.yml",
    ]);
    fs::write(root.join(".gitignore"), "ignored.yml\n").expect("can write .gitignore");
    assert_eq!(violations(&root, &["vendor/**"]), ["kept.yml"]);
}

#[test]
fn rejects_invalid_pattern() {
    let root = tree("invalid", &[]);
    assert!(conceptory_files::check(&root, &["[".to_owned()]).is_err());
}

#[test]
fn reports_workflow_names_that_differ_from_file_names() {
    let root = tree("workflows", &[]);
    write(
        &root,
        ".github/workflows/check.yaml",
        "name: check\non: push\n",
    );
    write(
        &root,
        ".github/workflows/quoted.yaml",
        "name: \"quoted\"\non: push\n",
    );
    write(
        &root,
        ".github/workflows/renamed.yaml",
        "on: push\nname: CI\n",
    );
    write(&root, ".github/workflows/unnamed.yaml", "on: push\n");
    write(&root, ".github/workflows/nested/ignored.yaml", "on: push\n");
    write(&root, ".github/dependabot.yaml", "version: 2\n");
    write(&root, "config.yaml", "on: push\n");
    let diagnostics = conceptory_files::check(&root, &[]).expect("check succeeds");
    let found: Vec<(String, usize)> = diagnostics
        .iter()
        .map(|diagnostic| {
            let path = Path::new(&diagnostic.path)
                .strip_prefix(&root)
                .expect("diagnostic path is under root")
                .display()
                .to_string();
            (path, diagnostic.line)
        })
        .collect();
    assert_eq!(found, [
        (".github/workflows/renamed.yaml".to_owned(), 2),
        (".github/workflows/unnamed.yaml".to_owned(), 1),
    ]);
}
