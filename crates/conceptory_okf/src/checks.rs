mod adr;
pub mod index;
mod links;
mod okf;
pub mod schema;
pub mod terms;
pub mod vocabulary;

use crate::{bundle::Bundle, diagnostic::Diagnostic};

/// Runs the per-bundle checks, plus the checks that span OKF bundles, such as terms and avoided
/// words
pub fn run(bundles: &[Bundle], schemas: &schema::Schemas) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for bundle in bundles {
        okf::check(bundle, &mut diagnostics);
        schemas.check(bundle, &mut diagnostics);
        adr::check(bundle, &mut diagnostics);
        links::check(bundle, &mut diagnostics);
        index::check(bundle, &mut diagnostics);
    }
    terms::check(bundles, &mut diagnostics);
    vocabulary::check(bundles, &mut diagnostics);
    diagnostics
}
