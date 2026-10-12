//! Builds the display data for the viewer.
//!
//! Documents are identified by the same display path as in diagnostics: the path given as the
//! OKF bundle root joined with the path relative to that root.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    hash::{DefaultHasher, Hash, Hasher},
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use serde::Serialize;
use serde_json::Value;

use crate::{
    Loaded,
    bundle::{Bundle, Frontmatter, Kind, MarkdownFile},
    checks::terms,
    diagnostic::Diagnostic,
    markdown,
};

/// Summary of a document, used in the document list and the graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub doc_type: Option<String>,
    /// `concept`, `index` or `log`
    pub kind: &'static str,
    pub description: Option<String>,
    pub tags: Vec<String>,
    /// Identifier of the document that deprecated this one
    pub deprecated_by: Option<String>,
    /// Hash for detecting whether the displayed content changed
    pub hash: String,
}

/// Data for displaying a document.
#[derive(Debug, Clone, Serialize)]
pub struct DocumentDetail {
    #[serde(flatten)]
    pub summary: DocumentSummary,
    pub frontmatter: Value,
    /// Sanitized HTML of the body
    pub html: String,
    /// Identifiers of the documents this document links to
    pub links: Vec<String>,
    /// Identifiers of the documents that link to this document
    pub backlinks: Vec<String>,
    /// Terms defined by this document
    pub terms: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    Link,
    DeprecatedBy,
}

/// A relationship between documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
}

/// A glossary entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TermView {
    pub name: String,
    pub avoid: Vec<String>,
    /// Identifier of the document that defines the term
    pub document: String,
}

/// Data for the viewer's overall display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Overview {
    pub documents: Vec<DocumentSummary>,
    pub edges: Vec<Edge>,
    pub terms: Vec<TermView>,
    pub diagnostics: Vec<Diagnostic>,
}

/// All the data passed to the viewer.
#[derive(Debug, Clone)]
pub struct View {
    pub overview: Overview,
    pub details: BTreeMap<String, DocumentDetail>,
}

/// Builds the viewer data from the loaded OKF bundles. `diagnostics` is the result of the checks.
#[must_use]
pub fn build(loaded: &Loaded, diagnostics: Vec<Diagnostic>) -> View {
    let index = Index::new(&loaded.bundles);
    let mut drafts: BTreeMap<String, Draft> = BTreeMap::new();
    for (bundle, root) in loaded.bundles.iter().zip(&index.roots) {
        for file in &bundle.files {
            drafts.insert(file.display.clone(), Draft::new(&index, root, file));
        }
    }

    let mut backlinks: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (id, draft) in &drafts {
        if draft.kind == "concept" {
            for target in &draft.links {
                backlinks.entry(target.as_str()).or_default().insert(id);
            }
        }
    }

    let defined_terms = terms::collect(&loaded.bundles);
    let mut details = BTreeMap::new();
    for (id, draft) in &drafts {
        let backlinks: Vec<String> = backlinks.get(id.as_str()).map_or_else(Vec::new, |set| {
            set.iter().map(ToString::to_string).collect()
        });
        let terms: Vec<String> = defined_terms
            .iter()
            .filter(|term| &term.file.display == id)
            .map(|term| term.name.to_owned())
            .collect();
        details.insert(id.clone(), draft.finish(backlinks, terms));
    }

    let overview = Overview {
        documents: details
            .values()
            .map(|detail| detail.summary.clone())
            .collect(),
        edges: edges(&details),
        terms: defined_terms
            .iter()
            .map(|term| TermView {
                name: term.name.to_owned(),
                avoid: term.avoid.iter().map(ToString::to_string).collect(),
                document: term.file.display.clone(),
            })
            .collect(),
        diagnostics,
    };
    View { overview, details }
}

/// Turns links between concept documents and deprecation relationships into edges.
fn edges(details: &BTreeMap<String, DocumentDetail>) -> Vec<Edge> {
    let is_concept = |id: &str| {
        details
            .get(id)
            .is_some_and(|detail| detail.summary.kind == "concept")
    };
    let mut edges = Vec::new();
    for (id, detail) in details {
        if detail.summary.kind != "concept" {
            continue;
        }
        for target in detail.links.iter().filter(|target| is_concept(target)) {
            edges.push(Edge {
                source: id.clone(),
                target: target.clone(),
                kind: EdgeKind::Link,
            });
        }
        if let Some(target) = &detail.summary.deprecated_by {
            edges.push(Edge {
                source: id.clone(),
                target: target.clone(),
                kind: EdgeKind::DeprecatedBy,
            });
        }
    }
    edges
}

