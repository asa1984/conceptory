//! Checks body links and frontmatter paths (ADR 0001, Write documents as an OKF bundle).
//!
//! Body links are written as relative paths, and frontmatter paths as bundle-relative paths.

use std::path::Path;

use serde_json::Value;

use crate::{
    bundle::{Bundle, Kind, MarkdownFile},
    diagnostic::Diagnostic,
};

pub fn check(bundle: &Bundle, out: &mut Vec<Diagnostic>) {
    for file in &bundle.files {
        check_body_links(bundle, file, out);
        if file.kind == Kind::Concept {
            check_frontmatter_paths(bundle, file, out);
            check_deprecated_by(bundle, file, out);
        }
    }
}

/// Whether the destination starts with a scheme such as `https:` or `mailto:`
fn has_scheme(dest: &str) -> bool {
    let Some(colon) = dest.find(':') else {
        return false;
    };
    let scheme = &dest[..colon];
    scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
}

fn strip_fragment(dest: &str) -> &str {
    dest.split(['#', '?']).next().unwrap_or(dest)
}

fn check_body_links(bundle: &Bundle, file: &MarkdownFile, out: &mut Vec<Diagnostic>) {
    for link in &file.outline.links {
        let dest = link.dest.as_str();
        if dest.is_empty() || dest.starts_with('#') || has_scheme(dest) {
            continue;
        }
        if dest.starts_with('/') {
            out.push(Diagnostic::new(
                &file.display,
                link.line,
                "link-bundle-relative",
                format!(
                    "本文のリンク `{dest}` は相対パスで書く。GitHub は `/` \
                     で始まるリンクをリポジトリのルートから解決する"
                ),
            ));
            continue;
        }
        let target = bundle.root.join(file.dir()).join(strip_fragment(dest));
        if !target.exists() {
            out.push(Diagnostic::new(
                &file.display,
                link.line,
                "link-broken",
                format!("リンク `{dest}` の指す先が存在しません"),
            ));
        }
    }
}

fn check_frontmatter_paths(bundle: &Bundle, file: &MarkdownFile, out: &mut Vec<Diagnostic>) {
    let mut paths: Vec<(&str, &str)> = Vec::new();
    for key in ["resource", "deprecated-by"] {
        if let Some(value) = file.str_field(key) {
            paths.push((key, value));
        }
    }
    if let Some(Value::Array(sources)) = file.field("sources") {
        for source in sources {
            if let Some(value) = source.get("resource").and_then(Value::as_str) {
                paths.push(("sources[].resource", value));
            }
        }
    }

    for (key, value) in paths {
        if has_scheme(value) {
            continue;
        }
        let line = file.line_containing(value);
        match value.strip_prefix('/') {
            None => out.push(Diagnostic::new(
                &file.display,
                line,
                "frontmatter-path",
                format!(
                    "`{key}` のパス `{value}` は bundle-relative（`/` 始まり）で書いてください"
                ),
            )),
            Some(rel) => {
                if !Path::new(&bundle.root).join(strip_fragment(rel)).exists() {
                    out.push(Diagnostic::new(
                        &file.display,
                        line,
                        "frontmatter-path",
                        format!("`{key}` のパス `{value}` の指す先が存在しません"),
                    ));
                }
            },
        }
    }
}

fn check_deprecated_by(bundle: &Bundle, file: &MarkdownFile, out: &mut Vec<Diagnostic>) {
    let Some(value) = file.str_field("deprecated-by") else {
        return;
    };
    let Some(rel) = value.strip_prefix('/') else {
        return;
    };
    let line = file.line_containing("deprecated-by");
    if rel == file.rel {
        out.push(Diagnostic::new(
            &file.display,
            line,
            "deprecated-by",
            "`deprecated-by` が文書自身を指しています",
        ));
        return;
    }
    if let Some(target) = bundle.file(rel)
        && target.doc_type() != Some("ADR")
    {
        out.push(Diagnostic::new(
            &file.display,
            line,
            "deprecated-by",
            format!("`deprecated-by` の指す `{value}` が ADR ではありません"),
        ));
    }
}
