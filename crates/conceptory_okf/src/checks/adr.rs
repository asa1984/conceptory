//! Checks that the body of an ADR follows the structure of the template (templates/decision.md).

use std::{collections::BTreeMap, iter};

use crate::{
    bundle::{Bundle, MarkdownFile},
    diagnostic::Diagnostic,
};

const CONTEXT: &str = "Context and Problem Statement";
const DRIVERS: &str = "Decision Drivers";
const OPTIONS: &str = "Considered Options";
const OUTCOME: &str = "Decision Outcome";
const PROS_AND_CONS: &str = "Pros and Cons of the Options";
const MORE_INFORMATION: &str = "More Information";

/// Sections allowed as H2, in the required order
const SECTIONS: &[&str] = &[
    CONTEXT,
    DRIVERS,
    OPTIONS,
    OUTCOME,
    PROS_AND_CONS,
    MORE_INFORMATION,
];
const REQUIRED_SECTIONS: &[&str] = &[CONTEXT, OPTIONS, OUTCOME];
/// Sections allowed as H3 under Decision Outcome, in the required order
const OUTCOME_SECTIONS: &[&str] = &["Consequences", "Confirmation"];
const BULLET_PREFIXES: &[&str] = &["Good: ", "Neutral: ", "Bad: "];
const CHOSEN_PREFIX: &str = "採用した選択肢:";

pub fn check(bundle: &Bundle, out: &mut Vec<Diagnostic>) {
    let mut numbers: BTreeMap<&str, &MarkdownFile> = BTreeMap::new();
    for file in bundle.concepts().filter(|f| f.doc_type() == Some("ADR")) {
        let name = file.file_name();
        if is_adr_file_name(name) {
            if let Some(first) = numbers.insert(&name[..4], file) {
                out.push(Diagnostic::new(
                    &file.display,
                    1,
                    "adr-number",
                    format!("番号 {} が {} と重複しています", &name[..4], first.display),
                ));
            }
        } else {
            out.push(Diagnostic::new(
                &file.display,
                1,
                "adr-filename",
                "ファイル名が NNNN-title-with-dashes.md の形式になっていません",
            ));
        }
        check_title(file, out);
        check_sections(file, out);
        check_options(file, out);
        check_bullets(file, out);
    }
}

fn is_adr_file_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".md") else {
        return false;
    };
    let Some((number, title)) = stem.split_at_checked(4) else {
        return false;
    };
    let Some(title) = title.strip_prefix('-') else {
        return false;
    };
    number.bytes().all(|b| b.is_ascii_digit())
        && !title.is_empty()
        && title.split('-').all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

fn check_title(file: &MarkdownFile, out: &mut Vec<Diagnostic>) {
    let h1s: Vec<_> = file
        .outline
        .headings
        .iter()
        .filter(|h| h.level == 1)
        .collect();
    match h1s.as_slice() {
        [h1] => {
            if file
                .str_field("title")
                .is_some_and(|title| title != h1.text)
            {
                out.push(Diagnostic::new(
                    &file.display,
                    h1.line,
                    "adr-title",
                    "H1 が frontmatter の `title` と一致しません",
                ));
            }
        },
        _ => out.push(Diagnostic::new(
            &file.display,
            1,
            "adr-title",
            format!("H1 は1つだけ書いてください（{} 個あります）", h1s.len()),
        )),
    }
}

fn check_sections(file: &MarkdownFile, out: &mut Vec<Diagnostic>) {
    let mut last_section: Option<usize> = None;
    let mut last_outcome_section: Option<usize> = None;
    let mut current: Option<&str> = None;
    let mut seen = Vec::new();

    for heading in &file.outline.headings {
        match heading.level {
            2 => {
                current = Some(&heading.text);
                let Some(position) = SECTIONS.iter().position(|s| *s == heading.text) else {
                    out.push(Diagnostic::new(
                        &file.display,
                        heading.line,
                        "adr-sections",
                        format!("テンプレートにない節 `{}` があります", heading.text),
                    ));
                    continue;
                };
                if last_section.is_some_and(|last| last >= position) {
                    out.push(Diagnostic::new(
                        &file.display,
                        heading.line,
                        "adr-sections",
                        format!(
                            "節 `{}` の位置がテンプレートの順序と異なります",
                            heading.text
                        ),
                    ));
                }
                last_section = Some(position);
                seen.push(SECTIONS[position]);
            },
            3 if current == Some(OUTCOME) => {
                let Some(position) = OUTCOME_SECTIONS.iter().position(|s| *s == heading.text)
                else {
                    out.push(Diagnostic::new(
                        &file.display,
                        heading.line,
                        "adr-sections",
                        format!(
                            "{OUTCOME} にテンプレートにない節 `{}` があります",
                            heading.text
                        ),
                    ));
                    continue;
                };
                if last_outcome_section.is_some_and(|last| last >= position) {
                    out.push(Diagnostic::new(
                        &file.display,
                        heading.line,
                        "adr-sections",
                        format!(
                            "節 `{}` の位置がテンプレートの順序と異なります",
                            heading.text
                        ),
                    ));
                }
                last_outcome_section = Some(position);
            },
            _ => {},
        }
    }

    for required in REQUIRED_SECTIONS {
        if !seen.contains(required) {
            out.push(Diagnostic::new(
                &file.display,
                1,
                "adr-sections",
                format!("必須の節 `{required}` がありません"),
            ));
        }
    }
}

