//! Projets et inventaire : fiches, emplacements, contenants, articles, exemplaires, tâches.
//!
//! Les règles sont dans `strata_domain::project` ; ici, chaque écriture vérifie la
//! révision attendue et garde les exemplaires en accord avec les quantités.

use crate::{Result, Store, StoreError, map_unique, new_id, now};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite, Transaction};
use strata_domain::project::{
    Accessory, ArticleNature, ContainerData, ProjectData, TaskData, tag_from,
};
use strata_domain::text::normalize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Project {
    pub id: String,
    #[serde(flatten)]
    pub data: ProjectData,
    pub rev: i64,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Location {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub code: String,
    pub position: i64,
    pub rev: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Container {
    pub id: String,
    pub project_id: String,
    pub location_id: String,
    pub name: String,
    pub position: i64,
    #[serde(flatten)]
    pub data: ContainerData,
    pub rev: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Article {
    pub id: String,
    pub project_id: String,
    pub location_id: String,
    pub container_id: Option<String>,
    pub position: i64,
    pub nature: ArticleNature,
    pub item_id: Option<String>,
    pub quantity: i64,
    pub accessory: Option<Accessory>,
    /// Module glissé dans un châssis du projet : l'article hôte et le premier emplacement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<ModuleHost>,
    pub rev: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleHost {
    pub article_id: String,
    pub slot: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Instance {
    pub id: String,
    pub project_id: String,
    pub article_id: String,
    pub tag: String,
    pub serial: Option<String>,
    pub mac: Option<String>,
    pub inventory_tag: Option<String>,
    pub rev: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub position: i64,
    #[serde(flatten)]
    pub data: TaskData,
    pub rev: i64,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn json<T: serde::de::DeserializeOwned + Default>(r: &SqliteRow, col: &str) -> T {
    serde_json::from_str(r.get::<&str, _>(col)).unwrap_or_default()
}

fn enum_str<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

impl Project {
    fn from_row(r: &SqliteRow) -> Project {
        Project {
            id: r.get("id"),
            data: json(r, "data"),
            rev: r.get("rev"),
            created_by: r.get("created_by"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }
    }
}
impl Location {
    fn from_row(r: &SqliteRow) -> Location {
        Location {
            id: r.get("id"),
            project_id: r.get("project_id"),
            name: r.get("name"),
            code: r.get("code"),
            position: r.get("position"),
            rev: r.get("rev"),
        }
    }
}
impl Container {
    fn from_row(r: &SqliteRow) -> Container {
        Container {
            id: r.get("id"),
            project_id: r.get("project_id"),
            location_id: r.get("location_id"),
            name: r.get("name"),
            position: r.get("position"),
            data: json(r, "data"),
            rev: r.get("rev"),
        }
    }
}
impl Article {
    fn from_row(r: &SqliteRow) -> Article {
        let data: Value = json(r, "data");
        Article {
            id: r.get("id"),
            project_id: r.get("project_id"),
            location_id: r.get("location_id"),
            container_id: r.get("container_id"),
            position: r.get("position"),
            nature: serde_json::from_value(Value::String(r.get("nature")))
                .unwrap_or(ArticleNature::Equipment),
            item_id: r.get("item_id"),
            quantity: r.get("quantity"),
            accessory: data
                .get("accessory")
                .and_then(|a| serde_json::from_value(a.clone()).ok()),
            host: data
                .get("host")
                .and_then(|h| serde_json::from_value(h.clone()).ok()),
            rev: r.get("rev"),
        }
    }
}
impl Instance {
    fn from_row(r: &SqliteRow) -> Instance {
        Instance {
            id: r.get("id"),
            project_id: r.get("project_id"),
            article_id: r.get("article_id"),
            tag: r.get("tag"),
            serial: r.get("serial"),
            mac: r.get("mac"),
            inventory_tag: r.get("inventory_tag"),
            rev: r.get("rev"),
        }
    }
}
impl Task {
    fn from_row(r: &SqliteRow) -> Task {
        Task {
            id: r.get("id"),
            project_id: r.get("project_id"),
            position: r.get("position"),
            data: json(r, "data"),
            rev: r.get("rev"),
            created_by: r.get("created_by"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }
    }
}

/// Tout l'inventaire d'un projet, d'une seule lecture.
#[derive(Debug, Clone, Serialize)]
pub struct Inventory {
    pub locations: Vec<Location>,
    pub containers: Vec<Container>,
    pub articles: Vec<Article>,
    pub instances: Vec<Instance>,
}

/// Résultat d'une modification attendue sur une ligne : absente, ou révision dépassée.
async fn check_rev(
    tx: &mut Transaction<'_, Sqlite>,
    table: &str,
    id: &str,
    rev: i64,
) -> Result<()> {
    let cur: Option<i64> = sqlx::query_scalar(&format!("SELECT rev FROM {table} WHERE id = ?"))
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
    match cur {
        None => Err(StoreError::NotFound),
        Some(r) if r != rev => Err(StoreError::Stale),
        Some(_) => Ok(()),
    }
}

/// Repères « #XXXX » : la suite des numéros passe par une bijection de 0..2^20 pour
/// paraître aléatoire, sans jamais se répéter dans un projet.
fn scrambled_tag(n: i64) -> String {
    let n = n as u64 & 0xF_FFFF;
    tag_from((n.wrapping_mul(0x9E3B5) + 0x5A3C1) & 0xF_FFFF)
}

async fn add_instances(
    tx: &mut Transaction<'_, Sqlite>,
    project_id: &str,
    article_id: &str,
    count: i64,
) -> Result<()> {
    if count <= 0 {
        return Ok(());
    }
    let seq: i64 = sqlx::query_scalar(
        "UPDATE projects SET tag_seq = tag_seq + ? WHERE id = ? RETURNING tag_seq",
    )
    .bind(count)
    .bind(project_id)
    .fetch_one(&mut **tx)
    .await?;
    let t = now();
    for n in (seq - count)..seq {
        sqlx::query("INSERT INTO instances (id, project_id, article_id, tag, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(new_id())
            .bind(project_id)
            .bind(article_id)
            .bind(scrambled_tag(n))
            .bind(&t)
            .bind(&t)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

async fn touch_project(tx: &mut Transaction<'_, Sqlite>, project_id: &str) -> Result<()> {
    sqlx::query("UPDATE projects SET updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(project_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

impl Store {
    // ----- Projets -----

    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        let rows = sqlx::query("SELECT * FROM projects ORDER BY updated_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(Project::from_row).collect())
    }

    pub async fn project(&self, id: &str) -> Result<Option<Project>> {
        Ok(sqlx::query("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(Project::from_row))
    }

    pub async fn project_name_taken(&self, name: &str, except: Option<&str>) -> Result<bool> {
        let n: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM projects WHERE name_norm = ? AND id != ?")
                .bind(normalize(name))
                .bind(except.unwrap_or(""))
                .fetch_one(&self.pool)
                .await?;
        Ok(n > 0)
    }

    pub async fn insert_project(&self, data: &ProjectData, actor: Option<&str>) -> Result<Project> {
        let id = new_id();
        let t = now();
        sqlx::query("INSERT INTO projects (id, name, name_norm, status, intervention, data, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(&data.name)
            .bind(normalize(&data.name))
            .bind(enum_str(&data.status))
            .bind(enum_str(&data.intervention))
            .bind(serde_json::to_string(data).unwrap_or_default())
            .bind(actor)
            .bind(&t)
            .bind(&t)
            .execute(&self.pool)
            .await
            .map_err(map_unique)?;
        self.project(&id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn update_project(&self, id: &str, rev: i64, data: &ProjectData) -> Result<Project> {
        let done = sqlx::query("UPDATE projects SET name = ?, name_norm = ?, status = ?, data = ?, rev = rev + 1, updated_at = ? WHERE id = ? AND rev = ?")
            .bind(&data.name)
            .bind(normalize(&data.name))
            .bind(enum_str(&data.status))
            .bind(serde_json::to_string(data).unwrap_or_default())
            .bind(now())
            .bind(id)
            .bind(rev)
            .execute(&self.pool)
            .await
            .map_err(map_unique)?;
        if done.rows_affected() == 0 {
            return Err(if self.project(id).await?.is_some() {
                StoreError::Stale
            } else {
                StoreError::NotFound
            });
        }
        self.project(id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn delete_project(&self, id: &str) -> Result<()> {
        let done = sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if done.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Tout l'inventaire d'un projet.
    pub async fn inventory(&self, project_id: &str) -> Result<Inventory> {
        let q = |sql: &'static str| sqlx::query(sql).bind(project_id.to_string());
        let locations =
            q("SELECT * FROM locations WHERE project_id = ? ORDER BY position, created_at")
                .fetch_all(&self.pool)
                .await?;
        let containers =
            q("SELECT * FROM containers WHERE project_id = ? ORDER BY position, created_at")
                .fetch_all(&self.pool)
                .await?;
        let articles =
            q("SELECT * FROM articles WHERE project_id = ? ORDER BY position, created_at")
                .fetch_all(&self.pool)
                .await?;
        let instances = q("SELECT * FROM instances WHERE project_id = ? ORDER BY created_at, tag")
            .fetch_all(&self.pool)
            .await?;
        Ok(Inventory {
            locations: locations.iter().map(Location::from_row).collect(),
            containers: containers.iter().map(Container::from_row).collect(),
            articles: articles.iter().map(Article::from_row).collect(),
            instances: instances.iter().map(Instance::from_row).collect(),
        })
    }

    /// Nombre d'emplacements, contenants et articles de chaque projet (pour la liste).
    pub async fn project_counts(&self) -> Result<Vec<(String, i64, i64, i64)>> {
        let rows = sqlx::query(
            "SELECT p.id, \
             (SELECT COUNT(*) FROM locations l WHERE l.project_id = p.id) AS locs, \
             (SELECT COUNT(*) FROM containers c WHERE c.project_id = p.id) AS conts, \
             (SELECT COUNT(*) FROM articles a WHERE a.project_id = p.id) AS arts \
             FROM projects p",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get("id"), r.get("locs"), r.get("conts"), r.get("arts")))
            .collect())
    }

    // ----- Emplacements -----

    pub async fn location(&self, id: &str) -> Result<Option<Location>> {
        Ok(sqlx::query("SELECT * FROM locations WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(Location::from_row))
    }

    pub async fn insert_location(
        &self,
        project_id: &str,
        name: &str,
        code: &str,
    ) -> Result<Location> {
        let id = new_id();
        let t = now();
        let mut tx = self.pool.begin().await?;
        let pos: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM locations WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO locations (id, project_id, name, code, position, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(project_id)
            .bind(name)
            .bind(code)
            .bind(pos)
            .bind(&t)
            .bind(&t)
            .execute(&mut *tx)
            .await?;
        touch_project(&mut tx, project_id).await?;
        tx.commit().await?;
        self.location(&id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn update_location(
        &self,
        id: &str,
        rev: i64,
        name: &str,
        code: &str,
    ) -> Result<Location> {
        let mut tx = self.pool.begin().await?;
        check_rev(&mut tx, "locations", id, rev).await?;
        let pid: String = sqlx::query_scalar("UPDATE locations SET name = ?, code = ?, rev = rev + 1, updated_at = ? WHERE id = ? RETURNING project_id")
            .bind(name)
            .bind(code)
            .bind(now())
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        touch_project(&mut tx, &pid).await?;
        tx.commit().await?;
        self.location(id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn delete_location(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM locations WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Range les lignes d'une table dans l'ordre donné (les identifiants absents restent après).
    pub async fn reorder(
        &self,
        table: &'static str,
        project_id: &str,
        ids: &[String],
    ) -> Result<()> {
        assert!(matches!(
            table,
            "locations" | "containers" | "tasks" | "articles"
        ));
        let mut tx = self.pool.begin().await?;
        for (i, id) in ids.iter().enumerate() {
            sqlx::query(&format!(
                "UPDATE {table} SET position = ?, rev = rev + 1 WHERE id = ? AND project_id = ?"
            ))
            .bind(i as i64 + 1)
            .bind(id)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        }
        touch_project(&mut tx, project_id).await?;
        tx.commit().await?;
        Ok(())
    }

    // ----- Contenants -----

    pub async fn container(&self, id: &str) -> Result<Option<Container>> {
        Ok(sqlx::query("SELECT * FROM containers WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(Container::from_row))
    }

    pub async fn insert_container(
        &self,
        project_id: &str,
        location_id: &str,
        name: &str,
        data: &ContainerData,
    ) -> Result<Container> {
        let id = new_id();
        let t = now();
        let mut tx = self.pool.begin().await?;
        let pos: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM containers WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO containers (id, project_id, location_id, name, name_norm, position, data, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(project_id)
            .bind(location_id)
            .bind(name)
            .bind(normalize(name))
            .bind(pos)
            .bind(serde_json::to_string(data).unwrap_or_default())
            .bind(&t)
            .bind(&t)
            .execute(&mut *tx)
            .await
            .map_err(map_unique)?;
        crate::regies::attach_to_main(&mut tx, project_id, &id).await?;
        touch_project(&mut tx, project_id).await?;
        tx.commit().await?;
        self.container(&id).await?.ok_or(StoreError::NotFound)
    }

    /// Modifie un contenant ; s'il change d'emplacement, ses articles le suivent.
    pub async fn update_container(
        &self,
        id: &str,
        rev: i64,
        location_id: &str,
        name: &str,
        data: &ContainerData,
    ) -> Result<Container> {
        let mut tx = self.pool.begin().await?;
        check_rev(&mut tx, "containers", id, rev).await?;
        let pid: String = sqlx::query_scalar("UPDATE containers SET location_id = ?, name = ?, name_norm = ?, data = ?, rev = rev + 1, updated_at = ? WHERE id = ? RETURNING project_id")
            .bind(location_id)
            .bind(name)
            .bind(normalize(name))
            .bind(serde_json::to_string(data).unwrap_or_default())
            .bind(now())
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_unique)?;
        sqlx::query("UPDATE articles SET location_id = ? WHERE container_id = ?")
            .bind(location_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        touch_project(&mut tx, &pid).await?;
        tx.commit().await?;
        self.container(id).await?.ok_or(StoreError::NotFound)
    }

    /// Supprime un contenant avec ses articles et leurs exemplaires.
    pub async fn delete_container(&self, id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let pid: Option<String> =
            sqlx::query_scalar("DELETE FROM containers WHERE id = ? RETURNING project_id")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let pid = pid.ok_or(StoreError::NotFound)?;
        touch_project(&mut tx, &pid).await?;
        tx.commit().await?;
        Ok(())
    }

    // ----- Articles -----

    pub async fn article(&self, id: &str) -> Result<Option<Article>> {
        Ok(sqlx::query("SELECT * FROM articles WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(Article::from_row))
    }

    pub async fn article_instances(&self, article_id: &str) -> Result<Vec<Instance>> {
        let rows =
            sqlx::query("SELECT * FROM instances WHERE article_id = ? ORDER BY created_at, tag")
                .bind(article_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.iter().map(Instance::from_row).collect())
    }

    /// Ajoute un article. Pour un équipement ou un adaptateur déjà présent au même endroit,
    /// la quantité augmente au lieu de créer une seconde ligne. Renvoie l'article et s'il est nouveau.
    #[allow(clippy::too_many_arguments)]
    pub async fn add_article(
        &self,
        project_id: &str,
        location_id: &str,
        container_id: Option<&str>,
        nature: ArticleNature,
        item_id: Option<&str>,
        quantity: i64,
        accessory: Option<&Accessory>,
        merge: bool,
    ) -> Result<(Article, bool)> {
        let mut tx = self.pool.begin().await?;
        if merge && item_id.is_some() {
            let existing: Option<(String, i64)> = sqlx::query_as(
                "SELECT id, quantity FROM articles WHERE project_id = ? AND item_id = ? AND nature = ? AND location_id = ? AND container_id IS ?",
            )
            .bind(project_id)
            .bind(item_id)
            .bind(enum_str(&nature))
            .bind(location_id)
            .bind(container_id)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some((id, q)) = existing {
                sqlx::query(
                    "UPDATE articles SET quantity = ?, rev = rev + 1, updated_at = ? WHERE id = ?",
                )
                .bind(q + quantity)
                .bind(now())
                .bind(&id)
                .execute(&mut *tx)
                .await?;
                if nature == ArticleNature::Equipment {
                    add_instances(&mut tx, project_id, &id, quantity).await?;
                }
                touch_project(&mut tx, project_id).await?;
                tx.commit().await?;
                return Ok((self.article(&id).await?.ok_or(StoreError::NotFound)?, false));
            }
        }
        let id = new_id();
        let t = now();
        let pos: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM articles WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        let data = serde_json::json!({ "accessory": accessory });
        sqlx::query("INSERT INTO articles (id, project_id, location_id, container_id, position, nature, item_id, quantity, data, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(project_id)
            .bind(location_id)
            .bind(container_id)
            .bind(pos)
            .bind(enum_str(&nature))
            .bind(item_id)
            .bind(quantity)
            .bind(data.to_string())
            .bind(&t)
            .bind(&t)
            .execute(&mut *tx)
            .await?;
        if nature == ArticleNature::Equipment {
            add_instances(&mut tx, project_id, &id, quantity).await?;
        }
        touch_project(&mut tx, project_id).await?;
        tx.commit().await?;
        Ok((self.article(&id).await?.ok_or(StoreError::NotFound)?, true))
    }

    /// Change la quantité. Les exemplaires suivent : en baisse, les derniers créés partent
    /// (le placement et le câblage, à venir, fixeront un ordre plus fin).
    /// Remplace la description d'un accessoire (panel modifié dans le projet).
    pub async fn set_article_accessory(
        &self,
        id: &str,
        rev: i64,
        accessory: &Accessory,
    ) -> Result<Article> {
        let mut tx = self.pool.begin().await?;
        check_rev(&mut tx, "articles", id, rev).await?;
        let pid: String = sqlx::query_scalar("SELECT project_id FROM articles WHERE id = ?")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query("UPDATE articles SET data = ?, rev = rev + 1, updated_at = ? WHERE id = ?")
            .bind(serde_json::json!({ "accessory": accessory }).to_string())
            .bind(now())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        touch_project(&mut tx, &pid).await?;
        tx.commit().await?;
        self.article(id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn set_article_quantity(&self, id: &str, rev: i64, quantity: i64) -> Result<Article> {
        let mut tx = self.pool.begin().await?;
        check_rev(&mut tx, "articles", id, rev).await?;
        let (pid, nature, old): (String, String, i64) =
            sqlx::query_as("SELECT project_id, nature, quantity FROM articles WHERE id = ?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        sqlx::query("UPDATE articles SET quantity = ?, rev = rev + 1, updated_at = ? WHERE id = ?")
            .bind(quantity)
            .bind(now())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if nature == "equipment" {
            if quantity > old {
                add_instances(&mut tx, &pid, id, quantity - old).await?;
            } else if quantity < old {
                // Les exemplaires non posés partent d'abord, puis les plus récents.
                sqlx::query("DELETE FROM instances WHERE id IN (SELECT i.id FROM instances i WHERE i.article_id = ? ORDER BY EXISTS (SELECT 1 FROM placements p WHERE p.instance_id = i.id), i.created_at DESC, i.tag DESC LIMIT ?)")
                    .bind(id)
                    .bind(old - quantity)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        touch_project(&mut tx, &pid).await?;
        tx.commit().await?;
        self.article(id).await?.ok_or(StoreError::NotFound)
    }

    /// Déplace un article entier vers un autre contenant, ou le rend libre dans un emplacement.
    pub async fn move_article(
        &self,
        id: &str,
        rev: i64,
        location_id: &str,
        container_id: Option<&str>,
    ) -> Result<Article> {
        let mut tx = self.pool.begin().await?;
        check_rev(&mut tx, "articles", id, rev).await?;
        // L'article change de contenant : il n'est plus posé dans l'ancien.
        sqlx::query("DELETE FROM placements WHERE article_id = ? AND container_id IS NOT ?")
            .bind(id)
            .bind(container_id)
            .execute(&mut *tx)
            .await?;
        let pid: String = sqlx::query_scalar("UPDATE articles SET location_id = ?, container_id = ?, rev = rev + 1, updated_at = ? WHERE id = ? RETURNING project_id")
            .bind(location_id)
            .bind(container_id)
            .bind(now())
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        // Les modules d'un châssis le suivent.
        sqlx::query("UPDATE articles SET location_id = ?, container_id = ?, rev = rev + 1, updated_at = ? WHERE project_id = ? AND json_extract(data, '$.host.article_id') = ?")
            .bind(location_id)
            .bind(container_id)
            .bind(now())
            .bind(&pid)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        touch_project(&mut tx, &pid).await?;
        tx.commit().await?;
        self.article(id).await?.ok_or(StoreError::NotFound)
    }

    /// Sort un exemplaire d'un article en quantité N vers un autre endroit : l'article
    /// d'origine perd une unité, un nouvel article reçoit l'exemplaire (avec son repère).
    pub async fn split_instance(
        &self,
        article_id: &str,
        rev: i64,
        instance_id: &str,
        location_id: &str,
        container_id: Option<&str>,
    ) -> Result<Article> {
        let mut tx = self.pool.begin().await?;
        check_rev(&mut tx, "articles", article_id, rev).await?;
        let a: (String, String, Option<String>, String) =
            sqlx::query_as("SELECT project_id, nature, item_id, data FROM articles WHERE id = ?")
                .bind(article_id)
                .fetch_one(&mut *tx)
                .await?;
        let id = new_id();
        let t = now();
        let pos: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM articles WHERE project_id = ?",
        )
        .bind(&a.0)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO articles (id, project_id, location_id, container_id, position, nature, item_id, quantity, data, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?)")
            .bind(&id)
            .bind(&a.0)
            .bind(location_id)
            .bind(container_id)
            .bind(pos)
            .bind(&a.1)
            .bind(&a.2)
            .bind(&a.3)
            .bind(&t)
            .bind(&t)
            .execute(&mut *tx)
            .await?;
        let moved = sqlx::query("UPDATE instances SET article_id = ?, rev = rev + 1, updated_at = ? WHERE id = ? AND article_id = ?")
            .bind(&id)
            .bind(&t)
            .bind(instance_id)
            .bind(article_id)
            .execute(&mut *tx)
            .await?;
        if moved.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        sqlx::query("DELETE FROM placements WHERE instance_id = ?")
            .bind(instance_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE articles SET quantity = quantity - 1, rev = rev + 1, updated_at = ? WHERE id = ?").bind(&t).bind(article_id).execute(&mut *tx).await?;
        touch_project(&mut tx, &a.0).await?;
        tx.commit().await?;
        self.article(&id).await?.ok_or(StoreError::NotFound)
    }

    /// Glisse un module dans un châssis (ou l'en sort avec `None`).
    pub async fn set_article_host(&self, id: &str, host: Option<&ModuleHost>) -> Result<Article> {
        let mut tx = self.pool.begin().await?;
        let pid: String = match host {
            Some(h) => sqlx::query_scalar("UPDATE articles SET data = json_set(data, '$.host', json(?)), rev = rev + 1, updated_at = ? WHERE id = ? RETURNING project_id")
                .bind(serde_json::to_string(h).unwrap_or_default())
                .bind(now())
                .bind(id)
                .fetch_one(&mut *tx)
                .await?,
            None => sqlx::query_scalar("UPDATE articles SET data = json_remove(data, '$.host'), rev = rev + 1, updated_at = ? WHERE id = ? RETURNING project_id")
                .bind(now())
                .bind(id)
                .fetch_one(&mut *tx)
                .await?,
        };
        touch_project(&mut tx, &pid).await?;
        tx.commit().await?;
        self.article(id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn delete_article(&self, id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        // Supprimer un châssis supprime les modules qu'il porte.
        sqlx::query("DELETE FROM articles WHERE json_extract(data, '$.host.article_id') = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let pid: Option<String> =
            sqlx::query_scalar("DELETE FROM articles WHERE id = ? RETURNING project_id")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        // Supprimer un adaptateur supprime ses câbles et ses attaches.
        sqlx::query("DELETE FROM cables WHERE a LIKE ? OR b LIKE ?")
            .bind(format!("a:{id}:%"))
            .bind(format!("a:{id}:%"))
            .execute(&mut *tx)
            .await?;
        touch_project(&mut tx, &pid.ok_or(StoreError::NotFound)?).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Projets qui utilisent une fiche du catalogue.
    pub async fn item_usage(&self) -> Result<Vec<(String, i64, i64)>> {
        let rows = sqlx::query("SELECT item_id, COUNT(DISTINCT project_id) AS projects, SUM(quantity) AS qty FROM articles WHERE item_id IS NOT NULL GROUP BY item_id")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get("item_id"), r.get("projects"), r.get("qty")))
            .collect())
    }

    // ----- Exemplaires -----

    pub async fn instance(&self, id: &str) -> Result<Option<Instance>> {
        Ok(sqlx::query("SELECT * FROM instances WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(Instance::from_row))
    }

    pub async fn update_instance(
        &self,
        id: &str,
        rev: i64,
        serial: Option<&str>,
        mac: Option<&str>,
        inventory_tag: Option<&str>,
    ) -> Result<Instance> {
        let mut tx = self.pool.begin().await?;
        check_rev(&mut tx, "instances", id, rev).await?;
        let pid: String = sqlx::query_scalar("UPDATE instances SET serial = ?, mac = ?, inventory_tag = ?, rev = rev + 1, updated_at = ? WHERE id = ? RETURNING project_id")
            .bind(serial)
            .bind(mac)
            .bind(inventory_tag)
            .bind(now())
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        touch_project(&mut tx, &pid).await?;
        tx.commit().await?;
        self.instance(id).await?.ok_or(StoreError::NotFound)
    }

    // ----- Tâches -----

    pub async fn tasks(&self, project_id: Option<&str>) -> Result<Vec<Task>> {
        let rows = match project_id {
            Some(p) => {
                sqlx::query(
                    "SELECT * FROM tasks WHERE project_id = ? ORDER BY position, created_at",
                )
                .bind(p)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query("SELECT * FROM tasks ORDER BY project_id, position")
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        Ok(rows.iter().map(Task::from_row).collect())
    }

    pub async fn task(&self, id: &str) -> Result<Option<Task>> {
        Ok(sqlx::query("SELECT * FROM tasks WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(Task::from_row))
    }

    pub async fn insert_task(
        &self,
        project_id: &str,
        data: &TaskData,
        actor: Option<&str>,
    ) -> Result<Task> {
        let id = new_id();
        let t = now();
        let mut tx = self.pool.begin().await?;
        let pos: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM tasks WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO tasks (id, project_id, position, data, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(project_id)
            .bind(pos)
            .bind(serde_json::to_string(data).unwrap_or_default())
            .bind(actor)
            .bind(&t)
            .bind(&t)
            .execute(&mut *tx)
            .await?;
        touch_project(&mut tx, project_id).await?;
        tx.commit().await?;
        self.task(&id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn update_task(&self, id: &str, rev: i64, data: &TaskData) -> Result<Task> {
        let mut tx = self.pool.begin().await?;
        check_rev(&mut tx, "tasks", id, rev).await?;
        let pid: String = sqlx::query_scalar("UPDATE tasks SET data = ?, rev = rev + 1, updated_at = ? WHERE id = ? RETURNING project_id")
            .bind(serde_json::to_string(data).unwrap_or_default())
            .bind(now())
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        touch_project(&mut tx, &pid).await?;
        tx.commit().await?;
        self.task(id).await?.ok_or(StoreError::NotFound)
    }

    pub async fn delete_task(&self, id: &str) -> Result<()> {
        let done = sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if done.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    // ----- Historique -----

    /// Comme `audit`, en rattachant l'entrée à un projet.
    #[allow(clippy::too_many_arguments)]
    pub async fn audit_project<B: Serialize, A: Serialize>(
        &self,
        project_id: &str,
        actor_id: Option<&str>,
        action: &str,
        object_type: &str,
        object_id: &str,
        before: Option<&B>,
        after: Option<&A>,
    ) -> Result<()> {
        sqlx::query("INSERT INTO audit (at, actor_id, action, object_type, object_id, before, after, project_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(now())
            .bind(actor_id)
            .bind(action)
            .bind(object_type)
            .bind(object_id)
            .bind(before.map(|b| serde_json::to_string(b).unwrap_or_default()))
            .bind(after.map(|a| serde_json::to_string(a).unwrap_or_default()))
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn project_history(
        &self,
        project_id: &str,
        limit: i64,
    ) -> Result<Vec<crate::AuditEntry>> {
        let rows = sqlx::query("SELECT * FROM audit WHERE project_id = ? ORDER BY id DESC LIMIT ?")
            .bind(project_id)
            .bind(limit.clamp(1, 500))
            .fetch_all(&self.pool)
            .await?;
        let parse = |s: Option<String>| s.and_then(|s| serde_json::from_str(&s).ok());
        Ok(rows
            .iter()
            .map(|r| crate::AuditEntry {
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

#[cfg(test)]
mod tests {
    use super::*;
    use strata_domain::project::ContainerKind;

    async fn setup() -> (Store, Project, Location, Container) {
        let s = Store::open_in_memory().await.unwrap();
        let p = s
            .insert_project(
                &ProjectData {
                    name: "Finale".into(),
                    ..Default::default()
                },
                None,
            )
            .await
            .unwrap();
        let l = s
            .insert_location(&p.id, "Car régie", "CARRE")
            .await
            .unwrap();
        let c = s
            .insert_container(
                &p.id,
                &l.id,
                "Baie V1",
                &ContainerData {
                    kind: ContainerKind::Rack,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        (s, p, l, c)
    }

    #[tokio::test]
    async fn project_names_are_unique() {
        let (s, ..) = setup().await;
        let e = s
            .insert_project(
                &ProjectData {
                    name: "FINALE".into(),
                    ..Default::default()
                },
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(e, StoreError::Conflict("project_name")), "{e:?}");
        assert!(
            s.project_name_taken(" finale", None).await.unwrap()
                || s.project_name_taken("finale", None).await.unwrap()
        );
    }

    #[tokio::test]
    async fn instances_follow_quantity_and_merge() {
        let (s, p, l, c) = setup().await;
        let (a, new) = s
            .add_article(
                &p.id,
                &l.id,
                Some(&c.id),
                ArticleNature::Equipment,
                Some("eq1"),
                3,
                None,
                true,
            )
            .await
            .unwrap();
        assert!(new);
        let (a2, new) = s
            .add_article(
                &p.id,
                &l.id,
                Some(&c.id),
                ArticleNature::Equipment,
                Some("eq1"),
                2,
                None,
                true,
            )
            .await
            .unwrap();
        assert!(!new);
        assert_eq!((a2.id.clone(), a2.quantity), (a.id.clone(), 5));
        let inst = s.article_instances(&a.id).await.unwrap();
        assert_eq!(inst.len(), 5);
        let tags: std::collections::HashSet<_> = inst.iter().map(|i| i.tag.clone()).collect();
        assert_eq!(tags.len(), 5);
        let a3 = s.set_article_quantity(&a.id, a2.rev, 2).await.unwrap();
        assert_eq!(s.article_instances(&a3.id).await.unwrap().len(), 2);
        assert!(matches!(
            s.set_article_quantity(&a.id, a2.rev, 1).await,
            Err(StoreError::Stale)
        ));
        // Sortir un exemplaire vers les équipements libres de l'emplacement.
        let first = s.article_instances(&a.id).await.unwrap()[0].clone();
        let solo = s
            .split_instance(&a.id, a3.rev, &first.id, &l.id, None)
            .await
            .unwrap();
        assert_eq!(solo.container_id, None);
        assert_eq!(
            s.article_instances(&solo.id).await.unwrap()[0].tag,
            first.tag
        );
        assert_eq!(s.article(&a.id).await.unwrap().unwrap().quantity, 1);
    }

    #[tokio::test]
    async fn deleting_a_project_cascades() {
        let (s, p, l, c) = setup().await;
        s.add_article(
            &p.id,
            &l.id,
            Some(&c.id),
            ArticleNature::Equipment,
            Some("eq1"),
            2,
            None,
            true,
        )
        .await
        .unwrap();
        s.insert_task(
            &p.id,
            &TaskData {
                title: "Étiqueter".into(),
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
        s.delete_project(&p.id).await.unwrap();
        let inv = s.inventory(&p.id).await.unwrap();
        assert!(
            inv.locations.is_empty()
                && inv.containers.is_empty()
                && inv.articles.is_empty()
                && inv.instances.is_empty()
        );
        assert!(s.tasks(Some(&p.id)).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn moving_a_container_moves_its_articles() {
        let (s, p, l, c) = setup().await;
        let l2 = s
            .insert_location(&p.id, "Bord terrain", "BORDT")
            .await
            .unwrap();
        s.add_article(
            &p.id,
            &l.id,
            Some(&c.id),
            ArticleNature::Adapter,
            Some("ad1"),
            4,
            None,
            true,
        )
        .await
        .unwrap();
        s.update_container(&c.id, c.rev, &l2.id, &c.name, &c.data)
            .await
            .unwrap();
        let inv = s.inventory(&p.id).await.unwrap();
        assert!(inv.articles.iter().all(|a| a.location_id == l2.id));
        assert!(
            inv.instances.is_empty(),
            "pas d'exemplaire pour un adaptateur"
        );
    }
}
