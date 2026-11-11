//! Comparaison de textes sans tenir compte de la casse ni des accents.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Clé de comparaison : minuscules, sans accents, espaces réduits.
pub fn normalize(s: &str) -> String {
    let stripped: String = s.nfd().filter(|c| !is_combining_mark(*c)).collect::<String>().to_lowercase();
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accents_case_and_spaces() {
        assert_eq!(normalize("  Écran   Régie "), "ecran regie");
        assert_eq!(normalize("ŒUVRE"), "œuvre");
        assert_eq!(normalize("Mélangeur 4 M/E"), normalize("melangeur 4 m/e"));
    }
}
