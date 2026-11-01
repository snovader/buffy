use crate::diagnostic::{Diagnostic, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    Integer,
    /// A `///` comment, which documents the declaration after it
    Doc,
    LeftBrace,
    RightBrace,
    LeftParenthesis,
    RightParenthesis,
    LeftBracket,
    RightBracket,
    Colon,
    Semicolon,
    Comma,
    End,
}

#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// Splits a schema into tokens, dropping whitespace and `//` comments
pub fn tokenize(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        let start = index;
        let byte = bytes[index];

        let kind = match byte {
            b' ' | b'\t' | b'\r' | b'\n' => {
                index += 1;
                continue;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                let is_doc =
                    bytes.get(index + 2) == Some(&b'/') && bytes.get(index + 3) != Some(&b'/');
                index = source[index..]
                    .find('\n')
                    .map_or(bytes.len(), |end| index + end);

                if !is_doc {
                    continue;
                }
                TokenKind::Doc
            }
            b'{' => single(&mut index, TokenKind::LeftBrace),
            b'}' => single(&mut index, TokenKind::RightBrace),
            b'(' => single(&mut index, TokenKind::LeftParenthesis),
            b')' => single(&mut index, TokenKind::RightParenthesis),
            b'[' => single(&mut index, TokenKind::LeftBracket),
            b']' => single(&mut index, TokenKind::RightBracket),
            b':' => single(&mut index, TokenKind::Colon),
            b';' => single(&mut index, TokenKind::Semicolon),
            b',' => single(&mut index, TokenKind::Comma),
            b'0'..=b'9' => {
                index += count_while(&bytes[index..], u8::is_ascii_digit);
                TokenKind::Integer
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                index += count_while(&bytes[index..], |byte| {
                    byte.is_ascii_alphanumeric() || *byte == b'_'
                });
                TokenKind::Identifier
            }
            _ => {
                let character = source[index..].chars().next().unwrap_or_default();
                let span = Span {
                    start,
                    end: start + character.len_utf8(),
                };
                return Err(Diagnostic::new(
                    format!("unexpected character `{character}`"),
                    span,
                ));
            }
        };

        tokens.push(Token {
            kind,
            span: Span { start, end: index },
        });
    }

    let end = Span {
        start: bytes.len(),
        end: bytes.len(),
    };
    tokens.push(Token {
        kind: TokenKind::End,
        span: end,
    });

    Ok(tokens)
}

fn single(index: &mut usize, kind: TokenKind) -> TokenKind {
    *index += 1;
    kind
}

fn count_while(bytes: &[u8], predicate: impl Fn(&u8) -> bool) -> usize {
    bytes.iter().take_while(|byte| predicate(byte)).count()
}
