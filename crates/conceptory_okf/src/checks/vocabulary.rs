//! Checks `vocabulary` in frontmatter and generates the ai-words-ja dictionary.
//!
//! See ADR 0004 (Define writing norms as style guides referenced from skills) and ADR 0005 (Check
//! Japanese prose in two stages: static checks and LLM review).

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::{
    bundle::{Bundle, MarkdownFile},
    checks::terms,
    diagnostic::Diagnostic,
};

pub struct Word<'a> {
    pub avoid: &'a str,
    pub prefer: Vec<&'a str>,
    /// Token sequence to match, keyed by kuromoji token field names
    pub tokens: Vec<Map<String, Value>>,
    pub file: &'a MarkdownFile,
}

/// Mapping from frontmatter token field names to kuromoji token field names
const TOKEN_FIELDS: [(&str, &str); 4] = [
    ("surface", "surface_form"),
    ("base", "basic_form"),
    ("pos", "pos"),
    ("pos-detail", "pos_detail_1"),
];

/// Converts a frontmatter token into an ai-words-ja dictionary token
fn token(entry: &Value) -> Map<String, Value> {
    TOKEN_FIELDS
        .iter()
        .filter_map(|(from, to)| Some(((*to).to_owned(), entry.get(*from)?.clone())))
        .collect()
}

/// Returns the `tokens` sequence if present, otherwise a single token with `avoid` as its base
/// form
fn tokens(entry: &Value, avoid: &str) -> Vec<Map<String, Value>> {
    if let Some(Value::Array(tokens)) = entry.get("tokens") {
        return tokens.iter().map(token).collect();
    }
    let mut single = token(entry);
    single.insert("basic_form".to_owned(), avoid.into());
    vec![single]
}

/// Collects avoided words from the non-deprecated documents of every OKF bundle
pub fn collect(bundles: &[Bundle]) -> Vec<Word<'_>> {
    let mut words = Vec::new();
    for file in bundles
        .iter()
        .flat_map(Bundle::concepts)
        .filter(|f| !f.is_deprecated())
    {
        let Some(Value::Array(entries)) = file.field("vocabulary") else {
            continue;
        };
        for entry in entries {
            let Some(avoid) = entry.get("avoid").and_then(Value::as_str) else {
                continue;
            };
            let prefer = entry
                .get("prefer")
                .and_then(Value::as_array)
                .map_or(Vec::new(), |values| {
                    values.iter().filter_map(Value::as_str).collect()
                });
            words.push(Word {
                avoid,
                prefer,
                tokens: tokens(entry, avoid),
                file,
            });
        }
    }
    words
}

pub fn check(bundles: &[Bundle], out: &mut Vec<Diagnostic>) {
    let words = collect(bundles);
    let terms = terms::collect(bundles);

    let mut seen: BTreeMap<&str, &MarkdownFile> = BTreeMap::new();
    for word in &words {
        let line = word.file.line_containing(&format!("avoid: {}", word.avoid));
        if let Some(first) = seen.insert(word.avoid, word.file) {
            out.push(Diagnostic::new(
                &word.file.display,
                line,
                "vocabulary-duplicate",
                format!(
                    "避ける語 `{}` は {} でも定義されています",
                    word.avoid, first.display
                ),
            ));
        }
        if let Some(term) = terms.iter().find(|t| t.name == word.avoid) {
            out.push(Diagnostic::new(
                &word.file.display,
                line,
                "vocabulary-conflict",
                format!(
                    "避ける語 `{}` が {} で用語として定義されています",
                    word.avoid, term.file.display
                ),
            ));
        }
        for prefer in &word.prefer {
            if words.iter().any(|other| other.avoid == *prefer) {
                out.push(Diagnostic::new(
                    &word.file.display,
                    line,
                    "vocabulary-conflict",
                    format!("代わりに使う語 `{prefer}` が避ける語にも入っています"),
                ));
            }
        }
    }
}

/// Writes, as JSON, the dictionary passed to `dictionaryPath` of textlint-rule-preset-ai-words-ja
pub fn dictionary(bundles: &[Bundle]) -> String {
    let entries: Vec<Value> = collect(bundles)
        .iter()
        .map(|word| {
            let prefer = word
                .prefer
                .iter()
                .map(|p| format!("「{p}」"))
                .collect::<Vec<_>>()
                .join("か");
            let message = format!(
                "「{}」は使いません。{prefer}を使ってください（{}）",
                word.avoid, word.file.display
            );
            json!({ "message": message, "tokens": word.tokens })
        })
        .collect();
    // `{:#}` on a `Value` prints pretty-printed JSON
    format!("{:#}\n", json!({ "entries": entries }))
}
