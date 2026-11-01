use std::fmt::Write;

/// A byte range in the schema source
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// An error in a schema, at the span that caused it
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            help: None,
        }
    }

    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Renders errors in a schema one after another, with a blank line
    /// between them, as compilers print them
    pub fn render_all(diagnostics: &[Self], file_name: &str, source: &str) -> String {
        let rendered: Vec<String> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.render(file_name, source))
            .collect();
        rendered.join("\n")
    }

    /// Renders the error with the line it points at, as compilers print them
    pub fn render(&self, file_name: &str, source: &str) -> String {
        let (line, column) = line_and_column(source, self.span.start);
        let text = source.lines().nth(line - 1).unwrap_or("");
        let width = source[self.span.start..self.span.end.max(self.span.start)]
            .chars()
            .count()
            .max(1);
        let gutter = " ".repeat(line.to_string().len());

        let mut rendered = String::new();
        let _ = writeln!(rendered, "error: {}", self.message);
        let _ = writeln!(rendered, "{gutter}--> {file_name}:{line}:{column}");
        let _ = writeln!(rendered, "{gutter} |");
        let _ = writeln!(rendered, "{line} | {text}");
        let _ = write!(
            rendered,
            "{gutter} | {}{}",
            " ".repeat(column - 1),
            "^".repeat(width)
        );
        if let Some(help) = &self.help {
            let _ = write!(rendered, " {help}");
        }
        rendered.push('\n');

        rendered
    }
}

/// The 1-based line and column, in characters, of a byte offset
fn line_and_column(source: &str, offset: usize) -> (usize, usize) {
    let before = &source[..offset.min(source.len())];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let column = before[line_start..].chars().count() + 1;

    (line, column)
}
