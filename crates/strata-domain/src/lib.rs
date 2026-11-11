//! Modèle métier de Strata Arkyo.
//!
//! Cette crate ne fait aucune entrée-sortie : elle porte les règles (droits,
//! garde-fous, validations) que le moteur applique et qui pourront être
//! compilées en WebAssembly pour le navigateur.

pub mod account;
pub mod permissions;

pub use account::*;
pub use permissions::*;
