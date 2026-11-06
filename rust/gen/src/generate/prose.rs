//! The English of generated documentation, which every target shares

/// The indefinite article before a declaration's name in documentation, by
/// how the name sounds: "an" before a vowel, but "a" before a `U` read as
/// "you", as in `Uint8Offsets`, `Unit` or `User`. It reads `U` that way when
/// a vowel follows it, or a consonant and then a vowel, so `Under` and
/// `Upper` take "an".
pub fn indefinite_article(name: &str) -> &'static str {
    let mut letters = name.chars().map(|letter| letter.to_ascii_lowercase());
    let is_vowel = |letter: Option<char>| letter.is_some_and(|letter| "aeiou".contains(letter));

    match letters.next() {
        Some('a' | 'e' | 'i' | 'o') => "an",
        Some('u') => {
            let second = letters.next();
            if is_vowel(second) || (second.is_some() && is_vowel(letters.next())) {
                "a"
            } else {
                "an"
            }
        }
        _ => "a",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indefinite_article() {
        let cases = [
            ("Point", "a"),
            ("Array", "an"),
            ("Event", "an"),
            ("Item", "an"),
            ("Order", "an"),
            ("Uint8Offsets", "a"),
            ("Unit", "a"),
            ("User", "a"),
            ("Under", "an"),
            ("Upper", "an"),
            ("U", "an"),
            ("Ux", "an"),
        ];

        for (name, article) in cases {
            assert_eq!(indefinite_article(name), article, "{name}");
        }
    }
}