/// Lookup from a document's absolute path to its identifier, plus the absolute paths of the OKF
/// bundle roots.
struct Index {
    roots: Vec<Root>,
    ids: Arc<HashMap<PathBuf, String>>,
}

/// An OKF bundle root. `display` is the path the user passed, and `absolute` is its absolute path.
struct Root {
    display: PathBuf,
    absolute: PathBuf,
}

impl Index {
    fn new(bundles: &[Bundle]) -> Self {
        let roots: Vec<Root> = bundles
            .iter()
            .map(|bundle| Root {
                display: bundle.root.clone(),
                absolute: absolute(&bundle.root),
            })
            .collect();
        let mut ids = HashMap::new();
        for (bundle, root) in bundles.iter().zip(&roots) {
            for file in &bundle.files {
                ids.insert(root.absolute.join(&file.rel), file.display.clone());
            }
        }
        Self {
            roots,
            ids: Arc::new(ids),
        }
    }
}

/// Display data for a document before its backlinks and terms are known.
struct Draft {
    summary: DocumentSummary,
    kind: &'static str,
    frontmatter: Value,
    html: String,
    links: BTreeSet<String>,
}

impl Draft {
    fn new(index: &Index, root: &Root, file: &MarkdownFile) -> Self {
        let dir = root.absolute.join(file.dir());
        let resolver = Resolver {
            dir,
            root: root.absolute.clone(),
            ids: Arc::clone(&index.ids),
            roots: index
                .roots
                .iter()
                .map(|r| (r.absolute.clone(), r.display.clone()))
                .collect(),
        };
        let links = file
            .outline
            .links
            .iter()
            .filter_map(|link| resolver.document(&link.dest))
            .collect();
        let deprecated_by = file
            .str_field("deprecated-by")
            .and_then(|path| resolver.document(path));
        let kind = match file.kind {
            Kind::Concept => "concept",
            Kind::Index => "index",
            Kind::Log => "log",
        };
        let summary = DocumentSummary {
            id: file.display.clone(),
            title: title_of(file),
            doc_type: file.doc_type().map(ToString::to_string),
            kind,
            description: file.str_field("description").map(ToString::to_string),
            tags: file
                .field("tags")
                .and_then(Value::as_array)
                .map_or_else(Vec::new, |tags| {
                    tags.iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect()
                }),
            deprecated_by,
            hash: String::new(),
        };
        let frontmatter = match &file.frontmatter {
            Frontmatter::Parsed(value) => value.clone(),
            Frontmatter::Missing | Frontmatter::Invalid => Value::Null,
        };
        Self {
            summary,
            kind,
            frontmatter,
            html: render(&file.source, resolver),
            links,
        }
    }

    fn finish(&self, backlinks: Vec<String>, terms: Vec<String>) -> DocumentDetail {
        let mut detail = DocumentDetail {
            summary: self.summary.clone(),
            frontmatter: self.frontmatter.clone(),
            html: self.html.clone(),
            links: self.links.iter().cloned().collect(),
            backlinks,
            terms,
        };
        let mut hasher = DefaultHasher::new();
        detail.summary.title.hash(&mut hasher);
        detail.summary.description.hash(&mut hasher);
        detail.summary.tags.hash(&mut hasher);
        detail.summary.deprecated_by.hash(&mut hasher);
        detail.frontmatter.to_string().hash(&mut hasher);
        detail.html.hash(&mut hasher);
        detail.links.hash(&mut hasher);
        detail.backlinks.hash(&mut hasher);
        detail.terms.hash(&mut hasher);
        detail.summary.hash = format!("{:016x}", hasher.finish());
        detail
    }
}

/// Uses `title`, falling back to the first H1 and then to the file name.
fn title_of(file: &MarkdownFile) -> String {
    file.str_field("title")
        .map(ToString::to_string)
        .or_else(|| {
            file.outline
                .headings
                .iter()
                .find(|heading| heading.level == 1)
                .map(|heading| heading.text.clone())
        })
        .unwrap_or_else(|| file.file_name().to_owned())
}

/// Resolves links in a document to identifiers of loaded documents or to viewer URLs.
#[derive(Clone)]
struct Resolver {
    /// Absolute path of the directory of the document containing the link
    dir: PathBuf,
    /// Absolute path of the OKF bundle root of the document containing the link
    root: PathBuf,
    ids: Arc<HashMap<PathBuf, String>>,
    /// Absolute and display paths of every OKF bundle root
    roots: Vec<(PathBuf, PathBuf)>,
}

