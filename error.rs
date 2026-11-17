//! Errors with source positions, rendered with the offending line and a caret.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Syntax,
    Runtime,
}

#[derive(Debug, Clone)]
pub struct Error {
    pub kind: Kind,
    pub message: String,
    pub pos: Pos,
    /// Function names from innermost to outermost, for runtime errors.
    pub trace: Vec<(String, Pos)>,
}

impl Error {
    pub fn syntax(message: impl Into<String>, pos: Pos) -> Self {
        Error {
            kind: Kind::Syntax,
            message: message.into(),
            pos,
            trace: Vec::new(),
        }
    }

    pub fn runtime(message: impl Into<String>, pos: Pos) -> Self {
        Error {
            kind: Kind::Runtime,
            message: message.into(),
            pos,
            trace: Vec::new(),
        }
    }

    /// Pretty report pointing into the source, e.g.
    ///
    /// ```text
    /// runtime error: division by zero
    ///   --> line 2, column 12
    ///    |
    ///  2 |   return a / b;
    ///    |            ^
    ///   in divide (called at line 5)
    /// ```
    pub fn render(&self, source: &str) -> String {
        let label = match self.kind {
            Kind::Syntax => "syntax error",
            Kind::Runtime => "runtime error",
        };
        let mut out = format!(
            "{label}: {}\n  --> line {}, column {}\n",
            self.message, self.pos.line, self.pos.col
        );
        if let Some(text) = source.lines().nth(self.pos.line.saturating_sub(1)) {
            let gutter = self.pos.line.to_string().len();
            let pad = " ".repeat(gutter);
            out += &format!(
                " {pad} |\n {} | {text}\n {pad} | {}^\n",
                self.pos.line,
                " ".repeat(self.pos.col.saturating_sub(1))
            );
        }
        for (name, at) in &self.trace {
            out += &format!("  in {name} (called at line {})\n", at.line);
        }
        out
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (line {}, column {})",
            self.message, self.pos.line, self.pos.col
        )
    }
}

impl std::error::Error for Error {}
