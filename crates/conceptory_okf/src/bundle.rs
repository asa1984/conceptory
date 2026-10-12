//! Loads Markdown files from the root of an OKF bundle.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;

use crate::{
    Error,
    diagnostic::Diagnostic,
    markdown::{self, Outline},
};

pub struct Bundle {
    pub root: PathBuf,
    pub files: Vec<MarkdownFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Concept,
    Index,
    Log,
}

pub enum Frontmatter {
    Missing,
    /// Could not be parsed as YAML. The problem was already reported while loading
    Invalid,
    Parsed(Value),
}

pub struct MarkdownFile {
    /// Path from the OKF bundle root, separated by `/`
    pub rel: String,
    /// Path shown in diagnostics
    pub display: String,
    pub kind: Kind,
    pub source: String,
    pub frontmatter: Frontmatter,
    pub outline: Outline,
}

impl MarkdownFile {
    /// Path within the OKF bundle of the directory containing the file. Empty for the root
    pub fn dir(&self) -> &str {
        self.rel.rfind('/').map_or("", |i| &self.rel[..i])
    }

    pub fn file_name(&self) -> &str {
        self.rel
            .rfind('/')
            .map_or(&self.rel, |i| &self.rel[i + 1..])
    }

    pub fn field(&self, key: &str) -> Option<&Value> {
        match &self.frontmatter {
            Frontmatter::Parsed(value) => value.get(key),
            _ => None,
        }
    }

    pub fn str_field(&self, key: &str) -> Option<&str> {
        self.field(key).and_then(Value::as_str)
    }

    pub fn doc_type(&self) -> Option<&str> {
        self.str_field("type")
    }

    pub fn is_deprecated(&self) -> bool {
        self.field("deprecated-by").is_some()
    }

    /// Line number of the first line containing `needle`, or 1 if none does
    pub fn line_containing(&self, needle: &str) -> usize {
        self.source
            .lines()
            .position(|line| line.contains(needle))
            .map_or(1, |i| i + 1)
    }
}

impl Bundle {
    /// Loads every Markdown file under `root`, treating it as the root of an OKF bundle.
    ///
    /// Files whose frontmatter cannot be parsed are still loaded, and the problem is added to
    /// `diagnostics`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if a directory or file cannot be read.
    pub fn load(root: &Path, diagnostics: &mut Vec<Diagnostic>) -> Result<Self, Error> {
        let mut files = Vec::new();
        visit(root, root, "", &mut files, diagnostics)?;
        Ok(Self {
            root: root.to_path_buf(),
            files,
        })
    }

    #[must_use]
    pub fn file(&self, rel: &str) -> Option<&MarkdownFile> {
        self.files.iter().find(|file| file.rel == rel)
    }

    pub fn concepts(&self) -> impl Iterator<Item = &MarkdownFile> {
        self.files.iter().filter(|file| file.kind == Kind::Concept)
    }
}

/// Walks `dir` in name order and adds the Markdown files to `files`.
///
/// # Errors
///
/// Returns [`Error::Io`] if a directory or file cannot be read.
fn visit(
    root: &Path,
    dir: &Path,
    rel_dir: &str,
    files: &mut Vec<MarkdownFile>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), Error> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .and_then(Iterator::collect)
        .map_err(|e| Error::io(dir, e))?;
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let rel = if rel_dir.is_empty() {
            name.clone()
        } else {
            format!("{rel_dir}/{name}")
        };
        let path = entry.path();
        if entry.file_type().map_err(|e| Error::io(&path, e))?.is_dir() {
            visit(root, &path, &rel, files, diagnostics)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        {
            files.push(read_file(root, rel, &path, diagnostics)?);
        }
    }
    Ok(())
}

/// Reads one Markdown file and parses its frontmatter and body structure.
///
/// # Errors
///
/// Returns [`Error::Io`] if the file cannot be read.
fn read_file(
    root: &Path,
    rel: String,
    path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<MarkdownFile, Error> {
    let source = fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
    let display = root.join(&rel).display().to_string();
    let kind = match path.file_name().and_then(|name| name.to_str()) {
        Some("index.md") => Kind::Index,
        Some("log.md") => Kind::Log,
        _ => Kind::Concept,
    };
    let frontmatter = match split_frontmatter(&source) {
        None => Frontmatter::Missing,
        Some(yaml) if yaml.trim().is_empty() => Frontmatter::Parsed(Value::Null),
        Some(yaml) => match serde_saphyr::from_str::<Value>(yaml) {
            Ok(value) => Frontmatter::Parsed(value),
            Err(error) => {
                diagnostics.push(Diagnostic::new(
                    &display,
                    1,
                    "okf-frontmatter",
                    format!("frontmatter を YAML として解析できません: {error}"),
                ));
                Frontmatter::Invalid
            },
        },
    };
    let outline = markdown::parse(&source);
    Ok(MarkdownFile {
        rel,
        display,
        kind,
        source,
        frontmatter,
        outline,
    })
}

/// Returns the YAML enclosed by `---` at the start of the file.
pub fn split_frontmatter(source: &str) -> Option<&str> {
    let rest = source
        .strip_prefix("---\n")
        .or_else(|| source.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return Some(&rest[..offset]);
        }
        offset += line.len();
    }
    None
}
