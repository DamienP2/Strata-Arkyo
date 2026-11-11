//! Validation des comptes et mots de passe temporaires (cahier des charges, section 4).

use rand::seq::IndexedRandom;
use std::fmt;

/// Erreur de saisie, avec le message montré à l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    Email,
    Username,
    Name,
    Password,
    RoleName,
    Color,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ValidationError::Email => "Saisissez une adresse e-mail valide.",
            ValidationError::Username => {
                "L'identifiant compte de 3 à 30 caractères : lettres minuscules, chiffres, point, tiret bas ou tiret."
            }
            ValidationError::Name => "Le nom compte de 1 à 200 caractères.",
            ValidationError::Password => "Le mot de passe compte au moins 8 caractères.",
            ValidationError::RoleName => "Le nom du rôle compte de 1 à 64 caractères.",
            ValidationError::Color => "La couleur s'écrit #RRVVBB.",
        })
    }
}

impl std::error::Error for ValidationError {}

/// Adresse e-mail nettoyée (espaces retirés), ou erreur.
pub fn validate_email(input: &str) -> Result<String, ValidationError> {
    let email = input.trim();
    let mut parts = email.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return Err(ValidationError::Email);
    };
    let ok = !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
        && email.len() <= 254
        && !email.chars().any(|c| c.is_whitespace() || c.is_control());
    if ok {
        Ok(email.to_string())
    } else {
        Err(ValidationError::Email)
    }
}

/// Identifiant facultatif : 3 à 30 caractères parmi a-z, 0-9, « . », « _ », « - ».
/// Une saisie vide vaut « pas d'identifiant ». Les majuscules sont ramenées en minuscules.
pub fn validate_username(input: Option<&str>) -> Result<Option<String>, ValidationError> {
    let Some(raw) = input.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let u = raw.to_lowercase();
    let len = u.chars().count();
    let ok = (3..=30).contains(&len)
        && u.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(Some(u))
    } else {
        Err(ValidationError::Username)
    }
}

/// Nom affiché : 1 à 200 caractères ; par défaut la partie de l'e-mail avant « @ ».
pub fn validate_name(input: Option<&str>, email: &str) -> Result<String, ValidationError> {
    let name = match input.map(str::trim).filter(|s| !s.is_empty()) {
        Some(n) => n.to_string(),
        None => email.split('@').next().unwrap_or_default().to_string(),
    };
    let len = name.chars().count();
    if (1..=200).contains(&len) {
        Ok(name)
    } else {
        Err(ValidationError::Name)
    }
}

pub fn validate_password(input: &str) -> Result<(), ValidationError> {
    if input.chars().count() >= 8 {
        Ok(())
    } else {
        Err(ValidationError::Password)
    }
}

pub fn validate_role_name(input: &str) -> Result<String, ValidationError> {
    let name = input.trim();
    if (1..=64).contains(&name.chars().count()) {
        Ok(name.to_string())
    } else {
        Err(ValidationError::RoleName)
    }
}

pub fn validate_color(input: &str) -> Result<String, ValidationError> {
    let c = input.trim();
    let ok = c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    if ok {
        Ok(c.to_uppercase())
    } else {
        Err(ValidationError::Color)
    }
}

/// Caractères d'un mot de passe temporaire : sans 0, O, o, 1, l, I.
const TEMP_ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Mot de passe temporaire de 12 caractères, avec au moins une minuscule,
/// une majuscule et un chiffre. Affiché une seule fois.
pub fn generate_temporary_password() -> String {
    let mut rng = rand::rng();
    loop {
        let pw: String = (0..12)
            .map(|_| *TEMP_ALPHABET.choose(&mut rng).expect("alphabet non vide") as char)
            .collect();
        let lower = pw.chars().any(|c| c.is_ascii_lowercase());
        let upper = pw.chars().any(|c| c.is_ascii_uppercase());
        let digit = pw.chars().any(|c| c.is_ascii_digit());
        if lower && upper && digit {
            return pw;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails() {
        assert_eq!(
            validate_email(" lea@studio.fr "),
            Ok("lea@studio.fr".into())
        );
        for bad in [
            "",
            "lea",
            "lea@",
            "@studio.fr",
            "lea@studio",
            "le a@studio.fr",
            "a@b@c.fr",
            "lea@.fr",
        ] {
            assert_eq!(validate_email(bad), Err(ValidationError::Email), "{bad}");
        }
    }

    #[test]
    fn usernames() {
        assert_eq!(validate_username(None), Ok(None));
        assert_eq!(validate_username(Some("  ")), Ok(None));
        assert_eq!(validate_username(Some("Lea.M")), Ok(Some("lea.m".into())));
        assert!(validate_username(Some("ab")).is_err());
        assert!(validate_username(Some("léa")).is_err());
        assert!(validate_username(Some(&"a".repeat(31))).is_err());
    }

    #[test]
    fn names_default_to_email_local_part() {
        assert_eq!(
            validate_name(None, "lea.martin@studio.fr"),
            Ok("lea.martin".into())
        );
        assert_eq!(
            validate_name(Some(" Léa Martin "), "x@y.fr"),
            Ok("Léa Martin".into())
        );
        assert!(validate_name(Some(&"x".repeat(201)), "x@y.fr").is_err());
    }

    #[test]
    fn temporary_passwords() {
        for _ in 0..200 {
            let pw = generate_temporary_password();
            assert_eq!(pw.len(), 12);
            assert!(!pw.chars().any(|c| "0Oo1lI".contains(c)), "{pw}");
            assert!(pw.chars().any(|c| c.is_ascii_lowercase()));
            assert!(pw.chars().any(|c| c.is_ascii_uppercase()));
            assert!(pw.chars().any(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn colors_and_role_names() {
        assert_eq!(validate_color("#15935a"), Ok("#15935A".into()));
        assert!(validate_color("15935A").is_err());
        assert!(validate_role_name("").is_err());
        assert!(validate_role_name(&"r".repeat(65)).is_err());
    }
}
