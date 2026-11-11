//! Modèle métier de Strata Arkyo.
//!
//! Cette crate ne fait aucune entrée-sortie : elle porte les règles (droits,
//! garde-fous, validations) que le moteur applique et qui pourront être
//! compilées en WebAssembly pour le navigateur.

pub mod account;
pub mod catalog;
pub mod element;
pub mod panel;
pub mod permissions;
pub mod project;
pub mod rack;
pub mod text;

pub use account::*;
pub use permissions::*;