/// Splits `a. option` into its label and text
fn split_label(text: &str) -> Option<(char, &str)> {
    let mut chars = text.chars();
    let label = chars.next().filter(char::is_ascii_lowercase)?;
    let rest = chars.as_str().strip_prefix(". ")?;
    Some((label, rest))
}

fn check_options(file: &MarkdownFile, out: &mut Vec<Diagnostic>) {
    let mut options: BTreeMap<char, &str> = BTreeMap::new();
    let items = file
        .outline
        .items
        .iter()
        .filter(|item| item.h2.as_deref() == Some(OPTIONS));
    // Labels run from a to z, so options beyond the 26th cannot be labeled
    let labels = ('a'..='z').map(Some).chain(iter::repeat(None));
    for (item, expected) in items.zip(labels) {
        let Some(expected) = expected else {
            out.push(Diagnostic::new(
                &file.display,
                item.line,
                "adr-options",
                "選択肢は 26 個までしか書けません",
            ));
            continue;
        };
        match split_label(&item.text) {
            Some((label, text)) if label == expected => {
                options.insert(label, text);
            },
            _ => out.push(Diagnostic::new(
                &file.display,
                item.line,
                "adr-options",
                format!("選択肢は `{expected}. ` で書き始めてください"),
            )),
        }
    }

    let chosen = file.outline.paragraphs.iter().find(|p| {
        p.h2.as_deref() == Some(OUTCOME) && p.h3.is_none() && p.text.starts_with(CHOSEN_PREFIX)
    });
    match chosen {
        None => {
            if file
                .outline
                .headings
                .iter()
                .any(|h| h.level == 2 && h.text == OUTCOME)
            {
                let line = file.line_containing(&format!("## {OUTCOME}"));
                out.push(Diagnostic::new(
                    &file.display,
                    line,
                    "adr-outcome",
                    format!("{OUTCOME} に「{CHOSEN_PREFIX} **記号**」の行がありません"),
                ));
            }
        },
        Some(paragraph) => {
            let label = paragraph.text[CHOSEN_PREFIX.len()..].trim();
            let known = label.chars().count() == 1
                && label
                    .chars()
                    .next()
                    .is_some_and(|c| options.contains_key(&c));
            if !known {
                out.push(Diagnostic::new(
                    &file.display,
                    paragraph.line,
                    "adr-outcome",
                    format!("採用した選択肢 `{label}` が {OPTIONS} にありません"),
                ));
            }
        },
    }

    let mut current: Option<&str> = None;
    for heading in &file.outline.headings {
        if heading.level == 2 {
            current = Some(&heading.text);
            continue;
        }
        if heading.level != 3 || current != Some(PROS_AND_CONS) {
            continue;
        }
        let matches = split_label(&heading.text)
            .is_some_and(|(label, text)| options.get(&label) == Some(&text));
        if !matches {
            out.push(Diagnostic::new(
                &file.display,
                heading.line,
                "adr-pros-cons",
                format!(
                    "見出し `{}` が {OPTIONS} のどの選択肢とも一致しません",
                    heading.text
                ),
            ));
        }
    }
}

fn check_bullets(file: &MarkdownFile, out: &mut Vec<Diagnostic>) {
    for item in &file.outline.items {
        let in_consequences =
            item.h2.as_deref() == Some(OUTCOME) && item.h3.as_deref() == Some("Consequences");
        let in_pros_and_cons = item.h2.as_deref() == Some(PROS_AND_CONS) && item.h3.is_some();
        if (in_consequences || in_pros_and_cons)
            && !BULLET_PREFIXES
                .iter()
                .any(|prefix| item.text.starts_with(prefix))
        {
            out.push(Diagnostic::new(
                &file.display,
                item.line,
                "adr-bullets",
                "箇条書きは `Good: ` か `Neutral: ` か `Bad: ` で書き始めてください",
            ));
        }
    }
}
