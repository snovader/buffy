//! Parses tokens into declarations, without resolving their types:
//!
//! ```text
//! schema      = declaration*
//! declaration = doc* ("message" | "table" | "struct") NAME attributes? "{" field* "}"
//! attributes  = "(" (attribute ("," attribute)* ","?)? ")"
//! attribute   = NAME (":" INTEGER)?
//! field       = doc* NAME ":" type ";"
//! type        = NAME | "[" NAME "]"
//! ```

use crate::diagnostic::{Diagnostic, Span};
use crate::lexer::{Token, TokenKind, tokenize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyword {
    Message,
    Table,
    Struct,
}

#[derive(Clone, Debug)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

#[derive(Debug)]
pub struct Declaration {
    pub keyword: Keyword,
    pub name: Name,
    pub docs: Vec<String>,
    pub attributes: Vec<Attribute>,
    pub fields: Vec<FieldDeclaration>,
}

#[derive(Debug)]
pub struct Attribute {
    pub name: Name,
    pub value: Option<(u64, Span)>,
}

#[derive(Debug)]
pub struct FieldDeclaration {
    pub name: Name,
    pub docs: Vec<String>,
    pub type_name: Name,
    pub is_array: bool,
}

pub fn parse(source: &str) -> Result<Vec<Declaration>, Diagnostic> {
    let tokens = tokenize(source)?;
    let mut parser = Parser {
        source,
        tokens: &tokens,
        index: 0,
    };

    let mut declarations = Vec::new();
    while parser.peek().kind != TokenKind::End {
        declarations.push(parser.declaration()?);
    }

    Ok(declarations)
}

struct Parser<'a> {
    source: &'a str,
    tokens: &'a [Token],
    index: usize,
}

impl Parser<'_> {
    fn declaration(&mut self) -> Result<Declaration, Diagnostic> {
        let docs = self.docs()?;

        let keyword_token = self.expect(TokenKind::Identifier, "`message`, `table` or `struct`")?;
        let keyword = match self.text(keyword_token) {
            "message" => Keyword::Message,
            "table" => Keyword::Table,
            "struct" => Keyword::Struct,
            other => {
                return Err(Diagnostic::new(
                    format!("expected `message`, `table` or `struct`, found `{other}`"),
                    keyword_token.span,
                ));
            }
        };

        let name = self.name("a name")?;
        let attributes = if self.peek().kind == TokenKind::LeftParenthesis {
            self.attributes()?
        } else {
            Vec::new()
        };

        self.expect(TokenKind::LeftBrace, "`{`")?;
        let mut fields = Vec::new();
        while self.peek().kind != TokenKind::RightBrace {
            fields.push(self.field()?);
        }
        self.expect(TokenKind::RightBrace, "`}`")?;

        Ok(Declaration {
            keyword,
            name,
            docs,
            attributes,
            fields,
        })
    }

    fn attributes(&mut self) -> Result<Vec<Attribute>, Diagnostic> {
        self.expect(TokenKind::LeftParenthesis, "`(`")?;

        let mut attributes = Vec::new();
        while self.peek().kind != TokenKind::RightParenthesis {
            let name = self.name("an attribute")?;
            let value = if self.peek().kind == TokenKind::Colon {
                self.advance();
                let token = self.expect(TokenKind::Integer, "an integer")?;
                let value = self
                    .text(token)
                    .parse::<u64>()
                    .map_err(|_| Diagnostic::new("the integer is too large", token.span))?;
                Some((value, token.span))
            } else {
                None
            };
            attributes.push(Attribute { name, value });

            if self.peek().kind != TokenKind::Comma {
                break;
            }
            self.advance();
        }

        self.expect(TokenKind::RightParenthesis, "`,` or `)`")?;
        Ok(attributes)
    }

    fn field(&mut self) -> Result<FieldDeclaration, Diagnostic> {
        let docs = self.docs()?;
        let name = self.name("a field name or `}`")?;
        self.expect(TokenKind::Colon, "`:`")?;

        let is_array = self.peek().kind == TokenKind::LeftBracket;
        if is_array {
            self.advance();
        }
        let type_name = self.name("a type")?;
        if is_array {
            self.expect(TokenKind::RightBracket, "`]`")?;
        }
        self.expect(TokenKind::Semicolon, "`;`")?;

        Ok(FieldDeclaration {
            name,
            docs,
            type_name,
            is_array,
        })
    }

    /// Collects the `///` lines before a declaration or field, without the
    /// slashes and the space after them
    fn docs(&mut self) -> Result<Vec<String>, Diagnostic> {
        let mut docs = Vec::new();
        let mut last = None;
        while self.peek().kind == TokenKind::Doc {
            let token = self.advance();
            let text = &self.text(token)[3..];
            docs.push(text.strip_prefix(' ').unwrap_or(text).trim_end().to_owned());
            last = Some(token);
        }

        // Before the end of a declaration or the file, there's nothing to
        // document, which `expect()` would report as a missing name
        if let Some(token) = last
            && matches!(self.peek().kind, TokenKind::RightBrace | TokenKind::End)
        {
            return Err(
                Diagnostic::new("found a doc comment that documents nothing", token.span)
                    .with_help("doc comments come before what they document"),
            );
        }

        Ok(docs)
    }

    fn name(&mut self, expected: &str) -> Result<Name, Diagnostic> {
        let token = self.expect(TokenKind::Identifier, expected)?;
        Ok(Name {
            text: self.text(token).to_owned(),
            span: token.span,
        })
    }

    fn expect(&mut self, kind: TokenKind, expected: &str) -> Result<Token, Diagnostic> {
        let token = self.peek();
        if token.kind != kind {
            let found = match token.kind {
                TokenKind::End => "the end of the file".to_owned(),
                TokenKind::Doc => "a doc comment".to_owned(),
                _ => format!("`{}`", self.text(token)),
            };
            return Err(Diagnostic::new(
                format!("expected {expected}, found {found}"),
                token.span,
            ));
        }

        Ok(self.advance())
    }

    fn peek(&self) -> Token {
        self.tokens[self.index]
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.index];
        if token.kind != TokenKind::End {
            self.index += 1;
        }
        token
    }

    fn text(&self, token: Token) -> &str {
        &self.source[token.span.start..token.span.end]
    }
}
