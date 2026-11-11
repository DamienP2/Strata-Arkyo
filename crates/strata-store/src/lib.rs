//! Stockage de Strata Arkyo dans une base SQLite (mode WAL).
//!
//! Toute la base vit dans un seul fichier du dossier de données. Les migrations
//! sont additives (voir `migrations/`), ce qui permet à une version plus ancienne
//! du moteur de rouvrir une base écrite par une version plus récente.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow, SqliteSynchronous,
};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use strata_domain::{Level, Permissions, RoleInfo, builtin};

pub mod catalog;
pub mod projects;
pub use catalog::*;
pub use projects::*;

/// Migrations du schéma. Une base écrite par une version plus récente porte des
/// migrations que cette version ne connaît pas : elles sont ignorées, ce qui permet
/// le retour arrière d'une mise à jour (les migrations étant additives).
fn migrator() -> sqlx::migrate::Migrator {
    let mut m = sqlx::migrate!("./migrations");
    m.set_ignore_missing(true);
    m
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Une contrainte d'unicité a refusé l'écriture ; le champ en cause est nommé.
    #[error("valeur déjà utilisée : {0}")]
    Conflict(&'static str),
    #[error("introuvable")]
    NotFound,
    /// La révision attendue n'est plus la bonne : quelqu'un a modifié l'objet entre-temps.
    #[error("révision dépassée")]
    Stale,
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// Horodatage RFC 3339 en UTC, à la seconde.
pub fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn timestamp(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub(crate) fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Traduit une violation d'unicité en `Conflict` nommant le champ.
pub(crate) fn map_unique(e: sqlx::Error) -> StoreError {
    if let sqlx::Error::Database(db) = &e {
        if db.is_unique_violation() {
            let msg = db.message();
            let field = if msg.contains("accounts_email") {
                "email"
            } else if msg.contains("accounts_username") || msg.contains("accounts.username") {
                "username"
            } else if msg.contains("roles_name") {
                "role_name"
            } else if msg.contains("projects.name_norm") {
                "project_name"
            } else if msg.contains("containers.name_norm") {
                "container_name"
            } else {
                "unique"
            };
            return StoreError::Conflict(field);
        }
    }
    StoreError::Db(e)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Account {
    pub id: String,
    pub email: String,
    pub username: Option<String>,
    pub name: String,
    pub role_id: String,
    pub active: bool,
    pub removed: bool,
    pub must_change_password: bool,
    pub phone: Option<String>,
    pub job: Option<String>,
    pub lang: String,
    pub theme: String,
    pub rev: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl Account {
    fn from_row(r: &SqliteRow) -> Account {
        Account {
            id: r.get("id"),
            email: r.get("email"),
            username: r.get("username"),
            name: r.get("name"),
            role_id: r.get("role_id"),
            active: r.get::<i64, _>("active") != 0,
            removed: r.get::<i64, _>("removed") != 0,
            must_change_password: r.get::<i64, _>("must_change_password") != 0,
            phone: r.get("phone"),
            job: r.get("job"),
            lang: r.get("lang"),
            theme: r.get("theme"),
            rev: r.get("rev"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }
    }
}

const ACCOUNT_COLS: &str = "id, email, username, name, role_id, active, removed, must_change_password, \
     phone, job, lang, theme, rev, created_at, updated_at";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub color: String,
    pub builtin: bool,
    pub locked: bool,
    pub permissions: Permissions,
    pub rev: i64,
    /// Nombre de membres actifs qui ont ce rôle.
    pub active_members: i64,
}

impl Role {
    fn from_row(r: &SqliteRow) -> Role {
        let id: String = r.get("id");
        Role {
            locked: id == builtin::OWNER,
            name: r.get("name"),
            color: r.get("color"),
            builtin: r.get::<i64, _>("builtin") != 0,
            permissions: Permissions {
                projets: Level::from_i64(r.get("perm_projets")),
                conception: Level::from_i64(r.get("perm_conception")),
                catalogue: Level::from_i64(r.get("perm_catalogue")),
                membres: Level::from_i64(r.get("perm_membres")),
                parametres: Level::from_i64(r.get("perm_parametres")),
            },
            rev: r.get("rev"),
            active_members: r.get("active_members"),
            id,
        }
    }

    pub fn info(&self) -> RoleInfo {
        RoleInfo {
            id: self.id.clone(),
            perms: self.permissions,
        }
    }
}

const ROLE_SELECT: &str = "SELECT r.*, (SELECT COUNT(*) FROM accounts a WHERE a.role_id = r.id AND a.active = 1 AND a.removed = 0) AS active_members FROM roles r";

pub struct NewAccount<'a> {
    pub email: &'a str,
    pub username: Option<&'a str>,
    pub name: &'a str,
    pub password_hash: &'a str,
    pub must_change_password: bool,
    pub role_id: &'a str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AuditEntry {
    pub id: i64,
    pub at: String,
    pub actor_id: Option<String>,
    pub action: String,
    pub object_type: String,
    pub object_id: String,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
}

/// Ouvre un fichier de base en lecture seule et renvoie (intégrité correcte, version de schéma).
pub async fn check_database_file(path: &Path) -> Result<(bool, i64)> {
    let opts = SqliteConnectOptions::new().filename(path).read_only(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await?;
    let schema: Option<i64> =
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
            .fetch_one(&pool)
            .await
            .unwrap_or(None);
    pool.close().await;
    Ok((integrity == "ok", schema.unwrap_or(0)))
}

#[derive(Clone)]
pub struct Store {
    pub(crate) pool: SqlitePool,
}

impl Store {
    /// Ouvre (ou crée) la base au chemin donné et applique les migrations.
    pub async fn open(path: &Path) -> Result<Store> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opts)
            .await?;
        let store = Store { pool };
        store.migrate().await?;
        Ok(store)
    }

    /// Base en mémoire, pour les tests.
    pub async fn open_in_memory() -> Result<Store> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(opts)
            .await?;
        let store = Store { pool };
        store.migrate().await?;
        Ok(store)
    }

    pub async fn migrate(&self) -> Result<()> {
        migrator().run(&self.pool).await?;
        self.seed_catalog().await?;
        Ok(())
    }

    /// Version de schéma la plus récente appliquée.
    pub async fn schema_version(&self) -> Result<i64> {
        let v: Option<i64> =
            sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
                .fetch_one(&self.pool)
                .await?;
        Ok(v.unwrap_or(0))
    }

    /// `PRAGMA integrity_check` : vrai si la base est saine.
    pub async fn integrity_ok(&self) -> Result<bool> {
        let r: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&self.pool)
            .await?;
        Ok(r == "ok")
    }

    /// Copie cohérente de la base, à chaud (`VACUUM INTO`), vers un fichier qui n'existe pas encore.
    pub async fn vacuum_into(&self, dest: &Path) -> Result<()> {
        sqlx::query("VACUUM INTO ?")
            .bind(dest.to_string_lossy().to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    // ----- Réglages du site -----

    pub async fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(sqlx::query_scalar("SELECT value FROM meta WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Enregistre un réglage ; `None` le remet à sa valeur par défaut.
    pub async fn set_meta(&self, key: &str, value: Option<&str>) -> Result<()> {
        match value {
            Some(v) => {
                sqlx::query("INSERT INTO meta (key, value) VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value")
                    .bind(key)
                    .bind(v)
                    .execute(&self.pool)
                    .await?;
            }
            None => {
                sqlx::query("DELETE FROM meta WHERE key = ?")
                    .bind(key)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }

    // ----- Comptes -----

    pub async fn count_accounts(&self) -> Result<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn insert_account(&self, a: NewAccount<'_>) -> Result<Account> {
        let id = new_id();
        let t = now();
        sqlx::query(
            "INSERT INTO accounts (id, email, username, name, password_hash, must_change_password, role_id, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(a.email)
        .bind(a.username)
        .bind(a.name)
        .bind(a.password_hash)
        .bind(a.must_change_password as i64)
        .bind(a.role_id)
        .bind(&t)
        .bind(&t)
        .execute(&self.pool)
        .await
        .map_err(map_unique)?;
        self.account(&id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn account(&self, id: &str) -> Result<Option<Account>> {
        let q = format!("SELECT {ACCOUNT_COLS} FROM accounts WHERE id = ?");
        let row = sqlx::query(&q).bind(id).fetch_optional(&self.pool).await?;
        Ok(row.as_ref().map(Account::from_row))
    }

    pub async fn account_by_email(&self, email: &str) -> Result<Option<Account>> {
        let q = format!("SELECT {ACCOUNT_COLS} FROM accounts WHERE lower(email) = lower(?)");
        let row = sqlx::query(&q)
            .bind(email.trim())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(Account::from_row))
    }

    /// Compte et empreinte du mot de passe, par e-mail ou identifiant, sans tenir compte de la casse.
    pub async fn credentials(&self, login: &str) -> Result<Option<(Account, String)>> {
        let q = format!(
            "SELECT {ACCOUNT_COLS}, password_hash FROM accounts \
             WHERE lower(email) = lower(?1) OR username = lower(?1) LIMIT 1"
        );
        let row = sqlx::query(&q)
            .bind(login.trim())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| (Account::from_row(&r), r.get("password_hash"))))
    }

    pub async fn password_hash(&self, id: &str) -> Result<Option<String>> {
        Ok(
            sqlx::query_scalar("SELECT password_hash FROM accounts WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_accounts(&self, include_removed: bool) -> Result<Vec<Account>> {
        let q = format!(
            "SELECT {ACCOUNT_COLS} FROM accounts {} ORDER BY lower(name)",
            if include_removed {
                ""
            } else {
                "WHERE removed = 0"
            }
        );
        let rows = sqlx::query(&q).fetch_all(&self.pool).await?;
        Ok(rows.iter().map(Account::from_row).collect())
    }

    async fn touch_account(&self, id: &str, set: &str, binds: Vec<Bind>) -> Result<Account> {
        let q = format!("UPDATE accounts SET {set}, rev = rev + 1, updated_at = ? WHERE id = ?");
        let mut query = sqlx::query(&q);
        for b in binds {
            query = match b {
                Bind::Text(s) => query.bind(s),
                Bind::OptText(s) => query.bind(s),
                Bind::Int(i) => query.bind(i),
            };
        }
        let res = query
            .bind(now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_unique)?;
        if res.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        self.account(id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn set_account_role(&self, id: &str, role_id: &str) -> Result<Account> {
        self.touch_account(id, "role_id = ?", vec![Bind::Text(role_id.into())])
            .await
    }

    pub async fn set_account_active(&self, id: &str, active: bool) -> Result<Account> {
        self.touch_account(id, "active = ?", vec![Bind::Int(active as i64)])
            .await
    }

    /// Retire un membre : il est désactivé, son historique est gardé.
    pub async fn mark_account_removed(&self, id: &str) -> Result<Account> {
        self.touch_account(id, "active = 0, removed = 1", vec![])
            .await
    }

    pub async fn set_password(&self, id: &str, hash: &str, must_change: bool) -> Result<Account> {
        self.touch_account(
            id,
            "password_hash = ?, must_change_password = ?",
            vec![Bind::Text(hash.into()), Bind::Int(must_change as i64)],
        )
        .await
    }

    /// Recrée un compte retiré : il redevient actif avec un nouveau mot de passe temporaire.
    pub async fn reactivate_account(
        &self,
        id: &str,
        name: &str,
        username: Option<&str>,
        role_id: &str,
        hash: &str,
    ) -> Result<Account> {
        self.touch_account(
            id,
            "name = ?, username = ?, role_id = ?, password_hash = ?, must_change_password = 1, active = 1, removed = 0",
            vec![
                Bind::Text(name.into()),
                Bind::OptText(username.map(str::to_string)),
                Bind::Text(role_id.into()),
                Bind::Text(hash.into()),
            ],
        )
        .await
    }

    /// Nombre de propriétaires actifs.
    pub async fn count_active_owners(&self) -> Result<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM accounts WHERE role_id = ? AND active = 1 AND removed = 0",
        )
        .bind(builtin::OWNER)
        .fetch_one(&self.pool)
        .await?)
    }

    // ----- Rôles -----

    pub async fn list_roles(&self) -> Result<Vec<Role>> {
        let q = format!(
            "{ROLE_SELECT} ORDER BY CASE r.id WHEN 'owner' THEN 0 WHEN 'project_lead' THEN 1 \
             WHEN 'technician' THEN 2 WHEN 'viewer' THEN 3 ELSE 4 END, lower(r.name)"
        );
        let rows = sqlx::query(&q).fetch_all(&self.pool).await?;
        Ok(rows.iter().map(Role::from_row).collect())
    }

    pub async fn role(&self, id: &str) -> Result<Option<Role>> {
        let q = format!("{ROLE_SELECT} WHERE r.id = ?");
        let row = sqlx::query(&q).bind(id).fetch_optional(&self.pool).await?;
        Ok(row.as_ref().map(Role::from_row))
    }

    pub async fn role_name_taken(&self, name: &str, except_id: Option<&str>) -> Result<bool> {
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM roles WHERE lower(name) = lower(?) AND id != ?",
        )
        .bind(name)
        .bind(except_id.unwrap_or(""))
        .fetch_one(&self.pool)
        .await?;
        Ok(n > 0)
    }

    pub async fn insert_role(&self, name: &str, color: &str, p: &Permissions) -> Result<Role> {
        let id = new_id();
        let t = now();
        sqlx::query(
            "INSERT INTO roles (id, name, color, builtin, perm_projets, perm_conception, perm_catalogue, perm_membres, perm_parametres, created_at, updated_at) \
             VALUES (?, ?, ?, 0, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(color)
        .bind(p.projets.as_i64())
        .bind(p.conception.as_i64())
        .bind(p.catalogue.as_i64())
        .bind(p.membres.as_i64())
        .bind(p.parametres.as_i64())
        .bind(&t)
        .bind(&t)
        .execute(&self.pool)
        .await
        .map_err(map_unique)?;
        self.role(&id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn update_role(
        &self,
        id: &str,
        name: &str,
        color: &str,
        p: &Permissions,
    ) -> Result<Role> {
        let res = sqlx::query(
            "UPDATE roles SET name = ?, color = ?, perm_projets = ?, perm_conception = ?, perm_catalogue = ?, \
             perm_membres = ?, perm_parametres = ?, rev = rev + 1, updated_at = ? WHERE id = ?",
        )
        .bind(name)
        .bind(color)
        .bind(p.projets.as_i64())
        .bind(p.conception.as_i64())
        .bind(p.catalogue.as_i64())
        .bind(p.membres.as_i64())
        .bind(p.parametres.as_i64())
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_unique)?;
        if res.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        self.role(id).await?.ok_or(StoreError::NotFound)
    }

    /// Supprime un rôle ; ses membres passent Technicien. Renvoie le nombre de membres déplacés.
    pub async fn delete_role(&self, id: &str) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        let moved = sqlx::query(
            "UPDATE accounts SET role_id = ?, rev = rev + 1, updated_at = ? WHERE role_id = ?",
        )
        .bind(builtin::TECHNICIAN)
        .bind(now())
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let res = sqlx::query("DELETE FROM roles WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if res.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        tx.commit().await?;
        Ok(moved)
    }

    // ----- Sessions -----

    pub async fn insert_session(
        &self,
        token_hash: &str,
        account_id: &str,
        expires_at: DateTime<Utc>,
        user_agent: Option<&str>,
    ) -> Result<()> {
        let t = now();
        sqlx::query(
            "INSERT INTO sessions (token_hash, account_id, created_at, last_seen_at, expires_at, user_agent) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(token_hash)
        .bind(account_id)
        .bind(&t)
        .bind(&t)
        .bind(timestamp(expires_at))
        .bind(user_agent)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Compte de la session si elle existe et n'a pas expiré.
    pub async fn session_account(&self, token_hash: &str) -> Result<Option<Account>> {
        let q = format!(
            "SELECT {} FROM sessions s JOIN accounts a ON a.id = s.account_id WHERE s.token_hash = ? AND s.expires_at > ?",
            ACCOUNT_COLS
                .split(", ")
                .map(|c| format!("a.{}", c.trim()))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let row = sqlx::query(&q)
            .bind(token_hash)
            .bind(now())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(Account::from_row))
    }

    pub async fn touch_session(&self, token_hash: &str) -> Result<()> {
        sqlx::query("UPDATE sessions SET last_seen_at = ? WHERE token_hash = ?")
            .bind(now())
            .bind(token_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_session(&self, token_hash: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(token_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Ferme les sessions d'un compte, sauf éventuellement celle en cours.
    pub async fn delete_sessions(&self, account_id: &str, keep: Option<&str>) -> Result<u64> {
        let res = sqlx::query("DELETE FROM sessions WHERE account_id = ? AND token_hash != ?")
            .bind(account_id)
            .bind(keep.unwrap_or(""))
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    pub async fn purge_expired_sessions(&self) -> Result<u64> {
        let res = sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
            .bind(now())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    // ----- Journal d'audit -----

    /// Enregistre une création, modification ou suppression avec l'état avant et après.
    pub async fn audit<B: Serialize, A: Serialize>(
        &self,
        actor_id: Option<&str>,
        action: &str,
        object_type: &str,
        object_id: &str,
        before: Option<&B>,
        after: Option<&A>,
    ) -> Result<()> {
        let before = before.map(|b| serde_json::to_string(b).unwrap_or_default());
        let after = after.map(|a| serde_json::to_string(a).unwrap_or_default());
        sqlx::query(
            "INSERT INTO audit (at, actor_id, action, object_type, object_id, before, after) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(now())
        .bind(actor_id)
        .bind(action)
        .bind(object_type)
        .bind(object_id)
        .bind(before)
        .bind(after)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Entrées du journal, de la plus récente à la plus ancienne.
    pub async fn audit_entries(&self, limit: i64) -> Result<Vec<AuditEntry>> {
        let rows = sqlx::query("SELECT * FROM audit ORDER BY id DESC LIMIT ?")
            .bind(limit.clamp(1, 500))
            .fetch_all(&self.pool)
            .await?;
        let parse = |s: Option<String>| s.and_then(|s| serde_json::from_str(&s).ok());
        Ok(rows
            .iter()
            .map(|r| AuditEntry {
                id: r.get("id"),
                at: r.get("at"),
                actor_id: r.get("actor_id"),
                action: r.get("action"),
                object_type: r.get("object_type"),
                object_id: r.get("object_id"),
                before: parse(r.get("before")),
                after: parse(r.get("after")),
            })
            .collect())
    }
}

enum Bind {
    Text(String),
    OptText(Option<String>),
    Int(i64),
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn store() -> Store {
        Store::open_in_memory().await.unwrap()
    }

    fn new<'a>(email: &'a str, username: Option<&'a str>, role: &'a str) -> NewAccount<'a> {
        NewAccount {
            email,
            username,
            name: "Test",
            password_hash: "h",
            must_change_password: false,
            role_id: role,
        }
    }

    #[tokio::test]
    async fn builtin_roles_are_seeded() {
        let s = store().await;
        let roles = s.list_roles().await.unwrap();
        let ids: Vec<_> = roles.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["owner", "project_lead", "technician", "viewer"]);
        let tech = s.role("technician").await.unwrap().unwrap();
        assert_eq!(tech.permissions.catalogue, Level::Lecture);
        assert!(s.role("owner").await.unwrap().unwrap().locked);
    }

    #[tokio::test]
    async fn emails_are_unique_case_insensitively() {
        let s = store().await;
        s.insert_account(new("Lea@Studio.fr", Some("lea"), "technician"))
            .await
            .unwrap();
        let e = s
            .insert_account(new("lea@studio.FR", None, "technician"))
            .await
            .unwrap_err();
        assert!(matches!(e, StoreError::Conflict("email")), "{e:?}");
        let e = s
            .insert_account(new("autre@studio.fr", Some("lea"), "technician"))
            .await
            .unwrap_err();
        assert!(matches!(e, StoreError::Conflict("username")), "{e:?}");
    }

    #[tokio::test]
    async fn login_by_email_or_username() {
        let s = store().await;
        s.insert_account(new("lea@studio.fr", Some("lea.m"), "technician"))
            .await
            .unwrap();
        assert!(s.credentials("LEA@studio.fr").await.unwrap().is_some());
        assert!(s.credentials("Lea.M").await.unwrap().is_some());
        assert!(s.credentials("personne").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn deleting_a_role_moves_members_to_technician() {
        let s = store().await;
        let r = s
            .insert_role("Régie son", "#123456", &Permissions::all(Level::Lecture))
            .await
            .unwrap();
        let a = s.insert_account(new("x@y.fr", None, &r.id)).await.unwrap();
        assert_eq!(s.role(&r.id).await.unwrap().unwrap().active_members, 1);
        assert_eq!(s.delete_role(&r.id).await.unwrap(), 1);
        assert_eq!(
            s.account(&a.id).await.unwrap().unwrap().role_id,
            "technician"
        );
    }

    #[tokio::test]
    async fn sessions_expire_and_close() {
        let s = store().await;
        let a = s
            .insert_account(new("x@y.fr", None, "technician"))
            .await
            .unwrap();
        s.insert_session("h1", &a.id, Utc::now() + chrono::Duration::days(30), None)
            .await
            .unwrap();
        s.insert_session("h2", &a.id, Utc::now() - chrono::Duration::seconds(1), None)
            .await
            .unwrap();
        assert!(s.session_account("h1").await.unwrap().is_some());
        assert!(s.session_account("h2").await.unwrap().is_none());
        s.insert_session("h3", &a.id, Utc::now() + chrono::Duration::days(1), None)
            .await
            .unwrap();
        s.delete_sessions(&a.id, Some("h3")).await.unwrap();
        assert!(s.session_account("h1").await.unwrap().is_none());
        assert!(s.session_account("h3").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn audit_keeps_before_and_after() {
        let s = store().await;
        s.audit(
            Some("a1"),
            "update",
            "account",
            "x",
            Some(&serde_json::json!({"active": true})),
            Some(&serde_json::json!({"active": false})),
        )
        .await
        .unwrap();
        let e = &s.audit_entries(10).await.unwrap()[0];
        assert_eq!(e.before.as_ref().unwrap()["active"], true);
        assert_eq!(e.after.as_ref().unwrap()["active"], false);
    }

    /// Retour arrière : une version plus ancienne rouvre une base migrée par une plus récente.
    #[tokio::test]
    async fn newer_database_reopens() {
        let dir = std::env::temp_dir().join(format!("strata-test-{}", new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = Store::open(&dir.join("strata.db")).await.unwrap();
        sqlx::query("INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) VALUES (999, 'futur', CURRENT_TIMESTAMP, 1, x'00', 0)")
            .execute(&s.pool)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE futur (id INTEGER)")
            .execute(&s.pool)
            .await
            .unwrap();
        s.close().await;
        let s = Store::open(&dir.join("strata.db")).await.unwrap();
        assert_eq!(s.schema_version().await.unwrap(), 999);
        assert!(s.integrity_ok().await.unwrap());
        s.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn file_database_is_sound() {
        let dir = std::env::temp_dir().join(format!("strata-test-{}", new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = Store::open(&dir.join("strata.db")).await.unwrap();
        assert!(s.integrity_ok().await.unwrap());
        assert_eq!(s.schema_version().await.unwrap(), 3);
        s.close().await;
        // Rouvrir ne rejoue pas les migrations.
        let s = Store::open(&dir.join("strata.db")).await.unwrap();
        assert_eq!(s.list_roles().await.unwrap().len(), 4);
        s.close().await;
        std::fs::remove_dir_all(dir).ok();
    }

    /// Règle des migrations additives (architecture, « Données et stockage ») :
    /// aucune migration ne renomme ni ne supprime.
    #[test]
    fn migrations_are_additive() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let sql = std::fs::read_to_string(&path).unwrap();
            let code: String = sql
                .lines()
                .map(|l| l.split("--").next().unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n")
                .to_uppercase();
            for forbidden in ["DROP ", "RENAME ", "DELETE FROM ", "ALTER TABLE"] {
                // ALTER TABLE n'est permis que pour ADD COLUMN.
                if forbidden == "ALTER TABLE" {
                    for stmt in code.split(';') {
                        if stmt.contains("ALTER TABLE") {
                            assert!(
                                stmt.contains("ADD COLUMN"),
                                "{}: ALTER TABLE sans ADD COLUMN",
                                path.display()
                            );
                        }
                    }
                } else {
                    assert!(
                        !code.contains(forbidden),
                        "{} contient {forbidden}",
                        path.display()
                    );
                }
            }
        }
    }
}
