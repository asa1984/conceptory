//! Checks `terms` in frontmatter and generates the prh dictionary (ADR 0003, Define terms in the
//! frontmatter of the introducing document and check their spelling with textlint).

use std::collections::BTreeMap;

use serde_json::Value;

use crate::{
    bundle::{Bundle, MarkdownFile},
    diagnostic::Diagnostic,
};

/// A single term listed in `terms` in a document's frontmatter.
pub struct Term<'a> {
    /// Preferred spelling
    pub name: &'a str,
    pub avoid: Vec<&'a str>,
    pub file: &'a MarkdownFile,
}

/// Collects terms from the non-deprecated documents of every OKF bundle
pub fn collect(bundles: &[Bundle]) -> Vec<Term<'_>> {
    let mut terms = Vec::new();
    for file in bundles
        .iter()
        .flat_map(Bundle::concepts)
        .filter(|f| !f.is_deprecated())
    {
        let Some(Value::Array(entries)) = file.field("terms") else {
            continue;
        };
        for entry in entries {
            let Some(term) = entry.get("term").and_then(Value::as_str) else {
                continue;
            };
            let avoid = entry
                .get("avoid")
                .and_then(Value::as_array)
                .map_or(Vec::new(), |values| {
                    values.iter().filter_map(Value::as_str).collect()
                });
            terms.push(Term {
                name: term,
                avoid,
                file,
            });
        }
    }
    terms
}

pub fn check(bundles: &[Bundle], out: &mut Vec<Diagnostic>) {
    let terms = collect(bundles);

    let mut defined: BTreeMap<&str, &MarkdownFile> = BTreeMap::new();
    for term in &terms {
        let line = term.file.line_containing(&format!("term: {}", term.name));
        if let Some(first) = defined.insert(term.name, term.file) {
            out.push(Diagnostic::new(
                &term.file.display,
                line,
                "term-duplicate",
                format!(
                    "用語 `{}` は {} でも定義されています",
                    term.name, first.display
                ),
            ));
        }
        if !term.file.outline.strongs.iter().any(|s| s == term.name) {
            out.push(Diagnostic::new(
                &term.file.display,
                line,
                "term-undefined",
                format!("用語 `{}` を本文で太字にして定義してください", term.name),
            ));
        }
    }

    for term in &terms {
        for other in &terms {
            if other.avoid.contains(&term.name) {
                out.push(Diagnostic::new(
                    &other.file.display,
                    other.file.line_containing(&format!("- {}", term.name)),
                    "term-conflict",
                    format!(
                        "避けるべき表記 `{}` が {} で用語として定義されています",
                        term.name, term.file.display
                    ),
                ));
            }
        }
    }
}

/// Writes the prh dictionary as YAML. Terms without `avoid` are left out
pub fn prh(bundles: &[Bundle]) -> String {
    // YAML is a superset of JSON, so writing JSON string literals gives correct escaping
    let quote = |s: &str| Value::from(s).to_string();
    let mut lines = vec!["version: 1".to_owned()];
    let terms = collect(bundles);
    let rules: Vec<_> = terms.iter().filter(|term| !term.avoid.is_empty()).collect();
    if rules.is_empty() {
        lines.push("rules: []".to_owned());
    } else {
        lines.push("rules:".to_owned());
    }
    for term in rules {
        lines.push(format!("  - expected: {}", quote(term.name)));
        lines.push("    patterns:".to_owned());
        lines.extend(
            term.avoid
                .iter()
                .map(|avoid| format!("      - {}", quote(avoid))),
        );
        let message = format!(
            "「{}」と書いてください（{} で定義した用語です）",
            term.name, term.file.display
        );
        lines.push(format!("    prh: {}", quote(&message)));
    }
    lines.push(String::new());
    lines.join("\n")
}

/// List of terms, each with the document that defines it
pub fn list(bundles: &[Bundle]) -> String {
    let mut lines: Vec<String> = collect(bundles)
        .iter()
        .map(|term| [term.name, term.file.display.as_str()].join("\t"))
        .collect();
    lines.push(String::new());
    lines.join("\n")
}