impl Resolver {
    /// Returns the absolute path of the linked file and the part from `#` on. `None` for external
    /// URLs
    fn target<'u>(&self, url: &'u str) -> Option<(PathBuf, &'u str)> {
        if url.is_empty() || url.starts_with('#') || has_scheme(url) {
            return None;
        }
        let (path, fragment) = url
            .find(['#', '?'])
            .map_or((url, ""), |i| (&url[..i], &url[i..]));
        let target = path.strip_prefix('/').map_or_else(
            || self.dir.join(path),
            |bundle_relative| self.root.join(bundle_relative),
        );
        Some((normalize(&target), fragment))
    }

    /// Returns the identifier of the linked document if it is a loaded one
    fn document(&self, url: &str) -> Option<String> {
        let (path, _) = self.target(url)?;
        self.ids.get(&path).cloned()
    }

    /// Rewrites a link to a viewer URL: `/?doc=` for documents and `/files?path=` for other files.
    ///
    /// Identifiers may start with `./docs` or `../docs`, and if they were put in the URL path the
    /// browser would resolve `.` and `..` into a different path. Queries are sent as-is, so the
    /// identifier goes in the query.
    fn rewrite(&self, url: &str) -> String {
        let Some((path, suffix)) = self.target(url) else {
            return url.to_owned();
        };
        // The original link's query means nothing for a local file, so drop it
        let fragment = suffix.find('#').map_or("", |i| &suffix[i..]);
        if let Some(id) = self.ids.get(&path) {
            return format!("/?doc={}{fragment}", encode_query(id));
        }
        self.roots
            .iter()
            .find_map(|(absolute, display)| {
                let rel = path.strip_prefix(absolute).ok()?;
                Some(format!(
                    "/files?path={}",
                    encode_query(&display.join(rel).to_string_lossy())
                ))
            })
            .unwrap_or_else(|| url.to_owned())
    }
}

/// Converts Markdown to GitHub-compatible HTML and sanitizes it.
fn render(source: &str, resolver: Resolver) -> String {
    let mut options = markdown::options();
    let link_resolver = resolver.clone();
    options.extension.link_url_rewriter =
        Some(Arc::new(move |url: &str| link_resolver.rewrite(url)));
    options.extension.image_url_rewriter = Some(Arc::new(move |url: &str| resolver.rewrite(url)));
    let html = comrak::markdown_to_html(source, &options);
    sanitizer().clean(&html).to_string()
}

/// Sanitizer that allows the tags and attributes GitHub's Markdown output needs.
fn sanitizer() -> ammonia::Builder<'static> {
    let mut builder = ammonia::Builder::default();
    builder
        .add_tags([
            "input", "section", "details", "summary", "kbd", "picture", "source",
        ])
        .add_generic_attributes([
            "id",
            "class",
            "aria-label",
            "aria-hidden",
            "data-heading-content",
            "data-math-style",
            "data-footnotes",
            "data-footnote-ref",
            "data-footnote-backref",
            "data-footnote-backref-idx",
        ])
        .add_tag_attributes("pre", ["lang"])
        .add_tag_attributes("th", ["align"])
        .add_tag_attributes("td", ["align"])
        .add_tag_attributes("input", ["type", "checked", "disabled"])
        .add_tag_attributes("source", ["srcset", "media", "type"])
        .add_tag_attributes("details", ["open"])
        .id_prefix(Some("user-content-"));
    builder
}

/// Whether the URL starts with a scheme such as `https:` or `mailto:`
fn has_scheme(url: &str) -> bool {
    url.find(':').is_some_and(|colon| {
        let scheme = &url[..colon];
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
    })
}

/// Returns the path with `.` and `..` removed, without touching the file system
fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {},
            Component::ParentDir => {
                normalized.pop();
            },
            other => normalized.push(other),
        }
    }
    normalized
}

/// Makes the path absolute, resolving symbolic links when possible
fn absolute(path: &Path) -> PathBuf {
    path.canonicalize()
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Percent-encodes characters not allowed in a query value. `/` is kept for readability
fn encode_query(path: &str) -> String {
    let mut encoded = String::new();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            encoded.push(char::from(byte));
        } else {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0F)]));
        }
    }
    encoded
}
