//! Diagnostics reported by the features of the conceptory CLI.

use std::fmt;

use serde::Serialize;

/// A single problem found by a check.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Diagnostic {
    pub path: String,
    pub line: usize,
    pub rule: &'static str,
    pub message: String,
}

impl Diagnostic {
    pub fn new(
        path: impl Into<String>,
        line: usize,
        rule: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            line,
            rule,
            message: message.into(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {} [{}]",
            self.path, self.line, self.message, self.rule
        )
    }
}
