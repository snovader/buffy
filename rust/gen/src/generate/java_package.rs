//! The package of generated Java classes, whose names are the directories of
//! their files too

use std::fmt;
use std::str::FromStr;

/// Java's keywords and literals, which a package's names can't be, and which
/// the Java generator's names take a `_` after
pub(super) const KEYWORDS: [&str; 54] = [
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "void",
    "volatile",
    "while",
    "_",
    "true",
    "false",
    "null",
];

/// The name of a Java package, such as `com.example.shapes`: identifiers in
/// ASCII, separated by dots. Its names are the directories of the generated
/// classes' files, so it can't name any other directory, such as an absolute
/// path or a parent directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaPackage(String);

impl JavaPackage {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for JavaPackage {
    type Err = InvalidJavaPackage;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        if name.split('.').all(is_identifier) {
            Ok(Self(name.to_owned()))
        } else {
            Err(InvalidJavaPackage(name.to_owned()))
        }
    }
}

/// A name that isn't a Java package's
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidJavaPackage(String);

impl fmt::Display for InvalidJavaPackage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` isn't a Java package, which is identifiers in ASCII separated by dots, \
             such as `com.example.shapes`",
            self.0
        )
    }
}

impl std::error::Error for InvalidJavaPackage {}

/// Whether one of a package's names is a Java identifier in ASCII: a letter,
/// `_` or `$`, then letters, digits, `_` and `$`, other than a keyword
fn is_identifier(name: &str) -> bool {
    name.starts_with(|character: char| character.is_ascii_alphabetic() || "_$".contains(character))
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_$".contains(character))
        && !KEYWORDS.contains(&name)
}
