//! Tests the OKF bundle checks and dictionary generation using conforming and violating fixtures.
#![allow(clippy::expect_used, clippy::missing_panics_doc)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use conceptory_okf::{Diagnostic, Loaded, Schemas};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn load(names: &[&str]) -> Loaded {
    let roots: Vec<_> = names.iter().map(|name| fixture(name)).collect();
    conceptory_okf::load(&roots).expect("can load the fixture")
}

fn check(names: &[&str]) -> Vec<Diagnostic> {
    conceptory_okf::check(
        &load(names),
        &Schemas::load(&[]).expect("can load the built-in schemas"),
    )
}

fn render(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn valid_bundle_passes() {
    let diagnostics = check(&["valid"]);
    assert!(diagnostics.is_empty(), "{}", render(&diagnostics));
}

#[test]
fn invalid_bundle_reports_every_rule() {
    let diagnostics = check(&["invalid"]);
    let rules = [
        "okf-frontmatter",
        "okf-type",
        "okf-version",
        "okf-log",
        "unknown-type",
        "schema",
        "no-status",
        "adr-filename",
        "adr-number",
        "adr-title",
        "adr-sections",
        "adr-options",
        "adr-outcome",
        "adr-pros-cons",
        "adr-bullets",
        "link-bundle-relative",
        "link-broken",
        "frontmatter-path",
        "deprecated-by",
        "index-missing",
        "index-unlisted",
        "term-duplicate",
        "term-undefined",
        "term-conflict",
        "vocabulary-duplicate",
        "vocabulary-conflict",
    ];
    for rule in rules {
        assert!(
            diagnostics.iter().any(|d| d.rule == rule),
            "{rule} was not reported:\n{}",
            render(&diagnostics)
        );
    }
}

#[test]
fn duplicate_terms_are_detected_across_bundles() {
    // Each OKF bundle satisfies the rules on its own
    assert_eq!(render(&check(&["duplicate"])), "");
    let diagnostics = check(&["valid", "duplicate"]);
    assert!(
        diagnostics.iter().any(|d| d.rule == "term-duplicate"),
        "{}",
        render(&diagnostics)
    );
}

#[test]
fn prh_dictionary_contains_only_active_terms_with_avoid() {
    let dictionary = conceptory_okf::prh_dictionary(&load(&["valid"]).bundles);
    assert!(dictionary.contains(r#"expected: "widget""#), "{dictionary}");
    assert!(dictionary.contains(r#"- "ウィジェット""#), "{dictionary}");
    // Terms from deprecated documents are left out
    assert!(!dictionary.contains("gadget"), "{dictionary}");
}

#[test]
fn vocabulary_dictionary_matches_ai_words_ja_format() {
    let text = conceptory_okf::vocabulary_dictionary(&load(&["valid"]).bundles);
    let dictionary: serde_json::Value =
        serde_json::from_str(&text).expect("dictionary is valid JSON");
    let entries = dictionary["entries"]
        .as_array()
        .expect("entries is an array");
    assert_eq!(entries.len(), 3);
    assert_eq!(
        entries[0]["tokens"],
        serde_json::json!([{ "basic_form": "版", "pos": "名詞", "pos_detail_1": "一般" }])
    );
    assert_eq!(
        entries[1]["tokens"],
        serde_json::json!([{ "basic_form": "足す" }])
    );
    // Expressions spanning several words are matched by a token sequence
    assert_eq!(
        entries[2]["tokens"],
        serde_json::json!([
            { "surface_form": "手", "pos": "名詞" },
            { "surface_form": "で", "pos": "助詞", "pos_detail_1": "格助詞" },
        ])
    );
    let message = entries[1]["message"].as_str().expect("message is a string");
    assert!(message.contains("「追加する」"), "{message}");
    assert!(message.contains("guides/writing.md"), "{message}");
}

#[test]
fn index_generation_reproduces_valid_indexes() {
    let indexes = conceptory_okf::indexes(&load(&["valid"]).bundles);
    assert_eq!(indexes.len(), 3);
    for (path, content) in indexes {
        assert_eq!(
            fs::read_to_string(&path).expect("can read the existing index.md"),
            content,
            "{} is not reproduced",
            path.display()
        );
    }
}
