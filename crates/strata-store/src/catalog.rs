//! Catalogue : référentiels, fiches, sessions d'import et instantanés.
//!
//! Le stockage ne juge pas le contenu : l'API valide avec `strata_domain::catalog`
//! puis écrit ici. Chaque écriture vérifie la révision attendue.

use crate::{Result, Store, StoreError, new_id, now};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite, Transaction};
use strata_domain::catalog::defaults::{DefaultRef, defaults, dimension_defaults};
use strata_domain::catalog::{ItemKind, RefKind, item_summary};
use strata_domain::text::normalize;

const META_SEEDED: &str = "catalog_seeded";
const META_D_FORMAT: &str = "catalog_d_format";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RefItem {
    pub kind: RefKind,
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub data: Value,
    pub active: bool,
    pub position: i64,
    pub builtin: bool,
    pub protected: bool,
    pub rev: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl RefItem {
    fn from_row(r: &SqliteRow) -> Option<RefItem> {
        Some(RefItem {
            kind: RefKind::from_path(r.get::<&str, _>("kind"))?,
            id: r.get("id"),
            name: r.get("name"),
            parent_id: r.get("parent_id"),
            data: serde_json::from_str(r.get::<&str, _>("data")).unwrap_or(Value::Null),
            active: r.get::<i64, _>("active") != 0,
            position: r.get("position"),
            builtin: r.get::<i64, _>("builtin") != 0,
            protected: r.get::<i64, _>("protected") != 0,
            rev: r.get("rev"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
    }
}

/// Fiche du catalogue. `name`, `reference`, `brand_id` et `active` recopient les
/// données pour le tri et la recherche.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogItem {
    pub id: String,
    pub kind: ItemKind,
    pub name: String,
    pub reference: Option<String>,
    pub brand_id: Option<String>,
    pub data: Value,
    pub active: bool,
    pub rev: i64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub created_by: Option<String>,
    #[serde(default)]
    pub updated_by: Option<String>,
}

impl CatalogItem {
    fn from_row(r: &SqliteRow) -> Option<CatalogItem> {
        Some(CatalogItem {
            id: r.get("id"),
            kind: ItemKind::from_path(r.get::<&str, _>("kind"))?,
            name: r.get("name"),
            reference: r.get("reference"),
            brand_id: r.get("brand_id"),
            data: serde_json::from_str(r.get::<&str, _>("data")).unwrap_or(Value::Null),
            active: r.get::<i64, _>("active") != 0,
            rev: r.get("rev"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            created_by: r.get("created_by"),
            updated_by: r.get("updated_by"),
        })
    }
}

#[derive(Debug, Clone)]
pub struct NewRef {
    pub kind: RefKind,
    pub id: Option<String>,
    pub name: String,
    pub parent_id: Option<String>,
    pub data: Value,
    pub active: bool,
    pub position: Option<i64>,
    pub builtin: bool,
    pub protected: bool,
}

impl From<DefaultRef> for NewRef {
    fn from(d: DefaultRef) -> NewRef {
        NewRef {
            kind: d.kind,
            id: Some(d.id),
            name: d.name,
            parent_id: d.parent_id,
            data: d.data,
            active: true,
            position: None,
            builtin: d.builtin,
            protected: d.protected,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ItemFilter {
    pub kind: Option<ItemKind>,
    pub q: Option<String>,
    pub active: Option<bool>,
    pub brand_id: Option<String>,
    /// Rôle modulaire : « host » (châssis) ou « component » (module).
    pub modular: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportSession {
    pub id: String,
    pub account_id: String,
    pub filename: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub id: String,
    pub created_at: String,
    pub actor_id: Option<String>,
    pub reason: String,
    pub sections: Vec<String>,
}

/// Contenu d'un instantané : les référentiels et fiches des sections concernées.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotData {
    pub ref_kinds: Vec<RefKind>,
    pub item_kinds: Vec<ItemKind>,
    pub refs: Vec<RefItem>,
    pub items: Vec<CatalogItem>,
}

/// Changements appliqués d'un bloc, dans une transaction.
#[derive(Debug, Clone, Default)]
pub struct CatalogChanges {
    /// Genres de référentiels vidés avant l'écriture.
    pub clear_refs: Vec<RefKind>,
    /// Genres de fiches vidés avant l'écriture.
    pub clear_items: Vec<ItemKind>,
    /// Écrits tels quels, révision comprise (remplacent l'existant de même identifiant).
    pub put_refs: Vec<RefItem>,
    pub put_items: Vec<CatalogItem>,
}

fn opt_norm(s: &Option<String>) -> Option<String> {
    s.as_deref().map(normalize).filter(|s| !s.is_empty())
}

fn item_columns(data: &Value) -> (String, Option<String>, Option<String>, bool) {
    let (name, reference, brand) = item_summary(data);
    let active = data.get("active").and_then(Value::as_bool).unwrap_or(true);
    (name, reference, brand, active)
}

async fn put_ref(tx: &mut Transaction<'_, Sqlite>, r: &RefItem) -> Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO ref_items (kind, id, name, name_norm, parent_id, data, active, position, builtin, protected, rev, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(r.kind.path())
    .bind(&r.id)
    .bind(&r.name)
    .bind(normalize(&r.name))
    .bind(&r.parent_id)
    .bind(r.data.to_string())
    .bind(r.active as i64)
    .bind(r.position)
    .bind(r.builtin as i64)
    .bind(r.protected as i64)
    .bind(r.rev)
    .bind(&r.created_at)
    .bind(&r.updated_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn put_item(tx: &mut Transaction<'_, Sqlite>, i: &CatalogItem) -> Result<()> {
    let (name, reference, brand, active) = item_columns(&i.data);
    sqlx::query(
        "INSERT OR REPLACE INTO catalog_items (id, kind, name, name_norm, reference, reference_norm, brand_id, data, active, rev, created_at, updated_at, created_by, updated_by) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&i.id)
    .bind(i.kind.path())
    .bind(&name)
    .bind(normalize(&name))
    .bind(&reference)
    .bind(opt_norm(&reference))
    .bind(&brand)
    .bind(i.data.to_string())
    .bind(active as i64)
    .bind(i.rev)
    .bind(&i.created_at)
    .bind(&i.updated_at)
    .bind(&i.created_by)
    .bind(&i.updated_by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

impl Store {
    /// Pose le pack de départ une seule fois. Un site qui a supprimé un élément
    /// du pack ne le voit pas revenir.
    pub(crate) async fn seed_catalog(&self) -> Result<()> {
        if self.meta(META_SEEDED).await?.is_some() {
            return self.backfill_d_format().await;
        }
        let mut tx = self.pool.begin().await?;
        let mut position = std::collections::HashMap::new();
        for d in defaults() {
            let p = position.entry(d.kind).or_insert(0i64);
            *p += 1;
            insert_ref_tx(
                &mut tx,
                NewRef {
                    position: Some(*p),
                    ..d.into()
                },
                true,
            )
            .await?;
        }
        sqlx::query("INSERT INTO meta (key, value) VALUES (?, ?)")
            .bind(META_SEEDED)
            .bind(now())
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO meta (key, value) VALUES (?, ?)")
            .bind(META_D_FORMAT)
            .bind(now())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Une base créée avant les panels (schéma 5) ne sait pas quels connecteurs du pack
    /// existent en format D : on le complète une fois, sans toucher aux autres champs.
    async fn backfill_d_format(&self) -> Result<()> {
        if self.meta(META_D_FORMAT).await?.is_some() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for d in defaults()
            .into_iter()
            .filter(|d| d.kind == RefKind::Connector)
        {
            let on = d
                .data
                .get("d_format")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            sqlx::query("UPDATE ref_items SET data = json_set(data, '$.d_format', json(?)) WHERE kind = ? AND id = ? AND json_extract(data, '$.d_format') IS NULL")
                .bind(if on { "true" } else { "false" })
                .bind(RefKind::Connector.path())
                .bind(&d.id)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("INSERT INTO meta (key, value) VALUES (?, ?)")
            .bind(META_D_FORMAT)
            .bind(now())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    // ----- Référentiels -----

    pub async fn list_refs(&self, kind: Option<RefKind>) -> Result<Vec<RefItem>> {
        let rows = match kind {
            Some(k) => {
                sqlx::query("SELECT * FROM ref_items WHERE kind = ? ORDER BY position, name_norm")
                    .bind(k.path())
                    .fetch_all(&self.pool)
                    .await?
            }
            None => {
                sqlx::query("SELECT * FROM ref_items ORDER BY kind, position, name_norm")
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        Ok(rows.iter().filter_map(RefItem::from_row).collect())
    }

    pub async fn ref_item(&self, kind: RefKind, id: &str) -> Result<Option<RefItem>> {
        let row = sqlx::query("SELECT * FROM ref_items WHERE kind = ? AND id = ?")
            .bind(kind.path())
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().and_then(RefItem::from_row))
    }

    pub async fn insert_ref(&self, n: NewRef) -> Result<RefItem> {
        let mut tx = self.pool.begin().await?;
        let (kind, id) = insert_ref_tx(&mut tx, n, false).await?;
        tx.commit().await?;
        self.ref_item(kind, &id).await?.ok_or(StoreError::NotFound)
    }

    /// Modifie un référentiel si sa révision est encore `rev`.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_ref(
        &self,
        kind: RefKind,
        id: &str,
        rev: i64,
        name: &str,
        parent_id: Option<&str>,
        data: &Value,
        active: bool,
        position: i64,
    ) -> Result<RefItem> {
        let done = sqlx::query(
            "UPDATE ref_items SET name = ?, name_norm = ?, parent_id = ?, data = ?, active = ?, position = ?, rev = rev + 1, updated_at = ? \
             WHERE kind = ? AND id = ? AND rev = ?",
        )
        .bind(name)
        .bind(normalize(name))
        .bind(parent_id)
        .bind(data.to_string())
        .bind(active as i64)
        .bind(position)
        .bind(now())
        .bind(kind.path())
        .bind(id)
        .bind(rev)
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(match self.ref_item(kind, id).await? {
                Some(_) => StoreError::Stale,
                None => StoreError::NotFound,
            });
        }
        self.ref_item(kind, id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn delete_ref(&self, kind: RefKind, id: &str) -> Result<()> {
        let done = sqlx::query("DELETE FROM ref_items WHERE kind = ? AND id = ? AND protected = 0")
            .bind(kind.path())
            .bind(id)
            .execute(&self.pool)
            .await?;
        if done.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Remet les préréglages de dimensions d'origine qui manquent ; renvoie leur nombre.
    pub async fn restore_dimension_defaults(&self) -> Result<u64> {
        let existing = self.list_refs(Some(RefKind::Dimension)).await?;
        let same = |a: &Value, b: &Value| {
            a.get("kind") == b.get("kind")
                && a.get("value").and_then(Value::as_f64) == b.get("value").and_then(Value::as_f64)
        };
        let mut tx = self.pool.begin().await?;
        let mut n = 0;
        for d in dimension_defaults() {
            if existing
                .iter()
                .any(|e| e.id == d.id || same(&e.data, &d.data))
            {
                continue;
            }
            insert_ref_tx(&mut tx, d.into(), false).await?;
            n += 1;
        }
        tx.commit().await?;
        Ok(n)
    }

    // ----- Fiches -----

    pub async fn list_items(&self, f: &ItemFilter) -> Result<(Vec<CatalogItem>, i64)> {
        let mut clause = String::from(" FROM catalog_items WHERE 1 = 1");
        let mut binds: Vec<String> = Vec::new();
        if let Some(k) = f.kind {
            clause.push_str(" AND kind = ?");
            binds.push(k.path().into());
        }
        if let Some(q) = f.q.as_deref().map(normalize).filter(|q| !q.is_empty()) {
            let like = format!(
                "%{}%",
                q.replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            );
            clause.push_str(
                " AND (name_norm LIKE ? ESCAPE '\\' OR reference_norm LIKE ? ESCAPE '\\')",
            );
            binds.push(like.clone());
            binds.push(like);
        }
        if let Some(a) = f.active {
            clause.push_str(if a {
                " AND active = 1"
            } else {
                " AND active = 0"
            });
        }
        if let Some(b) = &f.brand_id {
            clause.push_str(" AND brand_id = ?");
            binds.push(b.clone());
        }
        if let Some(m) = &f.modular {
            clause.push_str(" AND json_extract(data, '$.modular.role') = ?");
            binds.push(m.clone());
        }
        let count_sql = format!("SELECT COUNT(*){clause}");
        let mut count = sqlx::query_scalar::<_, i64>(&count_sql);
        for b in &binds {
            count = count.bind(b.clone());
        }
        let total = count.fetch_one(&self.pool).await?;
        let sql = format!("SELECT *{clause} ORDER BY name_norm, id LIMIT ? OFFSET ?");
        let mut q = sqlx::query(&sql);
        for b in &binds {
            q = q.bind(b.clone());
        }
        let rows = q
            .bind(f.limit.clamp(1, 1000))
            .bind(f.offset.max(0))
            .fetch_all(&self.pool)
            .await?;
        Ok((
            rows.iter().filter_map(CatalogItem::from_row).collect(),
            total,
        ))
    }

    /// Toutes les fiches d'un genre, ou de tous les genres.
    pub async fn all_items(&self, kind: Option<ItemKind>) -> Result<Vec<CatalogItem>> {
        let rows = match kind {
            Some(k) => {
                sqlx::query("SELECT * FROM catalog_items WHERE kind = ? ORDER BY name_norm, id")
                    .bind(k.path())
                    .fetch_all(&self.pool)
                    .await?
            }
            None => {
                sqlx::query("SELECT * FROM catalog_items ORDER BY kind, name_norm, id")
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        Ok(rows.iter().filter_map(CatalogItem::from_row).collect())
    }

    pub async fn item(&self, id: &str) -> Result<Option<CatalogItem>> {
        let row = sqlx::query("SELECT * FROM catalog_items WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().and_then(CatalogItem::from_row))
    }

    pub async fn insert_item(
        &self,
        kind: ItemKind,
        data: &Value,
        actor: Option<&str>,
    ) -> Result<CatalogItem> {
        let t = now();
        let item = CatalogItem {
            id: new_id(),
            kind,
            name: String::new(),
            reference: None,
            brand_id: None,
            data: data.clone(),
            active: true,
            rev: 1,
            created_at: t.clone(),
            updated_at: t,
            created_by: actor.map(str::to_string),
            updated_by: actor.map(str::to_string),
        };
        let mut tx = self.pool.begin().await?;
        put_item(&mut tx, &item).await?;
        tx.commit().await?;
        self.item(&item.id).await?.ok_or(StoreError::NotFound)
    }

    /// Remplace les données d'une fiche si sa révision est encore `rev`.
    pub async fn update_item(
        &self,
        id: &str,
        rev: i64,
        data: &Value,
        actor: Option<&str>,
    ) -> Result<CatalogItem> {
        let (name, reference, brand, active) = item_columns(data);
        let done = sqlx::query(
            "UPDATE catalog_items SET name = ?, name_norm = ?, reference = ?, reference_norm = ?, brand_id = ?, data = ?, active = ?, \
             rev = rev + 1, updated_at = ?, updated_by = ? WHERE id = ? AND rev = ?",
        )
        .bind(&name)
        .bind(normalize(&name))
        .bind(&reference)
        .bind(opt_norm(&reference))
        .bind(&brand)
        .bind(data.to_string())
        .bind(active as i64)
        .bind(now())
        .bind(actor)
        .bind(id)
        .bind(rev)
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(match self.item(id).await? {
                Some(_) => StoreError::Stale,
                None => StoreError::NotFound,
            });
        }
        self.item(id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn delete_items(&self, ids: &[String]) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        let mut n = 0;
        for id in ids {
            n += sqlx::query("DELETE FROM catalog_items WHERE id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        }
        tx.commit().await?;
        Ok(n)
    }

    /// Applique un lot de changements d'un seul bloc : tout ou rien.
    pub async fn apply_catalog(&self, c: &CatalogChanges) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for k in &c.clear_refs {
            sqlx::query("DELETE FROM ref_items WHERE kind = ? AND protected = 0")
                .bind(k.path())
                .execute(&mut *tx)
                .await?;
        }
        for k in &c.clear_items {
            sqlx::query("DELETE FROM catalog_items WHERE kind = ?")
                .bind(k.path())
                .execute(&mut *tx)
                .await?;
        }
        for r in &c.put_refs {
            put_ref(&mut tx, r).await?;
        }
        for i in &c.put_items {
            put_item(&mut tx, i).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    // ----- Sessions d'import -----

    pub async fn insert_import_session(
        &self,
        account_id: &str,
        filename: &str,
        headers: &[String],
        rows: &[Vec<String>],
    ) -> Result<ImportSession> {
        sqlx::query("DELETE FROM import_sessions WHERE expires_at < ?")
            .bind(now())
            .execute(&self.pool)
            .await?;
        let id = new_id();
        let created = now();
        let expires = crate::timestamp(chrono::Utc::now() + chrono::Duration::hours(24));
        sqlx::query("INSERT INTO import_sessions (id, account_id, filename, headers, rows, created_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(account_id)
            .bind(filename)
            .bind(serde_json::to_string(headers).unwrap_or_default())
            .bind(serde_json::to_string(rows).unwrap_or_default())
            .bind(&created)
            .bind(&expires)
            .execute(&self.pool)
            .await?;
        Ok(ImportSession {
            id,
            account_id: account_id.into(),
            filename: filename.into(),
            headers: headers.to_vec(),
            rows: rows.to_vec(),
            created_at: created,
            expires_at: expires,
        })
    }

    /// Session d'import encore valable, ouverte par ce compte.
    pub async fn import_session(
        &self,
        id: &str,
        account_id: &str,
    ) -> Result<Option<ImportSession>> {
        let row = sqlx::query(
            "SELECT * FROM import_sessions WHERE id = ? AND account_id = ? AND expires_at >= ?",
        )
        .bind(id)
        .bind(account_id)
        .bind(now())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| ImportSession {
            id: r.get("id"),
            account_id: r.get("account_id"),
            filename: r.get("filename"),
            headers: serde_json::from_str(r.get::<&str, _>("headers")).unwrap_or_default(),
            rows: serde_json::from_str(r.get::<&str, _>("rows")).unwrap_or_default(),
            created_at: r.get("created_at"),
            expires_at: r.get("expires_at"),
        }))
    }

    pub async fn delete_import_session(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM import_sessions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ----- Instantanés -----

    /// Photographie les sections nommées ; garde les 20 plus récents.
    pub async fn snapshot_catalog(
        &self,
        actor: Option<&str>,
        reason: &str,
        ref_kinds: &[RefKind],
        item_kinds: &[ItemKind],
    ) -> Result<SnapshotInfo> {
        let mut data = SnapshotData {
            ref_kinds: ref_kinds.to_vec(),
            item_kinds: item_kinds.to_vec(),
            ..Default::default()
        };
        for k in ref_kinds {
            data.refs.extend(self.list_refs(Some(*k)).await?);
        }
        for k in item_kinds {
            data.items.extend(self.all_items(Some(*k)).await?);
        }
        let sections: Vec<String> = ref_kinds
            .iter()
            .map(|k| format!("refs/{}", k.path()))
            .chain(item_kinds.iter().map(|k| k.path().to_string()))
            .collect();
        let info = SnapshotInfo {
            id: new_id(),
            created_at: now(),
            actor_id: actor.map(str::to_string),
            reason: reason.to_string(),
            sections,
        };
        sqlx::query("INSERT INTO catalog_snapshots (id, created_at, actor_id, reason, sections, data) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&info.id)
            .bind(&info.created_at)
            .bind(&info.actor_id)
            .bind(&info.reason)
            .bind(serde_json::to_string(&info.sections).unwrap_or_default())
            .bind(serde_json::to_string(&data).unwrap_or_default())
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM catalog_snapshots WHERE id NOT IN (SELECT id FROM catalog_snapshots ORDER BY created_at DESC, id DESC LIMIT 20)")
            .execute(&self.pool)
            .await?;
        Ok(info)
    }

    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotInfo>> {
        let rows = sqlx::query("SELECT id, created_at, actor_id, reason, sections FROM catalog_snapshots ORDER BY created_at DESC, id DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|r| SnapshotInfo {
                id: r.get("id"),
                created_at: r.get("created_at"),
                actor_id: r.get("actor_id"),
                reason: r.get("reason"),
                sections: serde_json::from_str(r.get::<&str, _>("sections")).unwrap_or_default(),
            })
            .collect())
    }

    pub async fn snapshot_data(&self, id: &str) -> Result<Option<SnapshotData>> {
        let s: Option<String> =
            sqlx::query_scalar("SELECT data FROM catalog_snapshots WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(s.and_then(|s| serde_json::from_str(&s).ok()))
    }
}

/// Insère un référentiel. `ignore` : un identifiant déjà pris est laissé tel quel.
async fn insert_ref_tx(
    tx: &mut Transaction<'_, Sqlite>,
    n: NewRef,
    ignore: bool,
) -> Result<(RefKind, String)> {
    let id = n.id.unwrap_or_else(new_id);
    let position = match n.position {
        Some(p) => p,
        None => {
            sqlx::query_scalar::<_, i64>(
                "SELECT COALESCE(MAX(position), 0) + 1 FROM ref_items WHERE kind = ?",
            )
            .bind(n.kind.path())
            .fetch_one(&mut **tx)
            .await?
        }
    };
    let t = now();
    let verb = if ignore { "INSERT OR IGNORE" } else { "INSERT" };
    sqlx::query(&format!(
        "{verb} INTO ref_items (kind, id, name, name_norm, parent_id, data, active, position, builtin, protected, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    ))
    .bind(n.kind.path())
    .bind(&id)
    .bind(n.name.trim())
    .bind(normalize(&n.name))
    .bind(&n.parent_id)
    .bind(n.data.to_string())
    .bind(n.active as i64)
    .bind(position)
    .bind(n.builtin as i64)
    .bind(n.protected as i64)
    .bind(&t)
    .bind(&t)
    .execute(&mut **tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => StoreError::Conflict("ref_id"),
        _ => StoreError::Db(e),
    })?;
    Ok((n.kind, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn defaults_are_seeded_once() {
        let s = Store::open_in_memory().await.unwrap();
        let functions = s.list_refs(Some(RefKind::Function)).await.unwrap();
        assert_eq!(functions.len(), 11);
        assert_eq!(functions[0].position, 1);
        s.delete_ref(
            RefKind::Phase,
            &s.list_refs(Some(RefKind::Phase)).await.unwrap()[0].id,
        )
        .await
        .unwrap();
        s.seed_catalog().await.unwrap();
        assert_eq!(
            s.list_refs(Some(RefKind::Phase)).await.unwrap().len(),
            9,
            "pas de retour du pack"
        );
        // Les catégories système sont protégées.
        let sys = &s.list_refs(Some(RefKind::SystemCategory)).await.unwrap()[0];
        assert!(matches!(
            s.delete_ref(RefKind::SystemCategory, &sys.id).await,
            Err(StoreError::NotFound)
        ));
    }

    #[tokio::test]
    async fn items_check_revision_and_search_without_accents() {
        let s = Store::open_in_memory().await.unwrap();
        let it = s
            .insert_item(
                ItemKind::CableModel,
                &json!({"name": "Câble Belden 1694A", "reference": "1694A"}),
                None,
            )
            .await
            .unwrap();
        assert_eq!(it.name, "Câble Belden 1694A");
        let up = s
            .update_item(
                &it.id,
                1,
                &json!({"name": "Câble Belden 1694A", "active": false}),
                Some("a"),
            )
            .await
            .unwrap();
        assert_eq!(up.rev, 2);
        assert!(!up.active);
        assert!(matches!(
            s.update_item(&it.id, 1, &json!({"name": "x"}), None).await,
            Err(StoreError::Stale)
        ));
        let f = ItemFilter {
            q: Some("CABLE".into()),
            limit: 50,
            ..Default::default()
        };
        let (found, total) = s.list_items(&f).await.unwrap();
        assert_eq!((found.len(), total), (1, 1));
        let f = ItemFilter {
            q: Some("1694".into()),
            active: Some(true),
            limit: 50,
            ..Default::default()
        };
        assert_eq!(s.list_items(&f).await.unwrap().1, 0);
    }

    #[tokio::test]
    async fn snapshot_and_restore_a_section() {
        let s = Store::open_in_memory().await.unwrap();
        s.insert_item(ItemKind::Adapter, &json!({"name": "BNC vers HD-BNC"}), None)
            .await
            .unwrap();
        let snap = s
            .snapshot_catalog(None, "test", &[RefKind::Phase], &[ItemKind::Adapter])
            .await
            .unwrap();
        s.apply_catalog(&CatalogChanges {
            clear_refs: vec![RefKind::Phase],
            clear_items: vec![ItemKind::Adapter],
            ..Default::default()
        })
        .await
        .unwrap();
        assert!(s.list_refs(Some(RefKind::Phase)).await.unwrap().is_empty());
        let data = s.snapshot_data(&snap.id).await.unwrap().unwrap();
        s.apply_catalog(&CatalogChanges {
            clear_refs: data.ref_kinds,
            clear_items: data.item_kinds,
            put_refs: data.refs,
            put_items: data.items,
        })
        .await
        .unwrap();
        assert_eq!(s.list_refs(Some(RefKind::Phase)).await.unwrap().len(), 10);
        assert_eq!(s.all_items(Some(ItemKind::Adapter)).await.unwrap().len(), 1);
        assert_eq!(
            s.list_snapshots().await.unwrap()[0].sections,
            ["refs/phases", "adapters"]
        );
    }

    #[tokio::test]
    async fn dimension_defaults_come_back() {
        let s = Store::open_in_memory().await.unwrap();
        let dims = s.list_refs(Some(RefKind::Dimension)).await.unwrap();
        s.delete_ref(RefKind::Dimension, &dims[0].id).await.unwrap();
        assert_eq!(s.restore_dimension_defaults().await.unwrap(), 1);
        assert_eq!(s.restore_dimension_defaults().await.unwrap(), 0);
    }
}
