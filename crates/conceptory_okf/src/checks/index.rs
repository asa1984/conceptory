//! Checks that each directory's index.md correctly lists the documents under it, and generates
//! index.md files.

use std::{collections::BTreeSet, path::PathBuf};

use crate::{
    bundle::{Bundle, Kind, MarkdownFile, split_frontmatter},
    diagnostic::Diagnostic,
};

struct Entry {
    title: String,
    url: String,
    description: Option<String>,
}

/// Parses lines of the form `- [Title](url) - description`. The leading marker may be `-` or `*`
fn parse_entries(source: &str) -> Vec<Entry> {
    source
        .lines()
        .filter_map(|line| {
            let rest = line
                .strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))?;
            let rest = rest.strip_prefix('[')?;
            let (title, rest) = rest.split_once("](")?;
            let (url, rest) = rest.split_once(')')?;
            let description = rest.strip_prefix(" - ").map(|d| d.trim().to_string());
            Some(Entry {
                title: title.to_string(),
                url: url.to_string(),
                description,
            })
        })
        .collect()
}

/// Directories containing Markdown files. The root is the empty string
fn content_dirs(bundle: &Bundle) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    for file in &bundle.files {
        let mut dir = file.dir();
        loop {
            dirs.insert(dir.to_string());
            match dir.rfind('/') {
                Some(i) => dir = &dir[..i],
                None if !dir.is_empty() => dir = "",
                None => break,
            }
        }
    }
    dirs
}

fn parent_of(dir: &str) -> Option<&str> {
    if dir.is_empty() {
        return None;
    }
    Some(dir.rfind('/').map_or("", |i| &dir[..i]))
}

fn base_name(dir: &str) -> &str {
    dir.rfind('/').map_or(dir, |i| &dir[i + 1..])
}

fn index_rel(dir: &str) -> String {
    if dir.is_empty() {
        "index.md".to_string()
    } else {
        format!("{dir}/index.md")
    }
}

fn documents_in<'a>(bundle: &'a Bundle, dir: &'a str) -> impl Iterator<Item = &'a MarkdownFile> {
    bundle
        .files
        .iter()
        .filter(move |file| file.kind == Kind::Concept && file.dir() == dir)
}

fn children<'a>(dirs: &'a BTreeSet<String>, dir: &'a str) -> impl Iterator<Item = &'a str> {
    dirs.iter()
        .map(String::as_str)
        .filter(move |child| parent_of(child) == Some(dir))
}

pub fn check(bundle: &Bundle, out: &mut Vec<Diagnostic>) {
    let dirs = content_dirs(bundle);
    for dir in &dirs {
        let rel = index_rel(dir);
        let Some(index) = bundle.file(&rel) else {
            out.push(Diagnostic::new(
                bundle.root.join(&rel).display().to_string(),
                1,
                "index-missing",
                "index.md がありません",
            ));
            continue;
        };
        let entries = parse_entries(&index.source);

        for document in documents_in(bundle, dir) {
            let name = document.file_name();
            let Some(entry) = entries.iter().find(|e| e.url == name) else {
                out.push(Diagnostic::new(
                    &index.display,
                    1,
                    "index-unlisted",
                    format!("{name} が一覧に載っていません"),
                ));
                continue;
            };
            let line = index.line_containing(&format!("]({name})"));
            let title = document.str_field("title").unwrap_or(name);
            if entry.title != title {
                out.push(Diagnostic::new(
                    &index.display,
                    line,
                    "index-stale",
                    format!("{name} のタイトルが `title` の「{title}」と一致しません"),
                ));
            }
            let description = document.str_field("description");
            if entry.description.as_deref() != description {
                out.push(Diagnostic::new(
                    &index.display,
                    line,
                    "index-stale",
                    format!("{name} の説明が `description` と一致しません"),
                ));
            }
        }

        for child in children(&dirs, dir) {
            let url = format!("{}/", base_name(child));
            if !entries.iter().any(|e| e.url == url) {
                out.push(Diagnostic::new(
                    &index.display,
                    1,
                    "index-unlisted",
                    format!("ディレクトリ {url} が一覧に載っていません"),
                ));
            }
        }
    }
}

/// Generates the content of each directory's index.md.
///
/// The heading and the root frontmatter are carried over from the existing index.md. Document
/// entries are rebuilt from each document's frontmatter. Directory entries have no source for a
/// description, so an existing line is reused as-is.
pub fn generate(bundle: &Bundle) -> Vec<(PathBuf, String)> {
    let dirs = content_dirs(bundle);
    dirs.iter()
        .map(|dir| {
            let rel = index_rel(dir);
            let content = index_content(bundle, &dirs, dir, bundle.file(&rel));
            (bundle.root.join(rel), content)
        })
        .collect()
}

/// Builds the index.md content for one directory. `existing` is the current index.md.
fn index_content(
    bundle: &Bundle,
    dirs: &BTreeSet<String>,
    dir: &str,
    existing: Option<&MarkdownFile>,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(yaml) = existing.and_then(|index| split_frontmatter(&index.source)) {
        lines.push(format!("---\n{yaml}---\n"));
    }

    let heading = existing
        .and_then(|index| index.outline.headings.iter().find(|h| h.level == 1))
        .map_or_else(
            || match base_name(dir) {
                "" => "Index".to_owned(),
                name => name.to_owned(),
            },
            |h| h.text.clone(),
        );
    lines.push(format!("# {heading}\n"));

    let existing_lines: Vec<&str> =
        existing.map_or_else(Vec::new, |index| index.source.lines().collect());
    for child in children(dirs, dir) {
        let name = base_name(child);
        let url = format!("]({name}/)");
        lines.push(
            existing_lines
                .iter()
                .find(|line| line.contains(&url))
                .map_or_else(|| format!("- [{name}]({name}/)"), ToString::to_string),
        );
    }
    for document in documents_in(bundle, dir) {
        let name = document.file_name();
        let title = document.str_field("title").unwrap_or(name);
        lines.push(document.str_field("description").map_or_else(
            || format!("- [{title}]({name})"),
            |description| format!("- [{title}]({name}) - {description}"),
        ));
    }

    let mut content = lines.join("\n");
    content.push('\n');
    content
}
