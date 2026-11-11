//! Droits par rôle : cinq domaines, quatre niveaux ordonnés (cahier des charges, section 3).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Niveau de droit sur un domaine : aucun < lecture < écriture < admin.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    #[default]
    Aucun,
    Lecture,
    Ecriture,
    Admin,
}

impl Level {
    pub fn as_i64(self) -> i64 {
        match self {
            Level::Aucun => 0,
            Level::Lecture => 1,
            Level::Ecriture => 2,
            Level::Admin => 3,
        }
    }

    /// Une valeur inconnue (base écrite par une version plus récente) vaut « aucun ».
    pub fn from_i64(v: i64) -> Level {
        match v {
            1 => Level::Lecture,
            2 => Level::Ecriture,
            3 => Level::Admin,
            _ => Level::Aucun,
        }
    }
}

/// Les cinq domaines de droits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Projets,
    Conception,
    Catalogue,
    Membres,
    Parametres,
}

impl Domain {
    pub const ALL: [Domain; 5] = [
        Domain::Projets,
        Domain::Conception,
        Domain::Catalogue,
        Domain::Membres,
        Domain::Parametres,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Domain::Projets => "Projets",
            Domain::Conception => "Conception",
            Domain::Catalogue => "Catalogue",
            Domain::Membres => "Membres et rôles",
            Domain::Parametres => "Paramètres et site",
        }
    }
}

/// Niveau par domaine. Un domaine absent vaut « aucun ».
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Permissions {
    pub projets: Level,
    pub conception: Level,
    pub catalogue: Level,
    pub membres: Level,
    pub parametres: Level,
}

impl Permissions {
    pub fn get(&self, d: Domain) -> Level {
        match d {
            Domain::Projets => self.projets,
            Domain::Conception => self.conception,
            Domain::Catalogue => self.catalogue,
            Domain::Membres => self.membres,
            Domain::Parametres => self.parametres,
        }
    }

    pub fn set(&mut self, d: Domain, l: Level) {
        match d {
            Domain::Projets => self.projets = l,
            Domain::Conception => self.conception = l,
            Domain::Catalogue => self.catalogue = l,
            Domain::Membres => self.membres = l,
            Domain::Parametres => self.parametres = l,
        }
    }

    /// Vrai si aucun domaine ne dépasse le niveau correspondant de `limit`.
    pub fn within(&self, limit: &Permissions) -> bool {
        Domain::ALL.iter().all(|d| self.get(*d) <= limit.get(*d))
    }

    pub fn all(level: Level) -> Permissions {
        Permissions {
            projets: level,
            conception: level,
            catalogue: level,
            membres: level,
            parametres: level,
        }
    }
}

/// Identifiants des rôles fournis.
pub mod builtin {
    pub const OWNER: &str = "owner";
    pub const PROJECT_LEAD: &str = "project_lead";
    pub const TECHNICIAN: &str = "technician";
    pub const VIEWER: &str = "viewer";

    pub fn is_builtin(id: &str) -> bool {
        matches!(id, OWNER | PROJECT_LEAD | TECHNICIAN | VIEWER)
    }
}

/// Ce que les garde-fous ont besoin de savoir d'un rôle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleInfo {
    pub id: String,
    pub perms: Permissions,
}

impl RoleInfo {
    pub fn is_owner(&self) -> bool {
        self.id == builtin::OWNER
    }
}

/// La personne qui agit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub account_id: String,
    pub role: RoleInfo,
}

impl Actor {
    pub fn is_owner(&self) -> bool {
        self.role.is_owner()
    }

    /// Le propriétaire passe tous les contrôles.
    pub fn allows(&self, domain: Domain, level: Level) -> bool {
        self.is_owner() || self.role.perms.get(domain) >= level
    }

    /// Peut accorder ou attribuer ces permissions : jamais au-dessus des siennes.
    pub fn can_grant(&self, perms: &Permissions) -> bool {
        self.is_owner() || perms.within(&self.role.perms)
    }
}

