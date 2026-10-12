//! Checks conformance to OKF v0.2 (SPEC.md §8, §9, §11).

use serde_json::Value;

use crate::{
    bundle::{Bundle, Frontmatter, Kind},
    diagnostic::Diagnostic,
};

pub fn check(bundle: &Bundle, out: &mut Vec<Diagnostic>) {
    for file in &bundle.files {
        match file.kind {
            Kind::Concept => match &file.frontmatter {
                Frontmatter::Missing => out.push(Diagnostic::new(
                    &file.display,
                    1,
                    "okf-frontmatter",
                    "frontmatter がありません",
                )),
                Frontmatter::Invalid => {},
                Frontmatter::Parsed(_) => {
                    if file.doc_type().is_none_or(|t| t.trim().is_empty()) {
                        out.push(Diagnostic::new(
                            &file.display,
                            1,
                            "okf-type",
                            "`type` には空でない文字列を書いてください",
                        ));
                    }
                },
            },
            Kind::Index if file.rel == "index.md" => match &file.frontmatter {
                Frontmatter::Parsed(Value::Object(map)) => {
                    if !map.get("okf_version").is_some_and(Value::is_string) {
                        out.push(Diagnostic::new(
                            &file.display,
                            1,
                            "okf-version",
                            "`okf_version` は文字列で宣言してください",
                        ));
                    }
                    for key in map.keys().filter(|key| *key != "okf_version") {
                        out.push(Diagnostic::new(
                            &file.display,
                            1,
                            "okf-index",
                            format!(
                                "OKF bundle のルートの index.md の frontmatter に `{key}` \
                                 は書けない"
                            ),
                        ));
                    }
                },
                Frontmatter::Invalid => {},
                _ => out.push(Diagnostic::new(
                    &file.display,
                    1,
                    "okf-version",
                    "OKF bundle のルートの index.md では `okf_version` を宣言してください",
                )),
            },
            Kind::Index => {
                if !matches!(file.frontmatter, Frontmatter::Missing) {
                    out.push(Diagnostic::new(
                        &file.display,
                        1,
                        "okf-index",
                        "OKF bundle のルート以外の index.md には frontmatter を書けません",
                    ));
                }
            },
            Kind::Log => {
                if !matches!(file.frontmatter, Frontmatter::Missing) {
                    out.push(Diagnostic::new(
                        &file.display,
                        1,
                        "okf-log",
                        "log.md には frontmatter を書けません",
                    ));
                }
                let mut previous: Option<&str> = None;
                for heading in file.outline.headings.iter().filter(|h| h.level == 2) {
                    if !is_iso_date(&heading.text) {
                        out.push(Diagnostic::new(
                            &file.display,
                            heading.line,
                            "okf-log",
                            format!(
                                "log.md の見出し `{}` が YYYY-MM-DD の形式になっていません",
                                heading.text
                            ),
                        ));
                        continue;
                    }
                    if previous.is_some_and(|p| p <= heading.text.as_str()) {
                        out.push(Diagnostic::new(
                            &file.display,
                            heading.line,
                            "okf-log",
                            "log.md の日付が新しい順に並んでいません",
                        ));
                    }
                    previous = Some(&heading.text);
                }
            },
        }
    }
}

fn is_iso_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        })
}
