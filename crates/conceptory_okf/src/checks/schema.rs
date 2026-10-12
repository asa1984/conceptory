//! Checks frontmatter against a JSON Schema for each type.

use std::{collections::BTreeMap, fs, path::PathBuf};

use jsonschema::{Draft, Registry, Validator, error::ValidationErrorKind};
use serde_json::Value;

use crate::{
    Error,
    bundle::{Bundle, Frontmatter},
    diagnostic::Diagnostic,
};

/// Name of the shared schema that every type references with `$ref`
const DOCUMENT: &str = "document";

const EMBEDDED: &[(&str, &str)] = &[
    (
        DOCUMENT,
        include_str!("../../../../schemas/document.schema.json"),
    ),
    ("adr", include_str!("../../../../schemas/adr.schema.json")),
    (
        "style-guide",
        include_str!("../../../../schemas/style-guide.schema.json"),
    ),
];

/// Frontmatter schemas for each type.
pub struct Schemas {
    validators: BTreeMap<String, Validator>,
}

impl Schemas {
    /// Loads the built-in schemas, overlaid with the `*.schema.json` files in `dirs`.
    ///
    /// When schemas share a name, the one loaded later wins.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if a directory cannot be read, or [`Error::Schema`] if a schema
    /// cannot be parsed or built.
    pub fn load(dirs: &[PathBuf]) -> Result<Self, Error> {
        let sources = read_sources(dirs)?;
        let document = sources
            .get(DOCUMENT)
            .ok_or_else(|| Error::Schema("document.schema.json がありません".into()))?
            .clone();
        let document_id = document
            .get("$id")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Schema("document.schema.json に `$id` がありません".into()))?
            .to_owned();
        let registry = Registry::new()
            .add(&document_id, document)
            .and_then(jsonschema::RegistryBuilder::prepare)
            .map_err(|e| Error::Schema(format!("document.schema.json を登録できません: {e}")))?;

        let validators = sources
            .iter()
            .filter(|(name, _)| *name != DOCUMENT)
            .map(|(name, schema)| {
                jsonschema::options()
                    .with_draft(Draft::Draft202012)
                    .should_validate_formats(true)
                    .with_registry(&registry)
                    .build(schema)
                    .map(|validator| (name.clone(), validator))
                    .map_err(|e| Error::Schema(format!("スキーマ {name} を構築できません: {e}")))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { validators })
    }

    pub fn check(&self, bundle: &Bundle, out: &mut Vec<Diagnostic>) {
        for file in bundle.concepts() {
            let Frontmatter::Parsed(value) = &file.frontmatter else {
                continue;
            };
            if file.field("status").is_some() {
                out.push(Diagnostic::new(
                    &file.display,
                    file.line_containing("status:"),
                    "no-status",
                    "`status` は使いません。廃止は `deprecated-by` で表してください",
                ));
            }
            let Some(doc_type) = file.doc_type() else {
                continue;
            };
            let name = schema_name(doc_type);
            let Some(validator) = self.validators.get(&name) else {
                out.push(Diagnostic::new(
                    &file.display,
                    1,
                    "unknown-type",
                    format!("`type: {doc_type}` のスキーマ {name}.schema.json がありません"),
                ));
                continue;
            };
            // When the shared schema referenced via allOf is violated, the properties it evaluated
            // are also reported as unevaluated. Skip these secondary errors when there are others
            let errors: Vec<_> = validator.iter_errors(value).collect();
            let is_secondary = |error: &jsonschema::ValidationError<'_>| {
                matches!(
                    error.kind(),
                    ValidationErrorKind::UnevaluatedProperties { .. }
                )
            };
            let has_primary = errors.iter().any(|error| !is_secondary(error));
            for error in &errors {
                // status was already reported more clearly as no-status
                if error.instance_path().to_string() == "/status"
                    || (has_primary && is_secondary(error))
                {
                    continue;
                }
                let location = error.instance_path().to_string();
                let location = if location.is_empty() {
                    "frontmatter".to_string()
                } else {
                    format!("frontmatter{location}")
                };
                out.push(Diagnostic::new(
                    &file.display,
                    1,
                    "schema",
                    format!("{location}: {error}"),
                ));
            }
        }
    }
}

/// Reads the built-in schemas and the `*.schema.json` files in `dirs`, keyed by schema name.
///
/// # Errors
///
/// Returns [`Error::Io`] if a directory cannot be read, or [`Error::Schema`] if a schema cannot be
/// parsed.
fn read_sources(dirs: &[PathBuf]) -> Result<BTreeMap<String, Value>, Error> {
    let mut sources = BTreeMap::new();
    for (name, text) in EMBEDDED {
        let value = serde_json::from_str(text).map_err(|e| {
            Error::Schema(format!("組み込みのスキーマ {name} を解析できません: {e}"))
        })?;
        sources.insert((*name).to_owned(), value);
    }
    for dir in dirs {
        let entries = fs::read_dir(dir).map_err(|e| Error::io(dir, e))?;
        for entry in entries {
            let path = entry.map_err(|e| Error::io(dir, e))?.path();
            let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".schema.json"))
            else {
                continue;
            };
            let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
            let value = serde_json::from_str(&text)
                .map_err(|e| Error::Schema(format!("{} を解析できません: {e}", path.display())))?;
            sources.insert(name.to_owned(), value);
        }
    }
    Ok(sources)
}

/// Derives the schema name from the value of `type`.
///
/// Whitespace and lowercase-to-uppercase transitions separate words. `StyleGuide` and
/// `Style Guide` become `style-guide`, and `ADR` becomes `adr`.
fn schema_name(doc_type: &str) -> String {
    let mut name = String::new();
    let mut previous_lowercase = false;
    for c in doc_type.trim().chars() {
        if c.is_whitespace() {
            if !name.ends_with('-') {
                name.push('-');
            }
            previous_lowercase = false;
            continue;
        }
        if c.is_uppercase() && previous_lowercase {
            name.push('-');
        }
        previous_lowercase = c.is_lowercase();
        name.extend(c.to_lowercase());
    }
    name
}

#[cfg(test)]
mod tests {
    use super::schema_name;

    #[test]
    #[expect(
        clippy::missing_panics_doc,
        reason = "tests report failures by panicking"
    )]
    fn schema_name_splits_words() {
        assert_eq!(schema_name("ADR"), "adr");
        assert_eq!(schema_name("StyleGuide"), "style-guide");
        assert_eq!(schema_name("Style Guide"), "style-guide");
        assert_eq!(
            schema_name("Architecture Decision Record"),
            "architecture-decision-record"
        );
    }
}