/// Refus d'un garde-fou, avec le message montré à l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    OwnRole,
    OwnPermissions,
    OwnerOnly,
    AboveOwnLevel,
    LastOwner,
    SelfDeactivation,
    OwnerRoleLocked,
    OwnerAtCreation,
    BuiltinRoleDeletion,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Refusal::OwnRole => "Vous ne pouvez pas modifier votre propre rôle.",
            Refusal::OwnPermissions => {
                "Vous ne pouvez pas modifier les permissions de votre propre rôle."
            }
            Refusal::OwnerOnly => {
                "Seul un propriétaire peut nommer, modifier, désactiver ou retirer un propriétaire."
            }
            Refusal::AboveOwnLevel => "Vous ne pouvez pas accorder un niveau supérieur au vôtre.",
            Refusal::LastOwner => "Il doit rester au moins un propriétaire actif.",
            Refusal::SelfDeactivation => "Vous ne pouvez pas vous désactiver vous-même.",
            Refusal::OwnerRoleLocked => "Le rôle Propriétaire est verrouillé.",
            Refusal::OwnerAtCreation => {
                "Un nouveau compte ne peut pas recevoir le rôle Propriétaire."
            }
            Refusal::BuiltinRoleDeletion => "Les rôles fournis ne se suppriment pas.",
        })
    }
}

impl std::error::Error for Refusal {}

/// Attribuer `new` au compte `target_id` qui a aujourd'hui `current`.
pub fn check_role_assignment(
    actor: &Actor,
    target_id: &str,
    current: &RoleInfo,
    new: &RoleInfo,
    active_owners: i64,
) -> Result<(), Refusal> {
    if !actor.is_owner() && actor.account_id == target_id {
        return Err(Refusal::OwnRole);
    }
    if (current.is_owner() || new.is_owner()) && !actor.is_owner() {
        return Err(Refusal::OwnerOnly);
    }
    if !actor.can_grant(&new.perms) {
        return Err(Refusal::AboveOwnLevel);
    }
    if current.is_owner() && !new.is_owner() && active_owners <= 1 {
        return Err(Refusal::LastOwner);
    }
    Ok(())
}

/// Désactiver ou retirer le compte `target_id` qui a le rôle `target_role`.
pub fn check_deactivation(
    actor: &Actor,
    target_id: &str,
    target_role: &RoleInfo,
    active_owners: i64,
) -> Result<(), Refusal> {
    if actor.account_id == target_id {
        return Err(Refusal::SelfDeactivation);
    }
    if target_role.is_owner() && !actor.is_owner() {
        return Err(Refusal::OwnerOnly);
    }
    if target_role.is_owner() && active_owners <= 1 {
        return Err(Refusal::LastOwner);
    }
    Ok(())
}

/// Réinitialiser le mot de passe d'un compte qui a le rôle `target_role`.
pub fn check_password_reset(actor: &Actor, target_role: &RoleInfo) -> Result<(), Refusal> {
    if target_role.is_owner() && !actor.is_owner() {
        return Err(Refusal::OwnerOnly);
    }
    Ok(())
}

/// Créer un compte avec le rôle `role`.
pub fn check_account_creation(actor: &Actor, role: &RoleInfo) -> Result<(), Refusal> {
    if role.is_owner() {
        return Err(Refusal::OwnerAtCreation);
    }
    if !actor.can_grant(&role.perms) {
        return Err(Refusal::AboveOwnLevel);
    }
    Ok(())
}

/// Créer un rôle personnalisé ou en modifier les permissions.
pub fn check_role_definition(
    actor: &Actor,
    existing: Option<&RoleInfo>,
    perms: &Permissions,
) -> Result<(), Refusal> {
    if let Some(role) = existing {
        if role.is_owner() {
            return Err(Refusal::OwnerRoleLocked);
        }
        if !actor.is_owner() && actor.role.id == role.id {
            return Err(Refusal::OwnPermissions);
        }
    }
    if !actor.can_grant(perms) {
        return Err(Refusal::AboveOwnLevel);
    }
    Ok(())
}

