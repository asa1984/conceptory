//! Tests the display data passed to the viewer.
#![allow(clippy::expect_used, clippy::missing_panics_doc)]

use std::path::{Path, PathBuf};

use conceptory_okf::{DocumentDetail, EdgeKind, View};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/view")
}

fn id(rel: &str) -> String {
    root().join(rel).display().to_string()
}

fn view() -> View {
    let loaded = conceptory_okf::load(&[root()]).expect("can load the fixture");
    conceptory_okf::view(&loaded, Vec::new())
}

fn detail<'v>(view: &'v View, rel: &str) -> &'v DocumentDetail {
    view.details.get(&id(rel)).expect("document exists")
}

#[test]
fn resolves_links_backlinks_and_deprecation() {
    let view = view();
    let a = detail(&view, "notes/a.md");
    assert_eq!(a.links, [id("notes/b.md")]);
    assert_eq!(a.summary.deprecated_by, Some(id("notes/b.md")));
    // Links from indexes do not count as backlinks
    assert_eq!(detail(&view, "notes/b.md").backlinks, [id("notes/a.md")]);

    let edges: Vec<_> = view
        .overview
        .edges
        .iter()
        .map(|edge| (edge.source.clone(), edge.target.clone(), edge.kind))
        .collect();
    assert_eq!(edges, [
        (id("notes/a.md"), id("notes/b.md"), EdgeKind::Link),
        (id("notes/a.md"), id("notes/b.md"), EdgeKind::DeprecatedBy),
    ]);
}

#[test]
fn rewrites_urls_for_viewer() {
    let view = view();
    let html = &detail(&view, "notes/a.md").html;
    let doc = format!("href=\"/?doc={}#details\"", id("notes/b.md"));
    assert!(html.contains(&doc), "{html}");
    let file = format!("src=\"/files?path={}\"", id("notes/figure.svg"));
    assert!(html.contains(&file), "{html}");
    assert!(html.contains("href=\"https://example.com/\""), "{html}");
}

#[test]
fn renders_github_markdown_safely() {
    let view = view();
    let html = &detail(&view, "notes/a.md").html;
    assert!(html.contains("markdown-alert-note"), "{html}");
    assert!(html.contains("id=\"user-content-a\""), "{html}");
    assert!(!html.contains("<script"), "{html}");
    assert!(!html.contains("onerror"), "{html}");
}

#[test]
fn hash_changes_only_with_content() {
    let first = view();
    let second = view();
    assert_eq!(first.overview, second.overview);
    let a = &detail(&first, "notes/a.md").summary.hash;
    let b = &detail(&first, "notes/b.md").summary.hash;
    assert_ne!(a, b);
}