/// Supprimer un rôle.
pub fn check_role_deletion(actor: &Actor, role: &RoleInfo) -> Result<(), Refusal> {
    if builtin::is_builtin(&role.id) {
        return Err(Refusal::BuiltinRoleDeletion);
    }
    if !actor.is_owner() && actor.role.id == role.id {
        return Err(Refusal::OwnPermissions);
    }
    if !actor.can_grant(&role.perms) {
        return Err(Refusal::AboveOwnLevel);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(id: &str, perms: Permissions) -> RoleInfo {
        RoleInfo {
            id: id.into(),
            perms,
        }
    }

    fn lead() -> RoleInfo {
        role(
            builtin::PROJECT_LEAD,
            Permissions {
                projets: Level::Ecriture,
                conception: Level::Ecriture,
                catalogue: Level::Ecriture,
                membres: Level::Admin,
                parametres: Level::Aucun,
            },
        )
    }

    fn owner_actor() -> Actor {
        Actor {
            account_id: "o".into(),
            role: role(builtin::OWNER, Permissions::all(Level::Admin)),
        }
    }

    fn lead_actor() -> Actor {
        Actor {
            account_id: "l".into(),
            role: lead(),
        }
    }

    fn tech() -> RoleInfo {
        role(
            builtin::TECHNICIAN,
            Permissions {
                projets: Level::Ecriture,
                conception: Level::Ecriture,
                catalogue: Level::Lecture,
                ..Default::default()
            },
        )
    }

    #[test]
    fn levels_are_ordered() {
        assert!(
            Level::Aucun < Level::Lecture
                && Level::Lecture < Level::Ecriture
                && Level::Ecriture < Level::Admin
        );
        assert_eq!(Level::from_i64(99), Level::Aucun);
    }

    #[test]
    fn owner_passes_every_check() {
        assert!(owner_actor().allows(Domain::Parametres, Level::Admin));
        assert!(!lead_actor().allows(Domain::Parametres, Level::Lecture));
    }

    #[test]
    fn cannot_grant_above_own_level() {
        let admin_params = role(
            "x",
            Permissions {
                parametres: Level::Admin,
                ..Default::default()
            },
        );
        assert_eq!(
            check_account_creation(&lead_actor(), &admin_params),
            Err(Refusal::AboveOwnLevel)
        );
        assert_eq!(check_account_creation(&lead_actor(), &tech()), Ok(()));
    }

    #[test]
    fn nobody_is_created_owner() {
        let owner = role(builtin::OWNER, Permissions::all(Level::Admin));
        assert_eq!(
            check_account_creation(&owner_actor(), &owner),
            Err(Refusal::OwnerAtCreation)
        );
    }

    #[test]
    fn non_owner_cannot_change_own_role() {
        let a = lead_actor();
        assert_eq!(
            check_role_assignment(&a, "l", &lead(), &tech(), 1),
            Err(Refusal::OwnRole)
        );
    }

    #[test]
    fn only_owner_touches_owners() {
        let owner = role(builtin::OWNER, Permissions::all(Level::Admin));
        assert_eq!(
            check_role_assignment(&lead_actor(), "t", &tech(), &owner, 1),
            Err(Refusal::OwnerOnly)
        );
        assert_eq!(
            check_deactivation(&lead_actor(), "o2", &owner, 2),
            Err(Refusal::OwnerOnly)
        );
        assert_eq!(
            check_password_reset(&lead_actor(), &owner),
            Err(Refusal::OwnerOnly)
        );
    }

    #[test]
    fn last_owner_stays() {
        let owner = role(builtin::OWNER, Permissions::all(Level::Admin));
        assert_eq!(
            check_role_assignment(&owner_actor(), "o2", &owner, &tech(), 1),
            Err(Refusal::LastOwner)
        );
        assert_eq!(
            check_role_assignment(&owner_actor(), "o2", &owner, &tech(), 2),
            Ok(())
        );
        assert_eq!(
            check_deactivation(&owner_actor(), "o2", &owner, 1),
            Err(Refusal::LastOwner)
        );
    }

    #[test]
    fn no_self_deactivation() {
        assert_eq!(
            check_deactivation(&owner_actor(), "o", &tech(), 3),
            Err(Refusal::SelfDeactivation)
        );
    }

    #[test]
    fn owner_role_is_locked_and_builtins_stay() {
        let owner = role(builtin::OWNER, Permissions::all(Level::Admin));
        assert_eq!(
            check_role_definition(&owner_actor(), Some(&owner), &Permissions::default()),
            Err(Refusal::OwnerRoleLocked)
        );
        assert_eq!(
            check_role_deletion(&owner_actor(), &tech()),
            Err(Refusal::BuiltinRoleDeletion)
        );
        assert_eq!(
            check_role_definition(&lead_actor(), Some(&lead()), &Permissions::default()),
            Err(Refusal::OwnPermissions)
        );
    }
}
